//! Source-neutral byte acquisition for iRacing telemetry.
//!
//! This module separates three concerns that were historically combined by the
//! live connection and IBT reader:
//!
//! 1. [`access_source`] defines checked byte ranges and absolute-offset reads.
//!    This is the narrow common capability shared by immutable IBT bytes and a
//!    live memory mapping.
//! 2. [`header`] interprets the offsets and lengths advertised by one
//!    [`Header`](crate::types::Header) snapshot and converts copied regions into
//!    the SDK's owned buffer newtypes.
//! 3. [`live`] and [`ibt`] supply source-specific storage and
//!    acquisition policy. Live acquisition must account for concurrently
//!    changing memory, while IBT acquisition adds immutable file layout and
//!    cursor semantics.
//!
//! The abstraction deliberately stops below providers. Readers acquire exact
//! wire snapshots; providers remain responsible for converting those snapshots
//! into schemas, cleaned session YAML, and frame packets.
//!
//! # Design rationale
//!
//! The common abstraction is absolute-offset access rather than a shared
//! high-level reader because live telemetry and recorded telemetry have
//! different correctness rules. IBT bytes are immutable and support ordinary
//! indexed access. Live bytes can change during a copy and require tick-based
//! validation and retry policy. Sharing only the byte-addressing and
//! header-interpretation layers removes duplicated bounds logic without hiding
//! those behavioral differences behind an overly broad trait.
//!
//! Snapshot methods return owned newtypes rather than borrowed slices. This
//! makes the lifetime and consistency boundary visible: once acquisition has
//! accepted a copy, providers can process or publish it without retaining a
//! file cursor, mapping borrow, or raw pointer. The newtypes also prevent the
//! acquisition layer from prematurely interpreting schema entries, YAML, or
//! telemetry values.
//!
//! # Consistency boundary
//!
//! Absolute-offset access does not imply that multiple reads observe one atomic
//! source state. That distinction does not matter for immutable IBT bytes, but it
//! is fundamental for live shared memory. The future live reader must surround
//! copies with the SDK's tick/version checks. Keeping that policy above
//! [`access_source::RandomAccessSource`] prevents the shared trait from making a
//! coherence promise that a raw memory mapping cannot satisfy.
//!
//! The IBT path uses these representations directly. The Windows connection is
//! being migrated separately because its consistency protocol must remain at
//! the live acquisition boundary.

pub mod access_source;
pub mod header;
pub mod ibt;
pub mod live;
