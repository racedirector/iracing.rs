//! Schema Discovery & Buffer Management
//!
//! This module provides comprehensive schema discovery for iRacing telemetry data,
//! including live variable schema discovery and session metadata parsing.
//!
//! # Architecture
//!
//! The schema system follows a layered approach:
//! - [`crate::headers::Header`] represents and validates the common SDK header
//! - Live variable schema discovery parses shared-memory variable definitions
//! - Session parsing converts iRacing's YAML metadata into typed structures
//! - Caching avoids redundant session parsing when its version is unchanged
//!
//! # Feature-Specific Implementation
//!
//! Live variable discovery is compiled only for Windows targets. Header types and
//! session schema parsing remain available on every supported platform.

#[cfg(windows)]
pub mod variables;

pub mod session;

pub use session::{SessionInfo, SessionInfoParser};
