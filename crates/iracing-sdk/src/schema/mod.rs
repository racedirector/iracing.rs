//! Schema Discovery & Buffer Management
//!
//! This module provides comprehensive schema discovery for iRacing telemetry data,
//! including header parsing, variable schema building, and buffer management.
//!
//! # Architecture
//!
//! The schema system follows a layered approach:
//! - Header parsing extracts iRacing's `irsdk_header` structure from shared memory
//! - Variable schema building parses the variable definitions into structured metadata
//! - Buffer management handles iRacing's 4-buffer rotation system
//! - Caching optimizes performance by avoiding redundant parsing operations
//!
//! # Feature-Specific Implementation
//!
//! Live header and variable discovery are compiled only for Windows targets;
//! session schema parsing remains available on every supported platform.

pub mod session;

pub use session::{SessionInfo, SessionInfoParser};
