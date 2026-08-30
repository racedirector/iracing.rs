//! Concrete mapped-memory reader for live iRacing telemetry.
//!
//! [`LiveReader::new`] parses and validates the static layout of one
//! [`MappedView`] generation. Later frame observations read only dynamic
//! control words, classify the producer tick, and apply the SDK 1.20
//! read/barrier/copy/barrier/read consistency protocol. Replacing, resizing, or
//! reopening a mapping requires constructing a new reader; [`LiveReader::reset`]
//! clears only the tick baseline.

use std::{
    fmt,
    marker::PhantomData,
    ptr::NonNull,
    sync::{
        Mutex, MutexGuard,
        atomic::{Ordering, compiler_fence},
    },
};

use crate::{
    IRacingSDKError, Result,
    irsdk::{
        FrameBuffer, Header, SessionInfoBuffer, StatusField, VariableBuffer, VariableHeader,
        VariableHeadersBuffer, WireType,
    },
};

use super::CheckedRegion;

const MAX_COPY_ATTEMPTS: usize = 2;
const STATUS_OFFSET: usize = std::mem::offset_of!(Header, status);
const SESSION_UPDATE_OFFSET: usize = std::mem::offset_of!(Header, session_info_update);
const SESSION_LENGTH_OFFSET: usize = std::mem::offset_of!(Header, session_info_len);
const SESSION_OFFSET_OFFSET: usize = std::mem::offset_of!(Header, session_info_offset);
const CURRENT_BUFFER_TICK_OFFSET: usize = std::mem::offset_of!(Header, current_buffer_tick_count);
const CURRENT_BUFFER_OFFSET: usize = std::mem::offset_of!(Header, current_buffer);
const BUFFER_DESCRIPTORS_OFFSET: usize = std::mem::offset_of!(Header, buffers);
const DESCRIPTOR_TICK_OFFSET: usize = std::mem::offset_of!(VariableBuffer, tick_count);
const DESCRIPTOR_TICK_BEGIN_OFFSET: usize = std::mem::offset_of!(VariableBuffer, tick_count_begin);

/// One coherent, owned live telemetry observation.
#[derive(Debug, Clone)]
pub struct LiveFrameSnapshot {
    buffer: FrameBuffer,
    tick_count: i32,
    session_info_update: i32,
}

impl LiveFrameSnapshot {
    /// Returns the complete owned frame bytes.
    pub fn buffer(&self) -> &FrameBuffer {
        &self.buffer
    }

    /// Releases the complete owned frame bytes without copying them.
    pub fn into_buffer(self) -> FrameBuffer {
        self.buffer
    }

    /// Returns the descriptor tick accepted for these frame bytes.
    pub fn tick_count(&self) -> i32 {
        self.tick_count
    }

    /// Returns the session version observed by the accepted attempt.
    pub fn session_info_update(&self) -> i32 {
        self.session_info_update
    }
}

/// One stable, owned copy of the current live session-information region.
#[derive(Debug, Clone)]
pub struct LiveSessionSnapshot {
    buffer: SessionInfoBuffer,
    session_info_update: i32,
}

impl LiveSessionSnapshot {
    /// Returns the stable session version associated with the owned bytes.
    pub fn session_info_update(&self) -> i32 {
        self.session_info_update
    }

    /// Releases the owned session-information bytes without copying them.
    pub fn into_buffer(self) -> SessionInfoBuffer {
        self.buffer
    }
}

/// Cumulative acquisition diagnostics for one [`LiveReader`].
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LiveReaderDiagnostics {
    /// Number of rejected frame-copy attempts followed by a retry.
    pub frame_retries: u64,
    /// Number of observations skipped after both frame attempts were unstable.
    pub frame_contention_skips: u64,
    /// Number of rejected session-copy attempts followed by a retry.
    pub session_retries: u64,
    /// Number of session observations skipped after both attempts were unstable.
    pub session_contention_skips: u64,
    /// Number of initial observations, disconnects, or tick regressions that reset the baseline.
    pub baseline_resets: u64,
}

#[derive(Debug, Clone, Copy)]
struct DescriptorControls {
    tick_count: CheckedRegion,
    tick_count_begin: CheckedRegion,
}

/// Validated layout and acquisition state for one mapped-memory generation.
///
/// The mapping base and extent supplied to later methods must match the view
/// used at construction. Mapping replacement, reconnect, resize, or remap is a
/// new generation and requires a new reader. [`Self::reset`] intentionally does
/// not revalidate layout or authorize a different view.
#[derive(Debug)]
pub struct LiveReader {
    mapping_base: usize,
    mapped_length: usize,
    tick_rate: i32,
    buffer_count: usize,
    frame_length: usize,
    frame_regions: [Option<CheckedRegion>; Header::MAX_BUFFERS],
    descriptor_controls: [Option<DescriptorControls>; Header::MAX_BUFFERS],
    variable_headers_region: Option<CheckedRegion>,
    status_control: CheckedRegion,
    session_update_control: CheckedRegion,
    session_length_control: CheckedRegion,
    session_offset_control: CheckedRegion,
    current_buffer_tick_control: CheckedRegion,
    current_buffer_control: CheckedRegion,
    last_tick_count: Option<i32>,
    diagnostics: Mutex<LiveReaderDiagnostics>,
}

