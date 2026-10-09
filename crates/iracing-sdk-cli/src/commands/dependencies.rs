//! Capabilities declared by SDK CLI commands, independent of concrete connections.
//!
//! Each command accepts only the behaviors it consumes. The executable's
//! application implements these contracts; fakes need no simulator or Win32 API.

use anyhow::Result;
use iracing_irsdk::Header;
use iracing_sdk::{FieldLayout, FramePacket, VariableHeaders, schema::SessionInfo};

/// An owned live header snapshot. Setup/readiness policy belongs to the implementor.
pub(crate) trait LiveHeaders {
    /// Return an owned header snapshot.
    ///
    /// Propagates implementation-specific initialization and acquisition errors.
    fn live_header(&mut self) -> Result<Header>;
}

/// The current decoded session, or absence when no session is published.
pub(crate) trait LiveSessions {
    /// Return the decoded session, or `None` when no session is published.
    ///
    /// Propagates initialization, acquisition, and parsing errors; failures are
    /// not treated as session absence.
    fn live_session(&mut self) -> Result<Option<SessionInfo>>;
}

/// An owned snapshot of live variable descriptions.
pub(crate) trait LiveVariables {
    /// Return owned variable descriptions, which may be empty if metadata is absent.
    ///
    /// Propagates initialization, acquisition, and metadata decoding errors.
    fn live_variable_headers(&mut self) -> Result<VariableHeaders>;
}

/// Capture behaviors used by live snapshot and recording commands.
///
/// The synchronous operation waits for one frame or errors on disconnect; async
/// recording ends with `None` on disconnect. Implementors own connection setup,
/// layout validation, readiness, and wait policy. Canceling a request must leave
/// application resource ownership with the implementor.
pub(crate) trait LiveFrames {
    /// Return owned field definitions for live capture; the list may be empty.
    ///
    /// Propagates resource initialization and layout acquisition errors.
    fn live_fields(&mut self) -> Result<Vec<FieldLayout>>;
    /// Wait for one owned live frame.
    ///
    /// Returns an error on disconnect and propagates initialization, acquisition,
    /// wait, and frame construction errors.
    fn next_live_frame(&mut self) -> Result<FramePacket>;
    /// Wait cooperatively for one owned frame, returning `None` on disconnect.
    ///
    /// Propagates initialization, acquisition, wait, and frame construction errors.
    /// Canceling the request leaves resource ownership with the implementor.
    async fn next_live_frame_async(&mut self) -> Result<Option<FramePacket>>;
}
