//! Raw decoding for fixed-size structures from the iRacing SDK wire format.
//!
//! [`WireType`] is intended for `Copy` types whose Rust memory layout exactly
//! matches a structure defined by the iRacing C SDK. It performs an unaligned
//! copy from a byte slice; it does not deserialize individual fields or perform
//! semantic validation.

use crate::{IRacingSDKError, Result};

/// A fixed-size type that can be copied directly from its iRacing wire representation.
///
/// The SDK wire format is little-endian. Because [`read_from_bytes`](Self::read_from_bytes)
/// copies the representation without byte swapping, this trait is suitable only on
/// little-endian targets.
///
/// # Safety
///
/// Implementing this trait asserts all of the following:
///
/// - `Self` has a stable layout that exactly matches the corresponding SDK structure,
///   including field offsets and padding. SDK structures should normally use `#[repr(C)]`.
/// - [`WIRE_SIZE`](Self::WIRE_SIZE) equals `size_of::<Self>()`.
/// - Every possible sequence of `WIRE_SIZE` bytes is a valid value of `Self`. In
///   particular, the type must not contain references, pointers with validity
///   requirements, `bool`, `char`, or enums with invalid discriminants.
/// - Interpreting the SDK's little-endian bytes as the target's native representation
///   produces the intended field values.
///
/// Violating these requirements can make the safe
/// [`read_from_bytes`](Self::read_from_bytes) method cause undefined behavior.
pub unsafe trait WireType: Copy + Sized {
    /// The exact size, in bytes, of this type's wire representation.
    ///
    /// Implementations should use the default value. An override must remain equal to
    /// `size_of::<Self>()` as required by the trait's safety contract.
    const WIRE_SIZE: usize = std::mem::size_of::<Self>();

    /// Copies a value from its fixed-size wire representation.
    ///
    /// The input may be unaligned. Bytes are copied as the target's native
    /// representation without byte-order conversion or semantic validation.
    ///
    /// # Errors
    ///
    /// Returns [`IRacingSDKError::WireSize`] when `bytes.len()` is not exactly
    /// [`WIRE_SIZE`](Self::WIRE_SIZE).
    fn read_from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != Self::WIRE_SIZE {
            return Err(IRacingSDKError::WireSize {
                expected: Self::WIRE_SIZE,
                actual: bytes.len(),
            }
            .into());
        }

        Ok(unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast::<Self>()) })
    }
}
