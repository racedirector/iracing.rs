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
    #[inline]
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
    /// Resolves a published name and validates its storage type and shape for `T`.
    ///
    /// If `required` is false, missing fields and validation errors become `Ok(None)`.
    ///
    /// # Errors
    /// Returns a parse error if a required field is missing or fails validation.
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
    #[inline]
    pub fn ensure_packet(&self, packet: &FramePacket) -> Result<()> {
        if !Arc::ptr_eq(&self.layout, packet.layout()) {
            return Err(layout_mismatch());
        }
        Ok(())
    }
    /// Decodes a zero-based slot in adapter declaration order.
    ///
    /// The requested `T` is validated against the selected field before decoding,
    /// so manual adapters cannot reinterpret a slot with a different same-sized
    /// telemetry type. Returns `Ok(None)` for absent optional/default fields and
    /// calculated or skipped slots.
    ///
    /// # Errors
    /// Returns a parse error for a different layout allocation, an out-of-range
    /// slot, or an invalid field ID. Propagates type, shape, and decoding errors.
    #[inline]
    pub fn decode<T: VarData>(&self, packet: &FramePacket, slot: usize) -> Result<Option<T>> {
        self.ensure_packet(packet)?;
        self.field_for_slot(slot)?
            .map(|field| T::decode_field(packet.data(), field))
            .transpose()
    }
    /// Decodes a slot after `T` has already been validated for that exact plan entry.
    ///
    /// This exists for generated adapters whose validation phase created the plan
    /// with the same `T`. It still checks packet layout identity and frame bounds,
    /// but intentionally skips repeated type/shape validation on the hot path.
    #[doc(hidden)]
    #[inline]
    pub fn decode_prevalidated<T: VarData>(
        &self,
        packet: &FramePacket,
        slot: usize,
    ) -> Result<Option<T>> {
        self.ensure_packet(packet)?;
        self.field_for_slot(slot)?
            .map(|field| T::decode_prevalidated(packet.data(), field))
            .transpose()
    }
    /// Returns a type default for a missing or undecodable slot.
    ///
    /// Uses the same checked slot and type requirements as [`Self::decode`]. Invalid
    /// slots, invalid field IDs, type mismatches, and decoding errors also produce
    /// `T::default()`.
    ///
    /// # Panics
    /// Panics if the packet does not retain the same layout allocation.
    #[inline]
    pub fn fetch_or_default<T: VarData + Default>(&self, packet: &FramePacket, slot: usize) -> T {
        self.ensure_packet(packet).expect("adapter layout mismatch");
        // Layout identity was checked above; decode the slot without repeating it.
        self.field_for_slot(slot)
            .ok()
            .flatten()
            .and_then(|field| T::decode_field(packet.data(), field).ok())
            .unwrap_or_default()
    }
    /// Returns a type default using a plan entry already validated for `T`.
    ///
    /// Generated adapters use this to preserve one-time type/shape validation.
    /// Invalid slots, invalid field IDs, and decoding errors produce `T::default()`.
    #[doc(hidden)]
    #[inline]
    pub fn fetch_or_default_prevalidated<T: VarData + Default>(
        &self,
        packet: &FramePacket,
        slot: usize,
    ) -> T {
        self.ensure_packet(packet).expect("adapter layout mismatch");
        self.decode_prevalidated(packet, slot)
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    #[inline]
    fn field_for_slot(&self, slot: usize) -> Result<Option<&FieldLayout>> {
        let Some(entry) = self.extraction_plan.get(slot) else {
            return Err(plan_error("Invalid plan slot"));
        };
        match entry.field_id() {
            None => Ok(None),
            Some(id) => match self.layout.field(id) {
                Some(field) => Ok(Some(field)),
                None => Err(plan_error("Invalid field ID")),
            },
        }
    }
}

/// Builds the layout-identity error away from the hot adaptation path.
#[cold]
#[inline(never)]
fn layout_mismatch() -> IRacingSDKError {
    IRacingSDKError::parse_error(
        "AdapterValidation",
        "Packet layout differs from validation layout",
    )
}

/// Builds plan-slot errors away from the hot adaptation path.
#[cold]
#[inline(never)]
fn plan_error(details: &'static str) -> IRacingSDKError {
    IRacingSDKError::parse_error("AdapterValidation", details)
}

/// Checks type/shape compatibility without probing an empty byte buffer.
///
/// Returns `None` on success and the diagnostic for a type-conversion error.
/// Propagates all other validation errors.
#[doc(hidden)]
pub fn telemetry_type_mismatch_details<T: VarData>(field: &FieldLayout) -> Result<Option<String>> {
    match T::validate_field(field) {
        Ok(()) => Ok(None),
        Err(IRacingSDKError::TypeConversion { details }) => Ok(Some(details)),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        VariableHeaders,
        irsdk::{VariableHeader, VariableType},
    };

    #[test]
    fn public_decode_revalidates_requested_type() {
        let header =
            VariableHeader::new(VariableType::Float, 0, 1, false, "Speed", "", "").unwrap();
        let headers = VariableHeaders::from(vec![header]);
        let layout = Arc::new(TelemetryLayout::try_from_headers(&headers, 4).unwrap());
        let id = AdapterValidation::resolve::<f32>(&layout, "Speed", true)
            .unwrap()
            .unwrap();
        let validation =
            AdapterValidation::new(Arc::clone(&layout), vec![FieldExtraction::Required(id)]);
        let packet = FramePacket::new(1.0f32.to_le_bytes().to_vec(), 0, 0, layout).unwrap();

        assert_eq!(validation.decode::<f32>(&packet, 0).unwrap(), Some(1.0));
        assert!(matches!(
            validation.decode::<i32>(&packet, 0),
            Err(IRacingSDKError::TypeConversion { .. })
        ));
    }
}
