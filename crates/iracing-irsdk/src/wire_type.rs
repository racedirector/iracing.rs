//! Owned decoding for fixed-size structures from the iRacing SDK wire format.
//!
//! The SDK structs use native `#[repr(C)]` field representations. Their wire
//! bytes are little-endian, so these copies have the intended field values on
//! little-endian targets only. Semantic validation is a separate step.

use std::io::{Read, Write};

use zerocopy::{FromBytes, IntoBytes};

use crate::{DiskSubHeader, Error, Header, Result, VariableBuffer, VariableHeader};

// Only the four audited SDK layouts implement this trait. Existing callers use
// WIRE_SIZE for offsets and buffer sizing, so retain that derived-size API.
pub(crate) mod sealed {
    pub trait Sealed {}

    impl Sealed for super::Header {}
    impl Sealed for super::DiskSubHeader {}
    impl Sealed for super::VariableBuffer {}
    impl Sealed for super::VariableHeader {}
}

/// A fixed-size SDK structure with derive-checked byte validity and layout.
///
/// Decoding and encoding copy the native representation without byte swapping.
/// Use this API only on little-endian targets. Call the type's validation method
/// separately when the decoded field values must satisfy SDK rules.
pub trait WireType: sealed::Sealed + Copy {
    /// Exact size of the fixed wire representation, derived from the Rust layout.
    const WIRE_SIZE: usize;

    /// Copies one complete, potentially unaligned wire value from `bytes`.
    ///
    /// # Errors
    ///
    /// Returns [`Error::WireSize`] unless the input has exactly [`Self::WIRE_SIZE`] bytes.
    fn read_from_bytes(bytes: &[u8]) -> Result<Self>;

    /// Reads one complete value from a reader, preserving its I/O error in a parse error.
    fn read_from_reader<R: Read>(reader: &mut R, name: &'static str) -> Result<Self>;

    /// Writes this value's exact wire representation to `writer`.
    ///
    /// No byte-order conversion is performed.
    fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()>;
}

macro_rules! impl_wire_type {
    ($($type:ty),+ $(,)?) => {
        $(
            impl WireType for $type {
                const WIRE_SIZE: usize = std::mem::size_of::<Self>();

                fn read_from_bytes(bytes: &[u8]) -> Result<Self> {
                    <Self as FromBytes>::read_from_bytes(bytes).map_err(|_| Error::WireSize {
                        expected: Self::WIRE_SIZE,
                        actual: bytes.len(),
                    })
                }

                fn read_from_reader<R: Read>(reader: &mut R, name: &'static str) -> Result<Self> {
                    <Self as FromBytes>::read_from_io(reader).map_err(|error| {
                        Error::parse(
                            format!("{name} reading"),
                            format!("Failed to read {} {name} bytes: {error}", Self::WIRE_SIZE),
                        )
                    })
                }

                fn write_to<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
                    writer.write_all(self.as_bytes())
                }
            }
        )+
    };
}

impl_wire_type!(Header, DiskSubHeader, VariableBuffer, VariableHeader);

#[cfg(test)]
mod tests {
    use super::WireType;
    use crate::{DiskSubHeader, Error, Header, VariableBuffer, VariableHeader};

    #[test]
    fn arbitrary_byte_pattern_round_trips_without_semantic_validation() {
        fn check<T: WireType>() {
            let bytes = vec![0xff; T::WIRE_SIZE];
            let value = <T as WireType>::read_from_bytes(&bytes).unwrap();
            let mut written = Vec::new();
            WireType::write_to(&value, &mut written).unwrap();
            assert_eq!(written, bytes);
        }

        check::<Header>();
        check::<DiskSubHeader>();
        check::<VariableBuffer>();
        check::<VariableHeader>();

        let invalid =
            <VariableHeader as WireType>::read_from_bytes(&[0xff; VariableHeader::WIRE_SIZE])
                .unwrap();
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn decode_accepts_unaligned_input_and_rejects_wrong_sizes() {
        let bytes = [0; Header::WIRE_SIZE + 2];
        assert!(<Header as WireType>::read_from_bytes(&bytes[1..1 + Header::WIRE_SIZE]).is_ok());

        for actual in [Header::WIRE_SIZE - 1, Header::WIRE_SIZE + 1] {
            assert!(matches!(
                <Header as WireType>::read_from_bytes(&bytes[..actual]),
                Err(Error::WireSize { expected, actual: found })
                    if expected == Header::WIRE_SIZE && found == actual
            ));
        }
    }

    #[test]
    fn reader_consumes_exactly_one_value() {
        let bytes = vec![0; DiskSubHeader::WIRE_SIZE + 1];
        let mut input = bytes.as_slice();
        DiskSubHeader::try_from_reader(&mut input).unwrap();
        assert_eq!(input, &[0]);
    }
}
