//! Frame adapter trait for type-safe telemetry extraction

use crate::{Result, VariableSchema};

use super::AdapterValidation;

/// Dual-phase frame adapter trait providing connection-time validation and runtime extraction.
///
/// `validate_schema()` runs once at connection time, `adapt()` runs at 60Hz using
/// pre-computed extraction plans. This separation minimizes runtime overhead.
pub trait FrameAdapter: Sized {
    /// Validate adapter against telemetry schema at connection time.
    ///
    /// This method:
    /// - Checks that all required fields exist in the schema
    /// - Validates type compatibility between adapter and telemetry
    /// - Builds pre-computed extraction plans for runtime efficiency
    /// - Provides helpful error messages with field name suggestions
    ///
    /// # Performance
    /// This method is called once per connection, not per frame.
    /// Expensive operations like HashMap lookups and string matching are acceptable here.
    fn validate_schema(schema: &VariableSchema) -> Result<AdapterValidation>;

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
    /// Arc<[u8]> buffer. Adapters extract fields directly from packet.data.
    fn adapt(packet: &crate::FramePacket, validation: &AdapterValidation) -> Self;
}
