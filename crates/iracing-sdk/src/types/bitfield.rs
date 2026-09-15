//! BitField type for handling iRacing bitfield variables

use serde::{Deserialize, Serialize};

/// BitField type for handling iRacing bitfield variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BitField(pub u32);

impl BitField {
    /// Create a new BitField from a u32 value.
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    /// Check if a specific bit is set.
    pub fn is_set(&self, bit: u32) -> bool {
        (self.0 & (1 << bit)) != 0
    }

    /// Check if a specific flag is set using a bitmask.
    pub fn has_flag(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    /// Get the raw u32 value.
    pub fn value(&self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitfield_constructor_works() {
        let bitfield = BitField::new(0x12345678);
        assert_eq!(bitfield.value(), 0x12345678);
    }

    #[test]
    fn bitfield_flag_operations_basic() {
        let bitfield = BitField::new(0b1010);
        assert!(bitfield.is_set(1));
        assert!(!bitfield.is_set(0));
        assert!(bitfield.is_set(3));
        assert!(!bitfield.is_set(2));
        assert!(bitfield.has_flag(0b0010));
        assert!(!bitfield.has_flag(0b0001));
        assert!(bitfield.has_flag(0b1000));
        assert!(!bitfield.has_flag(0b0100));
    }
}
