//! Declaration macros shared by the IRSDK type modules.

macro_rules! sdk_enum {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($variant:ident = $value:expr,)+
        }
    ) => {
        $(#[$meta])*
        #[repr(i32)]
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            serde::Serialize,
            serde::Deserialize,
        )]
        $vis enum $name {
            $(
                #[doc = concat!("SDK member `", stringify!($variant), "`.")]
                $variant = $value,
            )+
        }

        impl TryFrom<i32> for $name {
            type Error = i32;

            fn try_from(value: i32) -> Result<Self, Self::Error> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    raw => Err(raw),
                }
            }
        }

        impl From<$name> for i32 {
            fn from(value: $name) -> Self {
                value as i32
            }
        }

        #[cfg(feature = "codegen")]
        impl $name {
            /// Named variants and their SDK values for schema generation.
            pub const SCHEMA_VALUES: &'static [(&'static str, i64)] = &[
                $((stringify!($variant), $value as i64),)+
            ];
        }

        #[cfg(feature = "codegen")]
        impl schemars::JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($name).into()
            }

            fn schema_id() -> std::borrow::Cow<'static, str> {
                concat!(module_path!(), "::", stringify!($name)).into()
            }

            fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
                #[allow(dead_code, clippy::enum_variant_names)]
                #[derive(schemars::JsonSchema)]
                $(#[$meta])*
                enum SchemaRepresentation {
                    $($variant,)+
                }

                let mut schema = SchemaRepresentation::json_schema(generator);
                let schema_object = schema.ensure_object();
                schema_object.insert("x-irsdk-kind".into(), "enum".into());
                schema_object.insert(
                    "x-irsdk-values".into(),
                    $crate::types::codegen::named_schema_values(Self::SCHEMA_VALUES),
                );
                schema
            }
        }
    };
}

macro_rules! sdk_bitmask {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $($flag:ident = $value:expr,)+
        }
    ) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(
            Debug,
            Default,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            serde::Serialize,
            serde::Deserialize,
        )]
        $vis struct $name(u32);

        impl $name {
            $(
                #[doc = concat!("SDK mask `", stringify!($flag), "`.")]
                pub const $flag: Self = Self($value);
            )+

            /// Returns the value with no bits set.
            pub const fn empty() -> Self {
                Self(0)
            }

            /// Constructs the SDK value without discarding unknown bits.
            pub const fn from_bits(bits: u32) -> Self {
                Self(bits)
            }

            /// Constructs the SDK value while retaining every supplied bit.
            pub const fn from_bits_retain(bits: u32) -> Self {
                Self(bits)
            }

            /// Returns the complete underlying SDK bit pattern.
            pub const fn bits(self) -> u32 {
                self.0
            }

            /// Returns whether all bits in `other` are set.
            pub const fn contains(self, other: Self) -> bool {
                self.0 & other.0 == other.0
            }

            /// Returns whether any bits in `other` are set.
            pub const fn intersects(self, other: Self) -> bool {
                self.0 & other.0 != 0
            }

            /// Returns the union of this value and `other`.
            pub const fn union(self, other: Self) -> Self {
                Self(self.0 | other.0)
            }

            /// All named masks defined for this SDK bitmask family.
            pub const DEFINITIONS: &'static [(Self, &'static str)] = &[
                $((Self::$flag, stringify!($flag)),)+
            ];

            /// Returns the names of all defined masks that intersect this value.
            pub fn names(self) -> Vec<&'static str> {
                Self::DEFINITIONS
                    .iter()
                    .filter_map(|(flag, name)| self.intersects(*flag).then_some(*name))
                    .collect()
            }
        }

        #[cfg(feature = "codegen")]
        impl $name {
            /// Named masks and their values for schema generation.
            pub const SCHEMA_VALUES: &'static [(&'static str, i64)] = &[
                $((stringify!($flag), $value as i64),)+
            ];

            /// Mask containing every bit named by this SDK definition.
            pub const SCHEMA_KNOWN_MASK: u32 = 0u32 $(| ($value as u32))+;
        }

        #[cfg(feature = "codegen")]
        impl schemars::JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($name).into()
            }

            fn schema_id() -> std::borrow::Cow<'static, str> {
                concat!(module_path!(), "::", stringify!($name)).into()
            }

            fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
                #[allow(dead_code)]
                #[derive(schemars::JsonSchema)]
                $(#[$meta])*
                struct SchemaRepresentation(u32);

                let mut schema = SchemaRepresentation::json_schema(generator);
                let schema_object = schema.ensure_object();
                schema_object.insert("x-irsdk-kind".into(), "bitflags".into());
                schema_object.insert(
                    "x-irsdk-values".into(),
                    $crate::types::codegen::named_schema_values(Self::SCHEMA_VALUES),
                );
                schema_object.insert(
                    "x-irsdk-known-mask".into(),
                    (Self::SCHEMA_KNOWN_MASK as u64).into(),
                );
                schema
            }
        }

        impl From<u32> for $name {
            fn from(value: u32) -> Self {
                Self::from_bits_retain(value)
            }
        }

        impl From<$name> for u32 {
            fn from(value: $name) -> Self {
                value.bits()
            }
        }

        impl From<$crate::BitField> for $name {
            fn from(value: $crate::BitField) -> Self {
                Self::from_bits_retain(value.value())
            }
        }

        impl From<$name> for $crate::BitField {
            fn from(value: $name) -> Self {
                Self::new(value.bits())
            }
        }

        impl $crate::VarData for $name {
            fn from_bytes(
                data: &[u8],
                info: &$crate::VariableInfo,
            ) -> $crate::Result<Self> {
                if info.data_type != $crate::VariableType::BitField {
                    return Err($crate::IRacingSDKError::type_conversion(
                        "BitField",
                        info.data_type,
                    ));
                }

                <$crate::BitField as $crate::VarData>::from_bytes(data, info).map(Self::from)
            }
        }

        impl std::ops::BitOr for $name {
            type Output = Self;

            fn bitor(self, rhs: Self) -> Self::Output {
                Self(self.0 | rhs.0)
            }
        }

        impl std::ops::BitOrAssign for $name {
            fn bitor_assign(&mut self, rhs: Self) {
                self.0 |= rhs.0;
            }
        }

        impl std::ops::BitAnd for $name {
            type Output = Self;

            fn bitand(self, rhs: Self) -> Self::Output {
                Self(self.0 & rhs.0)
            }
        }

        impl std::ops::BitAndAssign for $name {
            fn bitand_assign(&mut self, rhs: Self) {
                self.0 &= rhs.0;
            }
        }
    };
}

