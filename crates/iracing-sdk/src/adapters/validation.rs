//! Layout-bound positional adapter plans.
use crate::{FieldId, FieldLayout, FramePacket, IRacingSDKError, Result, TelemetryLayout, VarData};
use std::sync::Arc;

/// Extraction strategy in adapter declaration order. No names or geometry are copied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldExtraction {
    /// Required, validated telemetry field.
    Required(FieldId),
    /// Optional telemetry field, absent when missing or incompatible.
    Optional(Option<FieldId>),
    /// Field with a generated fallback expression.
    WithDefault(Option<FieldId>),
    /// Computed by generated Rust code.
    Calculated,
    /// Managed by application code.
    Skipped,
}
impl FieldExtraction {
    /// Returns the selected telemetry ID, when present.
    pub fn field_id(&self) -> Option<FieldId> {
        match self {
            Self::Required(id) => Some(*id),
            Self::Optional(id) | Self::WithDefault(id) => *id,
            _ => None,
        }
    }
    /// Whether this entry requires a telemetry value.
    pub fn is_required(&self) -> bool {
        matches!(self, Self::Required(_))
    }
}

/// Validated extraction slots bound to one shared layout's identity.
#[derive(Debug, Clone)]
pub struct AdapterValidation {
    layout: Arc<TelemetryLayout>,
    extraction_plan: Box<[FieldExtraction]>,
}
impl AdapterValidation {
    /// Retains the validated layout and declaration-ordered extraction plan.
    ///
    /// IDs must originate from this layout. Use `resolve` to validate type/shape.
    pub fn new(layout: Arc<TelemetryLayout>, extraction_plan: Vec<FieldExtraction>) -> Self {
        Self {
            layout,
            extraction_plan: extraction_plan.into_boxed_slice(),
        }
    }
    /// Resolves and validates a field once, with diagnostics at validation time.
    pub fn resolve<T: VarData>(
        layout: &TelemetryLayout,
        name: &str,
        required: bool,
    ) -> Result<Option<FieldId>> {
        match layout.field_by_name(name) {
            Some((id, field)) => match T::validate_field(field) {
                Ok(()) => Ok(Some(id)),
                Err(_) if !required => Ok(None),
                Err(error) => Err(IRacingSDKError::parse_error(
                    "Frame adapter validation",
                    format!("Field '{name}' has incompatible telemetry type or shape: {error}"),
                )),
            },
            None if !required => Ok(None),
            None => Err(IRacingSDKError::parse_error(
                "Frame adapter validation",
                format!(
                    "Critical field '{name}' is missing. Available fields: {}",
                    layout
                        .fields()
                        .map(|(_, f)| f.name())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            )),
        }
    }
    /// Returns the retained layout.
    pub fn layout(&self) -> &Arc<TelemetryLayout> {
        &self.layout
    }
    /// Returns immutable plan slots for diagnostics and testing.
    pub fn extraction_plan(&self) -> &[FieldExtraction] {
        &self.extraction_plan
    }
    /// Number of adapter slots, including calculated/skipped fields.
    pub fn field_count(&self) -> usize {
        self.extraction_plan.len()
    }
    /// Whether a required slot exists.
    pub fn has_required_fields(&self) -> bool {
        self.extraction_plan
            .iter()
            .any(FieldExtraction::is_required)
    }
    /// Rejects use with a different layout, even when its field geometry matches.
    pub fn ensure_packet(&self, packet: &FramePacket) -> Result<()> {
        if !Arc::ptr_eq(&self.layout, packet.layout()) {
            return Err(IRacingSDKError::parse_error(
                "AdapterValidation",
                "Packet layout differs from validation layout",
            ));
        }
        Ok(())
    }
    /// Decodes a declaration-ordered slot without name lookup or geometry recomputation.
    pub fn decode<T: VarData>(&self, packet: &FramePacket, slot: usize) -> Result<Option<T>> {
        self.ensure_packet(packet)?;
        let entry = self.extraction_plan.get(slot).ok_or_else(|| {
            IRacingSDKError::parse_error("AdapterValidation", "Invalid plan slot")
        })?;
        entry
            .field_id()
            .map(|id| {
                let field = self.layout.field(id).ok_or_else(|| {
                    IRacingSDKError::parse_error("AdapterValidation", "Invalid field ID")
                })?;
                T::decode_prevalidated(packet.data(), field)
            })
            .transpose()
    }
    /// Returns a type default for a missing or undecodable slot.
    pub fn fetch_or_default<T: VarData + Default>(&self, packet: &FramePacket, slot: usize) -> T {
        self.ensure_packet(packet).expect("adapter layout mismatch");
        self.decode(packet, slot).ok().flatten().unwrap_or_default()
    }
}

/// Checks type/shape compatibility without probing an empty byte buffer.
#[doc(hidden)]
pub fn telemetry_type_mismatch_details<T: VarData>(field: &FieldLayout) -> Result<Option<String>> {
    match T::validate_field(field) {
        Ok(()) => Ok(None),
        Err(IRacingSDKError::TypeConversion { details }) => Ok(Some(details)),
        Err(err) => Err(err),
    }
}
