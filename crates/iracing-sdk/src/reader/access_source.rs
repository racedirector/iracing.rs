//! Checked absolute-offset access to a finite byte source.
//!
//! [`RandomAccessSource`] is intentionally smaller than [`std::io::Read`] plus
//! [`std::io::Seek`]. It has no mutable cursor: every operation identifies its
//! absolute offset, which fits both immutable in-memory IBT data and borrowed
//! memory mappings. A sequential [`Read`] can enter this layer by being fully
//! materialized into [`OwnedBytes`].
//!
//! This module guarantees bounds-checked copies. It does **not** guarantee that
//! the backing bytes remain unchanged during or between copies. Source-specific
//! readers are responsible for any stronger consistency protocol.
//!
//! # Why not `Read + Seek`?
//!
//! `Seek` mutates one shared cursor and therefore requires `&mut self`. The SDK
//! header instead describes independent absolute regions, and a mapped view has
//! no natural cursor at all. Positioned reads allow immutable access, avoid
//! save/restore cursor bookkeeping, and make random frame access explicit.
//!
//! # Why materialize a general `Read`?
//!
//! A plain `Read` cannot move backward. [`OwnedBytes::from_reader`] therefore
//! consumes it to EOF once and provides deterministic random access afterward.
//! This preserves support for files, in-memory cursors, and non-seekable inputs.
//! A future lazy file implementation can implement [`RandomAccessSource`]
//! directly without weakening the trait's guarantees.
//!
//! # Example
//!
//! ```
//! use iracing_sdk::reader::access_source::{
//!     ByteRegion, OwnedBytes, RandomAccessSource,
//! };
//!
//! # fn main() -> iracing_sdk::Result<()> {
//! let source = OwnedBytes::from(vec![10, 20, 30, 40]);
//! let snapshot = source.snapshot(ByteRegion::new(1, 2)?)?;
//!
//! assert_eq!(snapshot, [20, 30]);
//! # Ok(())
//! # }
//! ```

use std::{io::Read, sync::Arc};

use crate::{IRacingSDKError, Result};

/// A half-open byte range described by an absolute offset and byte length.
///
/// A region represents `offset..offset + length`. Construction proves that the
/// end calculation does not overflow `usize`; it does not prove that the region
/// is contained by a particular [`RandomAccessSource`]. Source containment is
/// checked by [`RandomAccessSource::validate_region`] before a copy.
///
/// Zero-length regions are valid, including a region whose offset equals the
/// source length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRegion {
    /// Absolute byte offset at which the region begins.
    offset: usize,
    /// Number of bytes in the region.
    length: usize,
}

impl ByteRegion {
    /// Creates a region whose exclusive end can be represented by `usize`.
    ///
    /// This validates arithmetic only. Call
    /// [`RandomAccessSource::validate_region`] to validate the region against a
    /// concrete source.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError::Memory`] when `offset + length` overflows.
    pub fn new(offset: usize, length: usize) -> Result<Self> {
        offset
            .checked_add(length)
            .ok_or_else(|| IRacingSDKError::memory_access_error(offset))?;

        Ok(Self { offset, length })
    }

    /// Returns the absolute byte offset at which this region begins.
    pub fn offset(self) -> usize {
        self.offset
    }

    /// Returns the number of bytes contained by this region.
    pub fn length(self) -> usize {
        self.length
    }

    /// Returns the exclusive end offset of this region.
    ///
    /// This addition cannot overflow because it was checked by [`Self::new`].
    pub fn end(self) -> usize {
        // Construction proves that this addition cannot overflow.
        self.offset + self.length
    }
}

/// A finite byte source that supports exact reads at absolute offsets.
///
/// Implementations must either fill the entire destination or return an error;
/// partial successful reads are not permitted. Reads do not advance implicit
/// state, so callers can request regions in any order.
///
/// The trait specifies addressability and ownership of returned snapshots, not
/// temporal consistency. In particular, an implementation backed by live
/// shared memory may change while a read is in progress. A higher layer must
/// perform the appropriate before/after consistency checks.
pub trait RandomAccessSource {
    /// Returns the fixed number of addressable bytes in this source view.
    ///
    /// Implementations must return the same length for the lifetime of the
    /// source value. The contents within that extent may still change.
    fn len(&self) -> usize;

