#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![warn(missing_docs)]
//! Higher-level streaming adapter layer over [`iracing_sdk`].
//!
//! This crate wraps the low-level parsing primitives in `iracing-sdk` with a
//! stream-oriented abstraction for consuming iRacing telemetry.
//!
//! # Core concepts
//!
//! - [`FramePacket`] — the fundamental unit of telemetry data: a raw frame buffer
//!   together with its tick index, session version, and a shared [`VariableSchema`].
//! - [`Provider`] — source of telemetry frames; call [`Provider::next_frame`] in a
//!   loop to drive playback or live data.
//! - [`IbtProvider`] — [`Provider`] implementation for `.ibt` replay files (cross-platform).
//! - `LiveProvider` (Windows only) — [`Provider`] implementation for live shared memory.
//! - [`FrameAdapter`] — dual-phase typed extraction: implement
//!   [`FrameAdapter::validate_schema`] once at connection time to build an extraction
//!   plan, then call [`FrameAdapter::adapt`] each frame for zero-`HashMap` access.
//! - [`DynamicFrame`] — by-name variable lookup without a typed struct; useful for
//!   exploration and debugging.

mod adapters;
mod dynamic_frame;
mod frame;
mod provider;
mod providers;

// Re-export iRacing SDK
pub use iracing_sdk::*;

pub use adapters::*;
pub use dynamic_frame::*;
pub use frame::FramePacket;
pub use provider::Provider;
pub use providers::ibt::IbtProvider;
#[cfg(windows)]
#[cfg_attr(docsrs, doc(cfg(windows)))]
pub use providers::live::LiveProvider;