impl LiveReader {
    /// Parses and validates the static layout of one mapped-memory generation.
    pub fn new(view: &MappedView<'_>) -> Result<Self> {
        let header = view.header_snapshot()?;
        header.validate_live()?;

        let buffer_count = usize::try_from(header.buffer_count).map_err(|_| {
            IRacingSDKError::parse_error(
                "live layout",
                format!("Buffer count cannot be negative: {}", header.buffer_count),
            )
        })?;
        let frame_length = usize::try_from(header.buffer_length).map_err(|_| {
            IRacingSDKError::parse_error(
                "live layout",
                format!("Frame length cannot be negative: {}", header.buffer_length),
            )
        })?;

        let mut frame_regions = [None; Header::MAX_BUFFERS];
        let mut descriptor_controls = [None; Header::MAX_BUFFERS];
        for (index, descriptor) in header.buffers[..buffer_count].iter().enumerate() {
            let frame_offset = usize::try_from(descriptor.buffer_offset).map_err(|_| {
                IRacingSDKError::parse_error(
                    "live layout",
                    format!(
                        "Variable buffer {index} offset cannot be negative: {}",
                        descriptor.buffer_offset
                    ),
                )
            })?;
            frame_regions[index] =
                Some(CheckedRegion::new(frame_offset, frame_length, view.length)?);

            let descriptor_offset = BUFFER_DESCRIPTORS_OFFSET
                .checked_add(index * VariableBuffer::WIRE_SIZE)
                .ok_or_else(|| IRacingSDKError::memory_access_error(BUFFER_DESCRIPTORS_OFFSET))?;
            descriptor_controls[index] = Some(DescriptorControls {
                tick_count: view
                    .checked_control::<i32>(descriptor_offset + DESCRIPTOR_TICK_OFFSET)?,
                tick_count_begin: view
                    .checked_control::<i32>(descriptor_offset + DESCRIPTOR_TICK_BEGIN_OFFSET)?,
            });
        }

        let variable_count = usize::try_from(header.variable_count).map_err(|_| {
            IRacingSDKError::parse_error(
                "live layout",
                format!(
                    "Variable count cannot be negative: {}",
                    header.variable_count
                ),
            )
        })?;
        let variable_headers_length = variable_count
            .checked_mul(VariableHeader::WIRE_SIZE)
            .ok_or_else(|| {
                IRacingSDKError::parse_error(
                    "live layout",
                    "Variable-header region length overflowed",
                )
            })?;
        let variable_header_offset =
            usize::try_from(header.variable_header_offset).map_err(|_| {
                IRacingSDKError::parse_error(
                    "live layout",
                    format!(
                        "Variable-header offset cannot be negative: {}",
                        header.variable_header_offset
                    ),
                )
            })?;
        let variable_headers_region = (variable_headers_length != 0)
            .then(|| {
                CheckedRegion::new(variable_header_offset, variable_headers_length, view.length)
            })
            .transpose()?;

        Ok(Self {
            mapping_base: view.base_address(),
            mapped_length: view.length,
            tick_rate: header.tick_rate,
            buffer_count,
            frame_length,
            frame_regions,
            descriptor_controls,
            variable_headers_region,
            status_control: view.checked_control::<i32>(STATUS_OFFSET)?,
            session_update_control: view.checked_control::<i32>(SESSION_UPDATE_OFFSET)?,
            session_length_control: view.checked_control::<i32>(SESSION_LENGTH_OFFSET)?,
            session_offset_control: view.checked_control::<i32>(SESSION_OFFSET_OFFSET)?,
            current_buffer_tick_control: view.checked_control::<i32>(CURRENT_BUFFER_TICK_OFFSET)?,
            current_buffer_control: view.checked_control::<u8>(CURRENT_BUFFER_OFFSET)?,
            last_tick_count: None,
            diagnostics: Mutex::new(LiveReaderDiagnostics::default()),
        })
    }

    /// Returns cumulative retry, skip, and reset counters.
    pub fn diagnostics(&self) -> LiveReaderDiagnostics {
        *self.lock_diagnostics()
    }

    /// Clears the producer tick baseline without clearing validated layout.
    pub fn reset(&mut self) {
        if self.last_tick_count.take().is_some() {
            self.diagnostics_mut().baseline_resets += 1;
        }
    }

    /// Returns the validated live tick rate.
    pub fn tick_rate(&self) -> i32 {
        self.tick_rate
    }

    /// Returns the validated frame byte length.
    pub fn frame_length(&self) -> usize {
        self.frame_length
    }

    /// Returns the validated number of live frame buffers.
    pub fn buffer_count(&self) -> usize {
        self.buffer_count
    }

    /// Copies the current complete header without revalidating static layout.
    pub fn header_snapshot(&self, view: &MappedView<'_>) -> Result<Header> {
        self.ensure_mapping(view)?;
        view.header_snapshot()
    }