    /// Returns whether this source exposes no addressable bytes.
    ///
    /// This is derived from [`Self::len`] so implementations cannot disagree
    /// about the empty-source boundary.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Copies exactly `destination.len()` bytes beginning at `offset`.
    ///
    /// An empty destination is a valid read when `offset <= self.len()`.
    ///
    /// # Errors
    ///
    /// Returns an error when the requested range overflows or extends beyond
    /// the addressable source. Implementations may return additional
    /// source-specific errors when the copy cannot be completed.
    fn read_exact_at(&self, offset: usize, destination: &mut [u8]) -> Result<()>;

    /// Verifies that the complete region is addressable by this source.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError::Memory`] with the requested exclusive end
    /// offset when the region extends beyond [`Self::len`].
    fn validate_region(&self, region: ByteRegion) -> Result<()> {
        if region.end() > self.len() {
            return Err(IRacingSDKError::memory_access_error(region.end()));
        }

        Ok(())
    }

    /// Copies a complete region into a newly allocated owned snapshot.
    ///
    /// The returned vector always has exactly [`ByteRegion::length`] bytes.
    /// Later mutations of the source cannot alter the returned snapshot.
    ///
    /// # Errors
    ///
    /// Propagates range-validation and source-copy errors. No partial snapshot
    /// is returned on failure.
    fn snapshot(&self, region: ByteRegion) -> Result<Vec<u8>> {
        self.validate_region(region)?;

        let mut bytes = vec![0; region.length()];
        self.read_exact_at(region.offset(), &mut bytes)?;
        Ok(bytes)
    }
}

/// An immutable, reference-counted random-access byte source.
///
/// Cloning `OwnedBytes` clones its [`Arc`] rather than copying the underlying
/// allocation. Because the bytes are immutable, each positioned read observes
/// the same contents and requires no source-specific consistency protocol.
#[derive(Debug, Clone)]
pub struct OwnedBytes {
    /// Shared immutable storage containing the complete source.
    bytes: Arc<[u8]>,
}

impl OwnedBytes {
    /// Reads a sequential source to EOF and materializes it for random access.
    ///
    /// This is the adapter from a general [`Read`] input to the absolute-offset
    /// model. It intentionally buffers the entire stream; callers that require
    /// lazy file I/O should provide a different [`RandomAccessSource`]
    /// implementation rather than changing this type's ownership contract.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError::Buffer`] with the underlying I/O error as its
    /// source when the input cannot be read completely. Bytes read before the
    /// error are discarded.
    pub fn from_reader<R: Read>(mut reader: R) -> Result<Self> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(|source| IRacingSDKError::Buffer {
                context: "materializing random-access source".to_owned(),
                buffer_index: None,
                source: Some(Box::new(source)),
            })?;

        Ok(Self::from(bytes))
    }
}

impl From<Vec<u8>> for OwnedBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }
}

impl From<Arc<[u8]>> for OwnedBytes {
    fn from(bytes: Arc<[u8]>) -> Self {
        Self { bytes }
    }
}

impl RandomAccessSource for OwnedBytes {
    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn read_exact_at(&self, offset: usize, destination: &mut [u8]) -> Result<()> {
        let region = ByteRegion::new(offset, destination.len())?;
        self.validate_region(region)?;
        destination.copy_from_slice(&self.bytes[offset..region.end()]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_region_rejects_overflow() {
        assert!(ByteRegion::new(usize::MAX, 1).is_err());
    }

    #[test]
    fn owned_bytes_supports_positioned_reads_and_snapshots() -> Result<()> {
        let source = OwnedBytes::from(vec![0, 1, 2, 3, 4]);
        let mut destination = [0; 2];

        source.read_exact_at(2, &mut destination)?;

        assert_eq!(destination, [2, 3]);
        assert_eq!(source.snapshot(ByteRegion::new(1, 3)?)?, [1, 2, 3]);
        Ok(())
    }

    #[test]
    fn owned_bytes_rejects_out_of_bounds_regions() -> Result<()> {
        let source = OwnedBytes::from(vec![0, 1, 2]);

        assert!(source.snapshot(ByteRegion::new(2, 2)?).is_err());
        assert!(source.read_exact_at(3, &mut [0; 1]).is_err());
        Ok(())
    }

    #[test]
    fn owned_bytes_materializes_a_sequential_reader() -> Result<()> {
        let source = OwnedBytes::from_reader(std::io::Cursor::new([4, 5, 6]))?;

        assert_eq!(source.snapshot(ByteRegion::new(0, 3)?)?, [4, 5, 6]);
        Ok(())
    }
}
