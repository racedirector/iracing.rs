//! Byte geometry with constructor-specific validation.
//!
//! [`ByteRegion`] proves representable half-open arithmetic, not source
//! containment. Specialized constructors add their documented checks:
//! metadata descriptors validate advertised dimensions; [`MetadataRegions`]
//! additionally checks IBT source bounds and overlap; [`VariableRegion`] offers
//! both unbounded header conversion and frame-bounded construction.
//!
//! [`FramesRegion`] partitions an existing span into complete frames;
//! [`FrameRegion`] carries one frame's geometry established by its caller.
//! None owns storage or stabilizes externally mutable bytes. Access boundaries
//! must check geometry against the retained source and separately uphold storage
//! lifetime and snapshot obligations. Adjacent constructor tests own edge cases.

mod bytes;
mod frame;
mod frames;
mod metadata;
mod session_info;
mod variable;
mod variable_headers;

pub use {
    bytes::ByteRegion, frame::FrameRegion, frames::FramesRegion, metadata::MetadataRegions,
    session_info::SessionInfoRegion, variable::VariableRegion,
    variable_headers::VariableHeadersRegion,
};