pub(crate) use sdk_bitmask;
pub(crate) use sdk_enum;

#[cfg(test)]
mod tests {
    use std::mem::{align_of, size_of};

    use crate::{BitField, IRacingSDKError, VarData, VariableInfo, VariableType};

    sdk_enum! {
        /// Enum used to exercise the code generated by `sdk_enum!`.
        enum TestEnum {
            Negative = -2,
            Zero = 0,
            Positive = 7,
        }
    }

    sdk_bitmask! {
        /// Bitmask used to exercise the code generated by `sdk_bitmask!`.
        struct TestBitmask {
            FIRST = 0x0000_0001,
            SECOND = 0x0000_0004,
        }
    }

    fn variable_info(data_type: VariableType, offset: usize) -> VariableInfo {
        VariableInfo {
            name: "TestBitmask".to_owned(),
            data_type,
            offset,
            count: 1,
            count_as_time: false,
            units: String::new(),
            description: String::new(),
        }
    }

    #[test]
    fn enum_uses_i32_representation_and_round_trips_known_values() {
        assert_eq!(size_of::<TestEnum>(), size_of::<i32>());
        assert_eq!(align_of::<TestEnum>(), align_of::<i32>());

        for (raw, value) in [
            (-2, TestEnum::Negative),
            (0, TestEnum::Zero),
            (7, TestEnum::Positive),
        ] {
            assert_eq!(TestEnum::try_from(raw), Ok(value));
            assert_eq!(i32::from(value), raw);
        }
    }

    #[test]
    fn enum_rejects_unknown_values_without_losing_the_raw_value() {
        assert_eq!(TestEnum::try_from(-1), Err(-1));
        assert_eq!(TestEnum::try_from(i32::MAX), Err(i32::MAX));
    }

    #[test]
    fn bitmask_constructors_and_queries_preserve_unknown_bits() {
        const UNKNOWN: u32 = 0x8000_0000;

        let mask = TestBitmask::from_bits(TestBitmask::FIRST.bits() | UNKNOWN);
        assert_eq!(TestBitmask::empty().bits(), 0);
        assert_eq!(TestBitmask::from_bits_retain(mask.bits()), mask);
        assert_eq!(mask.bits(), TestBitmask::FIRST.bits() | UNKNOWN);
        assert!(mask.contains(TestBitmask::FIRST));
        assert!(mask.contains(TestBitmask::empty()));
        assert!(!mask.contains(TestBitmask::SECOND));
        assert!(mask.intersects(TestBitmask::FIRST));
        assert!(!mask.intersects(TestBitmask::SECOND));
    }

