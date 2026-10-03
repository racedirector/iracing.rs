//! Type-safe adapters validate once against a shared layout and decode positional slots.
mod frame_adapter;
mod validation;
pub use frame_adapter::FrameAdapter;
pub use validation::{
    AdapterValidation, FieldExtraction, ValidatedFrame, telemetry_type_mismatch_details,
};