    /// Reads the current connection-status control word.
    pub fn connection_status(&self, view: &MappedView<'_>) -> Result<StatusField> {
        self.ensure_mapping(view)?;
        Ok(StatusField::from_bits(view.read_i32(self.status_control)))
    }

    /// Returns whether the current status advertises a connected simulator.
    pub fn is_connected(&self, view: &MappedView<'_>) -> Result<bool> {
        self.connection_status(view).map(StatusField::is_connected)
    }

    /// Reads the current session-information update counter.
    pub fn session_info_update(&self, view: &MappedView<'_>) -> Result<i32> {
        self.ensure_mapping(view)?;
        Ok(view.read_i32(self.session_update_control))
    }

    /// Reads the cached current-buffer tick control word.
    pub fn current_buffer_tick_count(&self, view: &MappedView<'_>) -> Result<i32> {
        self.ensure_mapping(view)?;
        Ok(view.read_i32(self.current_buffer_tick_control))
    }

    /// Reads and validates the dynamic current-buffer index.
    pub fn current_buffer_index(&self, view: &MappedView<'_>) -> Result<usize> {
        self.ensure_mapping(view)?;
        self.read_current_buffer_index(view)
    }

    /// Copies the variable-header array accepted during construction.
    pub fn variable_headers_buffer(
        &self,
        view: &MappedView<'_>,
    ) -> Result<Option<VariableHeadersBuffer>> {
        self.ensure_mapping(view)?;
        Ok(self
            .variable_headers_region
            .map(|region| VariableHeadersBuffer::from_snapshot(view.copy_prevalidated(region))))
    }

    /// Acquires the next coherent frame according to the SDK 1.20 protocol.
    ///
    /// `Ok(None)` means the source is disconnected, the first/reset tick only
    /// established a baseline, the tick is unchanged, or both permitted copy
    /// attempts observed contention. Static layout is not reparsed or
    /// revalidated on this path.
    pub fn next_frame(&mut self, view: &MappedView<'_>) -> Result<Option<LiveFrameSnapshot>> {
        self.ensure_mapping(view)?;

        for attempt in 0..MAX_COPY_ATTEMPTS {
            if !StatusField::from_bits(view.read_i32(self.status_control)).is_connected() {
                self.reset();
                return Ok(None);
            }

            let buffer_index = self.read_current_buffer_index(view)?;
            let controls = self.descriptor_controls[buffer_index]
                .expect("validated active buffer has control words");
            let tick_count = view.read_i32(controls.tick_count);

            match self.classify_tick(tick_count) {
                TickChange::Baseline | TickChange::Reset => {
                    self.last_tick_count = Some(tick_count);
                    self.diagnostics_mut().baseline_resets += 1;
                    return Ok(None);
                }
                TickChange::Unchanged => return Ok(None),
                TickChange::New => {}
            }

            // This sequence intentionally mirrors SDK 1.20 `irsdk_getNewData`:
            // completed tick, compiler read barrier, complete frame copy,
            // compiler read barrier, then begin tick from the same descriptor.
            MappedView::read_barrier();
            let buffer = FrameBuffer::from_snapshot(
                view.copy_prevalidated(
                    self.frame_regions[buffer_index]
                        .expect("validated active buffer has a frame region"),
                ),
            );
            MappedView::read_barrier();
            let tick_count_begin = view.read_i32(controls.tick_count_begin);

            if tick_count == tick_count_begin {
                let session_info_update = view.read_i32(self.session_update_control);
                self.last_tick_count = Some(tick_count);
                return Ok(Some(LiveFrameSnapshot {
                    buffer,
                    tick_count,
                    session_info_update,
                }));
            }

            if attempt + 1 < MAX_COPY_ATTEMPTS {
                self.diagnostics_mut().frame_retries += 1;
            }
        }

        self.diagnostics_mut().frame_contention_skips += 1;
        Ok(None)
    }

    /// Copies the current YAML region only when version, offset, and length are stable.
    ///
    /// Session offset and length are dynamic and therefore checked on this
    /// slower path. A changed proof retries once; a second change returns
    /// `Ok(None)` without publishing torn bytes.
    pub fn session_info(&self, view: &MappedView<'_>) -> Result<Option<LiveSessionSnapshot>> {
        self.ensure_mapping(view)?;

        for attempt in 0..MAX_COPY_ATTEMPTS {
            let before = self.read_session_proof(view);
            let offset = usize::try_from(before.offset).map_err(|_| {
                IRacingSDKError::parse_error(
                    "live session layout",
                    format!("Session offset cannot be negative: {}", before.offset),
                )
            })?;
            let length = usize::try_from(before.length).map_err(|_| {
                IRacingSDKError::parse_error(
                    "live session layout",
                    format!("Session length cannot be negative: {}", before.length),
                )
            })?;
            if length == 0 {
                return Ok(None);
            }
            let region = CheckedRegion::new(offset, length, self.mapped_length)?;

            MappedView::read_barrier();
            let buffer = SessionInfoBuffer::from_snapshot(view.copy_prevalidated(region));
            MappedView::read_barrier();
            let after = self.read_session_proof(view);

            if before == after {
                return Ok(Some(LiveSessionSnapshot {
                    buffer,
                    session_info_update: after.update,
                }));
            }

            if attempt + 1 < MAX_COPY_ATTEMPTS {
                self.lock_diagnostics().session_retries += 1;
            }
        }

        self.lock_diagnostics().session_contention_skips += 1;
        Ok(None)
    }

