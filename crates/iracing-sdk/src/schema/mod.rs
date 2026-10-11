//! Typed session metadata and optional discovery of unmodeled session fields.
//!
//! [`session`] owns YAML deserialization into [`SessionInfo`] and its domain
//! types. Source acquisition, physical frame geometry, telemetry layout,
//! buffering, and runtime caching are owned by their respective modules.
//! Session parsing is portable; optional features add schema generation and
//! unknown-field retention without moving source acquisition into this module.

pub mod session;

pub use session::SessionInfo;
