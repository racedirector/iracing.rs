//! Concrete acquisition boundaries for recorded and live iRacing telemetry.
//!
//! [`ibt::IbtReader`] owns immutable `.ibt` bytes, parses and validates their
//! complete layout once, and then performs only indexed copies and cursor
//! updates. [`live::LiveReader`] is bound to one mapped-memory generation,
//! validates its static layout once, and performs the SDK control-read/copy
//! protocol for dynamic observations.
//!
//! Both readers return owned wire-buffer newtypes. Schema construction, YAML
//! cleanup and parsing, telemetry decoding, and delivery policy remain above
//! this module.

use crate::{IRacingSDKError, Result};

pub mod ibt;
pub mod live;

/// A byte range proven to fit one source extent at construction time.
///
/// This is deliberately only arithmetic state: source-specific readers own the
/// bytes or mapping identity that makes the containment proof meaningful.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CheckedRegion {
    offset: usize,
    length: usize,
}

impl CheckedRegion {
    pub(super) fn new(offset: usize, length: usize, extent: usize) -> Result<Self> {
        let end = offset
            .checked_add(length)
            .ok_or_else(|| IRacingSDKError::memory_access_error(offset))?;
        if end > extent {
            return Err(IRacingSDKError::memory_access_error(end));
        }

        Ok(Self { offset, length })
    }

    pub(super) fn offset(self) -> usize {
        self.offset
    }

    pub(super) fn length(self) -> usize {
        self.length
    }

    pub(super) fn end(self) -> usize {
        // `new` proves this addition cannot overflow.
        self.offset + self.length
    }

    pub(super) fn overlaps(self, other: Self) -> bool {
        self.offset < other.end() && other.offset < self.end()
    }
}