    fn ensure_mapping(&self, view: &MappedView<'_>) -> Result<()> {
        if view.base_address() != self.mapping_base || view.length != self.mapped_length {
            return Err(IRacingSDKError::parse_error(
                "live mapped view",
                "Mapped-memory generation changed; construct a new LiveReader",
            ));
        }
        Ok(())
    }

    fn read_current_buffer_index(&self, view: &MappedView<'_>) -> Result<usize> {
        let index = usize::from(view.read_u8(self.current_buffer_control));
        if index >= self.buffer_count {
            return Err(IRacingSDKError::parse_error(
                "live frame acquisition",
                format!(
                    "Current buffer index {index} is outside buffer count {}",
                    self.buffer_count
                ),
            ));
        }
        Ok(index)
    }

    fn read_session_proof(&self, view: &MappedView<'_>) -> SessionProof {
        SessionProof {
            update: view.read_i32(self.session_update_control),
            length: view.read_i32(self.session_length_control),
            offset: view.read_i32(self.session_offset_control),
        }
    }

    fn classify_tick(&self, current_tick_count: i32) -> TickChange {
        match self.last_tick_count {
            None => TickChange::Baseline,
            Some(last) if current_tick_count > last => TickChange::New,
            Some(last) if current_tick_count == last => TickChange::Unchanged,
            Some(_) => TickChange::Reset,
        }
    }

    fn diagnostics_mut(&mut self) -> &mut LiveReaderDiagnostics {
        match self.diagnostics.get_mut() {
            Ok(diagnostics) => diagnostics,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn lock_diagnostics(&self) -> MutexGuard<'_, LiveReaderDiagnostics> {
        match self.diagnostics.lock() {
            Ok(diagnostics) => diagnostics,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TickChange {
    Baseline,
    Unchanged,
    New,
    Reset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SessionProof {
    update: i32,
    length: i32,
    offset: i32,
}

/// A lifetime-bound, non-owning view of readable mapped memory.
///
/// This is the narrow unsafe boundary for live access. Control words are read
/// with aligned volatile loads; the SDK compiler barriers and prevalidated
/// copies remain adjacent here and in [`LiveReader::next_frame`]. The view owns
/// neither the mapping nor its operating-system handle.
pub struct MappedView<'mapping> {
    base: NonNull<u8>,
    length: usize,
    #[cfg(any(test, feature = "benchmark"))]
    copy_observer: Option<&'mapping dyn Fn(usize, usize)>,
    _mapping: PhantomData<&'mapping [u8]>,
}

impl fmt::Debug for MappedView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MappedView")
            .field("base", &self.base)
            .field("length", &self.length)
            .finish_non_exhaustive()
    }
}

impl<'mapping> MappedView<'mapping> {
    /// Creates a non-owning mapped view from a base address and readable extent.
    ///
    /// # Safety
    ///
    /// `base` must remain readable for `length` bytes throughout `'mapping`.
    /// The owner must not unmap the memory while this view or a borrow exists.
    /// External writers must obey the iRacing SDK synchronization protocol, and
    /// a reader must be reconstructed whenever this mapping generation changes.
    pub unsafe fn from_raw_parts(base: NonNull<u8>, length: usize) -> Self {
        Self {
            base,
            length,
            #[cfg(any(test, feature = "benchmark"))]
            copy_observer: None,
            _mapping: PhantomData,
        }
    }

    /// Attaches a deterministic copy observer for tests and benchmarks.
    #[cfg(any(test, feature = "benchmark"))]
    #[doc(hidden)]
    pub fn with_copy_observer(mut self, observer: &'mapping dyn Fn(usize, usize)) -> Self {
        self.copy_observer = Some(observer);
        self
    }

    fn base_address(&self) -> usize {
        self.base.as_ptr() as usize
    }

    fn header_snapshot(&self) -> Result<Header> {
        let region = CheckedRegion::new(0, Header::WIRE_SIZE, self.length)?;
        let mut bytes = [0_u8; Header::WIRE_SIZE];
        self.copy_into(region, &mut bytes);
        Header::read_from_bytes(&bytes)
    }

    fn checked_control<T>(&self, offset: usize) -> Result<CheckedRegion> {
        let region = CheckedRegion::new(offset, std::mem::size_of::<T>(), self.length)?;
        let address = self
            .base_address()
            .checked_add(offset)
            .ok_or_else(|| IRacingSDKError::memory_access_error(offset))?;
        if !address.is_multiple_of(std::mem::align_of::<T>()) {
            return Err(IRacingSDKError::parse_error(
                "live mapped view",
                format!(
                    "Control word at offset {offset} is not aligned to {} bytes",
                    std::mem::align_of::<T>()
                ),
            ));
        }
        Ok(region)
    }

    #[inline]
    fn read_i32(&self, region: CheckedRegion) -> i32 {
        debug_assert_eq!(region.length(), std::mem::size_of::<i32>());
        let pointer = unsafe { self.base.as_ptr().add(region.offset()).cast::<i32>() };
        debug_assert_eq!(pointer.align_offset(std::mem::align_of::<i32>()), 0);

        // SAFETY: LiveReader construction proved this aligned control word is
        // in the same still-mapped extent. Volatility prevents the compiler
        // from replacing or coalescing the producer-owned control read.
        i32::from_le(unsafe { std::ptr::read_volatile(pointer) })
    }

    #[inline]
    fn read_u8(&self, region: CheckedRegion) -> u8 {
        debug_assert_eq!(region.length(), std::mem::size_of::<u8>());
        let pointer = unsafe { self.base.as_ptr().add(region.offset()) };

        // SAFETY: LiveReader construction proved this byte is in the same
        // still-mapped extent; `u8` has no alignment requirement.
        unsafe { std::ptr::read_volatile(pointer) }
    }

    /// Matches SDK 1.20 `_ReadBarrier`: a compiler ordering boundary, separate
    /// from the volatility of individual producer-owned control reads.
    #[inline]
    fn read_barrier() {
        compiler_fence(Ordering::SeqCst);
    }

    fn copy_prevalidated(&self, region: CheckedRegion) -> Vec<u8> {
        debug_assert!(region.end() <= self.length);
        let mut bytes = Vec::with_capacity(region.length());

        // SAFETY: The reader's construction or dynamic session check proved the
        // complete source region is within this mapping. The vector allocation
        // owns capacity for exactly `region.length()` bytes and does not overlap
        // the mapped source.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.base.as_ptr().add(region.offset()),
                bytes.as_mut_ptr(),
                region.length(),
            );
            bytes.set_len(region.length());
        }
        self.observe_copy(region);
        bytes
    }

