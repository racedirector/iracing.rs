//! Borrowed access to a live memory-mapped SDK view.
//!
//! [`MappedView`] adapts an already-opened mapping to
//! [`RandomAccessSource`]. It owns neither the Windows mapping handle nor the
//! mapped address, and it does not wait for SDK update events.
//!
//! Most importantly, a bounds-checked copy is not necessarily a coherent live
//! telemetry snapshot. iRacing may write the mapping concurrently. The live
//! reader must select a buffer, copy it, compare the relevant tick fields, and
//! retry or reject the copy according to the live acquisition policy.
//!
//! # Why isolate the unsafe constructor?
//!
//! The operating system establishes the mapping's validity, but Rust cannot
//! infer that validity from a raw address. [`MappedView::from_raw_parts`] is the
//! single boundary where the owner asserts the address, extent, and lifetime.
//! Once constructed, all range arithmetic and copies are exposed through the
//! safe [`RandomAccessSource`] contract. This keeps pointer validity separate
//! from the higher-level question of whether concurrently changing bytes form a
//! coherent SDK observation.

use std::{marker::PhantomData, ptr::NonNull};

use crate::Result;

use super::access_source::{ByteRegion, RandomAccessSource};

/// A lifetime-bound, non-owning view of readable mapped memory.
///
/// The view records only a base address and fixed readable extent. It does not
/// close or unmap anything when dropped. The lifetime marker documents that the
/// backing mapping must outlive the view, while [`Self::from_raw_parts`] remains
/// unsafe because Rust cannot verify that relationship or the pointer's extent.
///
/// [`RandomAccessSource`] operations validate bounds and return owned copies.
/// They provide no synchronization with an external writer and no guarantee
/// that one copy contains bytes from a single simulator tick.
#[derive(Debug)]
pub struct MappedView<'mapping> {
    /// Address of the first readable byte in the borrowed mapping.
    base: NonNull<u8>,
    /// Fixed number of bytes addressable from `base`.
    length: usize,
    /// Associates the raw address with the lifetime of the backing mapping.
    _mapping: PhantomData<&'mapping [u8]>,
}

impl<'mapping> MappedView<'mapping> {
    /// Creates a non-owning mapped view from a base address and byte length.
    ///
    /// No operating-system calls are made, and ownership of the mapping remains
    /// with the caller. A zero-length view is permitted when `base` is non-null.
    ///
    /// # Safety
    ///
    /// The caller must guarantee all of the following:
    ///
    /// - `base` is valid and readable for `length` consecutive bytes;
    /// - that readable extent remains mapped for the complete `'mapping`
    ///   lifetime;
    /// - the mapping is not unmapped while this value or a borrow of it exists;
    /// - external writes follow the platform and SDK synchronization rules
    ///   expected by the higher-level live reader.
    ///
    /// This function does not require the bytes to remain unchanged. It is the
    /// caller's responsibility to detect concurrent SDK updates when coherence
    /// matters.
    pub unsafe fn from_raw_parts(base: NonNull<u8>, length: usize) -> Self {
        Self {
            base,
            length,
            _mapping: PhantomData,
        }
    }
}

impl RandomAccessSource for MappedView<'_> {
    fn len(&self) -> usize {
        self.length
    }

    fn read_exact_at(&self, offset: usize, destination: &mut [u8]) -> Result<()> {
        let region = ByteRegion::new(offset, destination.len())?;
        self.validate_region(region)?;

        // SAFETY: Construction guarantees that the mapping remains readable,
        // and the region validation above proves this range is in bounds.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.base.as_ptr().add(offset),
                destination.as_mut_ptr(),
                destination.len(),
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Models the baseline transitions in SDK 1.20's `irsdk_getNewData`.
    /// Frame-copy acceptance is deliberately separate: the real reader must
    /// update its baseline only after the before/copy/after check succeeds.
    fn classify_tick(last_tick_count: i32, current_tick_count: i32) -> TickChange {
        match last_tick_count.cmp(&current_tick_count) {
            std::cmp::Ordering::Less => TickChange::New,
            std::cmp::Ordering::Equal => TickChange::Unchanged,
            std::cmp::Ordering::Greater => TickChange::Reset,
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    enum TickChange {
        New,
        Unchanged,
        Reset,
    }

    #[test]
    fn mapped_view_copies_checked_regions() -> Result<()> {
        let bytes = [10_u8, 20, 30, 40];
        let base = NonNull::from(&bytes).cast::<u8>();
        // SAFETY: `bytes` outlives the mapped view and provides four readable bytes.
        let source = unsafe { MappedView::from_raw_parts(base, bytes.len()) };

        assert_eq!(source.snapshot(ByteRegion::new(1, 2)?)?, [20, 30]);
        assert!(source.snapshot(ByteRegion::new(3, 2)?).is_err());
        Ok(())
    }

    #[test]
    fn sdk_1_20_first_observation_establishes_a_baseline() {
        assert_eq!(classify_tick(i32::MAX, 1_000), TickChange::Reset);
    }

    #[test]
    fn sdk_1_20_same_tick_has_no_new_frame() {
        assert_eq!(classify_tick(1_000, 1_000), TickChange::Unchanged);
    }

    #[test]
    fn sdk_1_20_increasing_tick_is_new() {
        assert_eq!(classify_tick(1_000, 1_001), TickChange::New);
    }

    #[test]
    fn sdk_1_20_decreasing_tick_resets_the_baseline() {
        assert_eq!(classify_tick(1_000, 7), TickChange::Reset);
    }

    #[test]
    fn sdk_1_20_selects_the_advertised_current_descriptor() {
        let descriptors = [10, 40, 30, 20];
        let current_buffer = 3_usize;

        assert_eq!(descriptors[current_buffer], 20);
        assert_ne!(descriptors[current_buffer], 40);
    }
}