    #[test]
    fn bitmask_definitions_and_names_follow_declaration_order() {
        assert_eq!(
            TestBitmask::DEFINITIONS,
            &[
                (TestBitmask::FIRST, "FIRST"),
                (TestBitmask::SECOND, "SECOND"),
            ]
        );
        assert_eq!(
            TestBitmask::FIRST.union(TestBitmask::SECOND).names(),
            vec!["FIRST", "SECOND"]
        );
        assert!(TestBitmask::from_bits(0x8000_0000).names().is_empty());
    }

    #[test]
    fn bitmask_operators_and_numeric_conversions_are_consistent() {
        let mut union = TestBitmask::FIRST | TestBitmask::SECOND;
        assert_eq!(union, TestBitmask::FIRST.union(TestBitmask::SECOND));

        union &= TestBitmask::SECOND;
        assert_eq!(union, TestBitmask::SECOND);
        union |= TestBitmask::FIRST;
        assert_eq!(union & TestBitmask::FIRST, TestBitmask::FIRST);

        let raw = union.bits() | 0x8000_0000;
        let from_raw = TestBitmask::from(raw);
        assert_eq!(u32::from(from_raw), raw);

        let bitfield = BitField::from(from_raw);
        assert_eq!(bitfield.value(), raw);
        assert_eq!(TestBitmask::from(bitfield), from_raw);
    }

    #[test]
    fn bitmask_serde_round_trip_keeps_the_complete_bit_pattern() {
        let mask = TestBitmask::from_bits(TestBitmask::SECOND.bits() | 0x8000_0000);
        let json = serde_json::to_string(&mask).expect("serialize test bitmask");
        assert_eq!(json, mask.bits().to_string());
        assert_eq!(
            serde_json::from_str::<TestBitmask>(&json).expect("deserialize test bitmask"),
            mask
        );
    }

    #[test]
    fn bitmask_var_data_decodes_little_endian_at_the_schema_offset() {
        let raw = TestBitmask::FIRST.bits() | 0x8000_0000;
        let mut data = vec![0xaa, 0xbb];
        data.extend_from_slice(&raw.to_le_bytes());

        let decoded = TestBitmask::from_bytes(&data, &variable_info(VariableType::BitField, 2))
            .expect("decode test bitmask");
        assert_eq!(decoded.bits(), raw);
    }

    #[test]
    fn bitmask_var_data_rejects_a_non_bitfield_schema() {
        let error = TestBitmask::from_bytes(
            &TestBitmask::FIRST.bits().to_le_bytes(),
            &variable_info(VariableType::Integer, 0),
        )
        .expect_err("integer schema must not decode as a bitmask");

        assert!(matches!(error, IRacingSDKError::TypeConversion { .. }));
        assert!(error.to_string().contains("BitField"));
        assert!(error.to_string().contains("Integer"));
    }

    #[cfg(feature = "codegen")]
    #[test]
    fn bitmask_schema_exposes_all_macro_metadata() {
        use serde_json::Value;

        assert_eq!(TestBitmask::SCHEMA_VALUES, &[("FIRST", 1), ("SECOND", 4)]);
        assert_eq!(TestBitmask::SCHEMA_KNOWN_MASK, 5);

        let schema = schemars::schema_for!(TestBitmask);
        let object = schema
            .as_value()
            .as_object()
            .expect("bitmask schema should be an object");
        assert_eq!(
            object.get("x-irsdk-kind").and_then(Value::as_str),
            Some("bitflags")
        );
        assert_eq!(
            object.get("x-irsdk-known-mask").and_then(Value::as_u64),
            Some(5)
        );

        let values = object
            .get("x-irsdk-values")
            .and_then(Value::as_array)
            .expect("schema values should be an array");
        assert_eq!(values.len(), 2);
        assert_eq!(values[0]["name"], "FIRST");
        assert_eq!(values[0]["value"], 1);
        assert_eq!(values[1]["name"], "SECOND");
        assert_eq!(values[1]["value"], 4);
    }
}
