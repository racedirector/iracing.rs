//! Frame adapter trait for type-safe telemetry extraction

use std::sync::Arc;

use crate::TelemetryLayout;

use super::AdapterValidation;

/// Dual-phase frame adapter trait providing connection-time validation and runtime extraction.
///
/// `validate_layout()` runs once at connection time, `adapt()` runs at 60Hz using
/// pre-computed extraction plans. This separation minimizes runtime overhead.
pub trait FrameAdapter: Sized {
    /// Validate adapter against telemetry layout at connection time.
    ///
    /// This method:
    /// - Checks that all required fields exist in the layout
    /// - Validates storage type and scalar/array shape compatibility
    /// - Builds pre-computed extraction plans for runtime efficiency
    /// - Retains the shared layout used to validate the extraction plan
    ///
    /// # Errors
    /// Returns an error when required fields are absent or incompatible. Derived
    /// adapters treat incompatible optional/default fields as absent.
    ///
    /// # Performance
    /// Connections call this method when creating each subscription, not per frame.
    /// Expensive operations like HashMap lookups and string matching are acceptable here.
    fn validate_layout(layout: &Arc<TelemetryLayout>) -> crate::Result<AdapterValidation>;

    /// Extract data from frame packet using pre-validated extraction plan.
    ///
    /// This method runs at 60Hz and must be extremely efficient:
    /// - Uses direct memory access with pre-validated offsets
    /// - No HashMap lookups or string operations
    /// - All field existence and type checks already performed
    ///
    /// # Performance Target
    /// Must complete in <1ms for typical adapter with 10-20 fields.
    ///
    /// The frame packet provides zero-copy access to telemetry data via its
    /// Arc<[u8]> buffer. Adapters extract fields directly from packet.data().
    fn adapt(packet: &crate::FramePacket, validation: &AdapterValidation) -> Self;
}