    fn copy_into(&self, region: CheckedRegion, destination: &mut [u8]) {
        debug_assert_eq!(region.length(), destination.len());
        debug_assert!(region.end() <= self.length);

        // SAFETY: The checked region is wholly inside the mapped extent and the
        // destination is a distinct initialized Rust slice of the same length.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.base.as_ptr().add(region.offset()),
                destination.as_mut_ptr(),
                destination.len(),
            );
        }
        self.observe_copy(region);
    }

    #[inline]
    fn observe_copy(&self, region: CheckedRegion) {
        #[cfg(any(test, feature = "benchmark"))]
        if let Some(observer) = self.copy_observer {
            observer(region.offset(), region.length());
        }

        #[cfg(not(any(test, feature = "benchmark")))]
        let _ = region;
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, collections::VecDeque};

    use proptest::prelude::*;

    use super::*;
    use crate::irsdk::{IRSDK_VERSION, StatusField};

    const SESSION_OFFSET: usize = 112;
    const FRAME_0_OFFSET: usize = 256;
    const FRAME_1_OFFSET: usize = 264;
    const FRAME_2_OFFSET: usize = 272;
    const FRAME_LENGTH: usize = 4;

    struct Mutation {
        after_offset: usize,
        after_length: usize,
        replacements: Vec<(usize, Vec<u8>)>,
    }

    struct TestMapping {
        bytes: RefCell<Box<[u8]>>,
        mutations: RefCell<VecDeque<Mutation>>,
        copies: RefCell<Vec<(usize, usize)>>,
    }

    impl TestMapping {
        fn new(bytes: Vec<u8>) -> Self {
            Self {
                bytes: RefCell::new(bytes.into_boxed_slice()),
                mutations: RefCell::new(VecDeque::new()),
                copies: RefCell::new(Vec::new()),
            }
        }

        fn with_view<T>(&self, operation: impl FnOnce(&MappedView<'_>) -> T) -> T {
            let (base, length) = {
                let bytes = self.bytes.borrow();
                (
                    NonNull::new(bytes.as_ptr().cast_mut()).expect("nonempty test mapping"),
                    bytes.len(),
                )
            };
            let observer = |offset, length| self.after_copy(offset, length);
            // SAFETY: The boxed allocation is stable and remains readable for
            // the duration of `operation`; replacements never resize it.
            let view =
                unsafe { MappedView::from_raw_parts(base, length) }.with_copy_observer(&observer);
            operation(&view)
        }

        fn replace(&self, offset: usize, replacement: &[u8]) {
            let mut bytes = self.bytes.borrow_mut();
            bytes[offset..offset + replacement.len()].copy_from_slice(replacement);
        }

        fn replace_header(&self, header: &Header) {
            self.replace(0, wire_bytes(header));
        }

        fn mutate_after_copy(
            &self,
            after_offset: usize,
            after_length: usize,
            replacements: Vec<(usize, Vec<u8>)>,
        ) {
            self.mutations.borrow_mut().push_back(Mutation {
                after_offset,
                after_length,
                replacements,
            });
        }

        fn clear_copies(&self) {
            self.copies.borrow_mut().clear();
        }

        fn after_copy(&self, offset: usize, length: usize) {
            self.copies.borrow_mut().push((offset, length));
            let mutation = {
                let mut mutations = self.mutations.borrow_mut();
                let should_mutate = mutations.front().is_some_and(|mutation| {
                    mutation.after_offset == offset && mutation.after_length == length
                });
                should_mutate.then(|| mutations.pop_front().unwrap())
            };
            if let Some(mutation) = mutation {
                for (replacement_offset, replacement) in mutation.replacements {
                    self.replace(replacement_offset, &replacement);
                }
            }
        }
    }

    fn wire_bytes<T: WireType>(value: &T) -> &[u8] {
        // SAFETY: `WireType` guarantees that its complete object representation
        // is initialized and has exactly `WIRE_SIZE` readable bytes.
        unsafe { std::slice::from_raw_parts((value as *const T).cast::<u8>(), T::WIRE_SIZE) }
    }

    fn live_header(tick: i32, tick_begin: i32, session_update: i32) -> Header {
        Header::new(
            IRSDK_VERSION,
            StatusField::CONNECTED,
            60,
            session_update,
            8,
            SESSION_OFFSET as i32,
            0,
            120,
            3,
            FRAME_LENGTH as i32,
            tick,
            1,
            [
                VariableBuffer::new(
                    tick.saturating_sub(1),
                    FRAME_0_OFFSET as i32,
                    tick.saturating_sub(1),
                ),
                VariableBuffer::new(tick, FRAME_1_OFFSET as i32, tick_begin),
                VariableBuffer::new(
                    tick.saturating_sub(2),
                    FRAME_2_OFFSET as i32,
                    tick.saturating_sub(2),
                ),
                VariableBuffer::new(0, 0, 0),
            ],
        )
    }

    fn mapping_for(header: &Header, frame: [u8; FRAME_LENGTH], yaml: &[u8]) -> TestMapping {
        let mut bytes = vec![0_u8; 320];
        bytes[..Header::WIRE_SIZE].copy_from_slice(wire_bytes(header));
        bytes[SESSION_OFFSET..SESSION_OFFSET + yaml.len()].copy_from_slice(yaml);
        bytes[FRAME_1_OFFSET..FRAME_1_OFFSET + FRAME_LENGTH].copy_from_slice(&frame);
        TestMapping::new(bytes)
    }

    fn reader_for(mapping: &TestMapping) -> LiveReader {
        mapping.with_view(LiveReader::new).unwrap()
    }

    fn next_frame(
        reader: &mut LiveReader,
        mapping: &TestMapping,
    ) -> Result<Option<LiveFrameSnapshot>> {
        mapping.with_view(|view| reader.next_frame(view))
    }

    fn establish_baseline(reader: &mut LiveReader, mapping: &TestMapping) {
        assert!(next_frame(reader, mapping).unwrap().is_none());
    }

    #[test]
    fn mapped_view_copies_checked_regions() -> Result<()> {
        let mapping = TestMapping::new(vec![10_u8, 20, 30, 40]);
        mapping.with_view(|view| {
            let region = CheckedRegion::new(1, 2, 4)?;
            assert_eq!(view.copy_prevalidated(region), [20, 30]);
            assert!(CheckedRegion::new(3, 2, 4).is_err());
            Result::<()>::Ok(())
        })
    }

    #[test]
    fn first_observation_and_tick_regression_only_establish_a_baseline() {
        let mapping = mapping_for(&live_header(10, 10, 1), [10; 4], b"yaml\0\0\0\0");
        let mut reader = reader_for(&mapping);

        assert!(next_frame(&mut reader, &mapping).unwrap().is_none());
        assert!(next_frame(&mut reader, &mapping).unwrap().is_none());

        mapping.replace_header(&live_header(3, 3, 1));
        assert!(next_frame(&mut reader, &mapping).unwrap().is_none());
        assert_eq!(reader.diagnostics().baseline_resets, 2);
    }

    #[test]
    fn unchanged_tick_does_not_copy_or_allocate_a_frame() {
        let mapping = mapping_for(&live_header(10, 10, 1), [10; 4], b"yaml\0\0\0\0");
        let mut reader = reader_for(&mapping);
        establish_baseline(&mut reader, &mapping);
        mapping.clear_copies();

        assert!(next_frame(&mut reader, &mapping).unwrap().is_none());
        assert!(mapping.copies.borrow().is_empty());
    }

    #[test]
    fn stable_new_tick_returns_owned_bytes_and_same_attempt_metadata() {
        let mapping = mapping_for(&live_header(10, 10, 4), [10; 4], b"yaml\0\0\0\0");
        let mut reader = reader_for(&mapping);
        establish_baseline(&mut reader, &mapping);
        mapping.replace_header(&live_header(11, 11, 5));
        mapping.replace(FRAME_1_OFFSET, &[11; 4]);

        let snapshot = next_frame(&mut reader, &mapping).unwrap().unwrap();
        mapping.replace(FRAME_1_OFFSET, &[99; 4]);

        assert_eq!(snapshot.tick_count(), 11);
        assert_eq!(snapshot.session_info_update(), 5);
        assert_eq!(Vec::<u8>::from(snapshot.into_buffer()), [11; 4]);
    }

    #[test]
    fn advertised_current_descriptor_is_used_instead_of_highest_tick() {
        let mut header = live_header(10, 10, 1);
        header.buffers[0] = VariableBuffer::new(100, FRAME_0_OFFSET as i32, 100);
        let mapping = mapping_for(&header, [10; 4], b"yaml\0\0\0\0");
        let mut reader = reader_for(&mapping);
        establish_baseline(&mut reader, &mapping);
        mapping.replace_header(&live_header(11, 11, 1));
        mapping.replace(FRAME_1_OFFSET, &[11; 4]);

        let snapshot = next_frame(&mut reader, &mapping).unwrap().unwrap();
        assert_eq!(snapshot.tick_count(), 11);
        assert_eq!(Vec::<u8>::from(snapshot.into_buffer()), [11; 4]);
    }

    #[test]
    fn one_concurrent_frame_change_retries_and_accepts_second_attempt() {
        let mapping = mapping_for(&live_header(1, 1, 1), [1; 4], b"yaml\0\0\0\0");
        let mut reader = reader_for(&mapping);
        establish_baseline(&mut reader, &mapping);
        mapping.replace_header(&live_header(2, 2, 2));
        mapping.replace(FRAME_1_OFFSET, &[2; 4]);
        mapping.mutate_after_copy(
            FRAME_1_OFFSET,
            FRAME_LENGTH,
            vec![
                (0, wire_bytes(&live_header(3, 3, 3)).to_vec()),
                (FRAME_1_OFFSET, vec![3; 4]),
            ],
        );

        let snapshot = next_frame(&mut reader, &mapping).unwrap().unwrap();

        assert_eq!(snapshot.tick_count(), 3);
        assert_eq!(snapshot.session_info_update(), 3);
        assert_eq!(Vec::<u8>::from(snapshot.into_buffer()), [3; 4]);
        assert_eq!(reader.diagnostics().frame_retries, 1);
    }

    #[test]
    fn two_concurrent_frame_changes_skip_without_advancing_baseline() {
        let mapping = mapping_for(&live_header(1, 1, 1), [1; 4], b"yaml\0\0\0\0");
        let mut reader = reader_for(&mapping);
        establish_baseline(&mut reader, &mapping);
        mapping.replace_header(&live_header(2, 2, 2));
        mapping.replace(FRAME_1_OFFSET, &[2; 4]);
        for tick in [3, 4] {
            mapping.mutate_after_copy(
                FRAME_1_OFFSET,
                FRAME_LENGTH,
                vec![
                    (0, wire_bytes(&live_header(tick, tick, tick)).to_vec()),
                    (FRAME_1_OFFSET, vec![tick as u8; 4]),
                ],
            );
        }

        assert!(next_frame(&mut reader, &mapping).unwrap().is_none());
        assert_eq!(reader.diagnostics().frame_retries, 1);
        assert_eq!(reader.diagnostics().frame_contention_skips, 1);

        let snapshot = next_frame(&mut reader, &mapping).unwrap().unwrap();
        assert_eq!(snapshot.tick_count(), 4);
    }

    #[test]
    fn stable_session_snapshot_owns_actual_version_and_bytes() {
        let mapping = mapping_for(&live_header(1, 1, 7), [1; 4], b"old\0\0\0\0\0");
        let reader = reader_for(&mapping);

        let snapshot = mapping
            .with_view(|view| reader.session_info(view))
            .unwrap()
            .unwrap();
        mapping.replace(SESSION_OFFSET, b"new\0\0\0\0\0");

        assert_eq!(snapshot.session_info_update(), 7);
        assert_eq!(String::from(snapshot.into_buffer()), "old");
    }

    #[test]
    fn changed_session_during_copy_retries_current_region() {
        let mapping = mapping_for(&live_header(1, 1, 7), [1; 4], b"old\0\0\0\0\0");
        mapping.mutate_after_copy(
            SESSION_OFFSET,
            8,
            vec![
                (0, wire_bytes(&live_header(1, 1, 8)).to_vec()),
                (SESSION_OFFSET, b"new\0\0\0\0\0".to_vec()),
            ],
        );
        let reader = reader_for(&mapping);

        let snapshot = mapping
            .with_view(|view| reader.session_info(view))
            .unwrap()
            .unwrap();

        assert_eq!(snapshot.session_info_update(), 8);
        assert_eq!(String::from(snapshot.into_buffer()), "new");
        assert_eq!(reader.diagnostics().session_retries, 1);
    }

    #[test]
    fn two_session_changes_skip_the_unstable_snapshot() {
        let mapping = mapping_for(&live_header(1, 1, 7), [1; 4], b"one\0\0\0\0\0");
        for (version, contents) in [(8, b"two\0\0\0\0\0"), (9, b"tri\0\0\0\0\0")] {
            mapping.mutate_after_copy(
                SESSION_OFFSET,
                8,
                vec![
                    (0, wire_bytes(&live_header(1, 1, version)).to_vec()),
                    (SESSION_OFFSET, contents.to_vec()),
                ],
            );
        }
        let reader = reader_for(&mapping);

        assert!(
            mapping
                .with_view(|view| reader.session_info(view))
                .unwrap()
                .is_none()
        );
        assert_eq!(reader.diagnostics().session_retries, 1);
        assert_eq!(reader.diagnostics().session_contention_skips, 1);
    }

    #[test]
    fn disconnect_clears_the_tick_baseline() {
        let mapping = mapping_for(&live_header(10, 10, 1), [10; 4], b"yaml\0\0\0\0");
        let mut reader = reader_for(&mapping);
        establish_baseline(&mut reader, &mapping);

        let mut disconnected = live_header(10, 10, 1);
        disconnected.status = StatusField::empty();
        mapping.replace_header(&disconnected);
        assert!(next_frame(&mut reader, &mapping).unwrap().is_none());

        mapping.replace_header(&live_header(20, 20, 1));
        assert!(next_frame(&mut reader, &mapping).unwrap().is_none());
        mapping.replace_header(&live_header(21, 21, 1));
        assert!(next_frame(&mut reader, &mapping).unwrap().is_some());
    }

    #[test]
    fn construction_rejects_malformed_and_out_of_bounds_layouts() {
        let mut malformed = live_header(1, 1, 1);
        malformed.current_buffer = 3;
        let malformed_mapping = mapping_for(&malformed, [1; 4], b"yaml\0\0\0\0");
        assert!(malformed_mapping.with_view(LiveReader::new).is_err());

        let mut out_of_bounds = live_header(1, 1, 1);
        out_of_bounds.buffers[2] = VariableBuffer::new(0, 319, 0);
        let out_of_bounds_mapping = mapping_for(&out_of_bounds, [1; 4], b"yaml\0\0\0\0");
        assert!(out_of_bounds_mapping.with_view(LiveReader::new).is_err());
    }

    #[test]
    fn invalid_dynamic_current_buffer_fails_before_copy() {
        let mapping = mapping_for(&live_header(1, 1, 1), [1; 4], b"yaml\0\0\0\0");
        let mut reader = reader_for(&mapping);
        mapping.replace(CURRENT_BUFFER_OFFSET, &[3]);
        mapping.clear_copies();

        assert!(next_frame(&mut reader, &mapping).is_err());
        assert!(mapping.copies.borrow().is_empty());
    }

    #[test]
    fn signed_tick_boundary_does_not_wrap_into_a_false_new_frame() {
        let mapping = mapping_for(&live_header(i32::MAX, i32::MAX, 1), [1; 4], b"yaml\0\0\0\0");
        let mut reader = reader_for(&mapping);
        establish_baseline(&mut reader, &mapping);
        mapping.replace_header(&live_header(i32::MIN, i32::MIN, 1));

        assert!(next_frame(&mut reader, &mapping).unwrap().is_none());
    }

    #[test]
    fn mapping_replacement_requires_reconstruction() {
        let first = mapping_for(&live_header(1, 1, 1), [1; 4], b"yaml\0\0\0\0");
        let second = mapping_for(&live_header(1, 1, 1), [1; 4], b"yaml\0\0\0\0");
        let mut reader = reader_for(&first);

        assert!(second.with_view(|view| reader.next_frame(view)).is_err());
    }

    proptest! {
        #[test]
        fn arbitrary_live_layouts_never_copy_out_of_bounds(
            session_offset in any::<i32>(),
            session_length in any::<i32>(),
            variable_count in any::<i32>(),
            variable_offset in any::<i32>(),
            buffer_count in any::<i32>(),
            buffer_length in any::<i32>(),
            current_buffer in any::<u8>(),
            tick in any::<i32>(),
            tick_begin in any::<i32>(),
            source_length in Header::WIRE_SIZE..2048_usize,
        ) {
            let header = Header::new(
                IRSDK_VERSION,
                StatusField::CONNECTED,
                60,
                1,
                session_length,
                session_offset,
                variable_count,
                variable_offset,
                buffer_count,
                buffer_length,
                tick,
                current_buffer,
                [VariableBuffer::new(tick, session_offset, tick_begin); Header::MAX_BUFFERS],
            );
            let mut bytes = vec![0_u8; source_length];
            bytes[..Header::WIRE_SIZE].copy_from_slice(wire_bytes(&header));
            let mapping = TestMapping::new(bytes);

            let _ = mapping.with_view(LiveReader::new);
            let all_copies_in_bounds = mapping.copies.borrow().iter().all(|(offset, length)| {
                offset.checked_add(*length).is_some_and(|end| end <= source_length)
            });
            prop_assert!(all_copies_in_bounds);
        }
    }
}
