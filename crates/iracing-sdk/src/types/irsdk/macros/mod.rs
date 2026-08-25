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
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
