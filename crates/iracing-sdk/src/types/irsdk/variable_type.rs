//! Exact Rust representation of `irsdk_VarType`.

/// Variable kinds advertised by an iRacing SDK variable header.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VariableType {
    /// `irsdk_char`.
    Character = 0,
    /// `irsdk_bool`.
    Boolean = 1,
    /// `irsdk_int`.
    Integer = 2,
    /// `irsdk_bitField`.
    BitField = 3,
    /// `irsdk_float`.
    Float = 4,
    /// `irsdk_double`.
    Double = 5,
    /// `irsdk_ETCount` is an array bound, not a variable kind.
    ElementTypeCount = 6,
}

impl VariableType {
    /// Exact contents of `irsdk_VarTypeBytes`.
    pub const BYTE_SIZES: [usize; Self::ElementTypeCount as usize] = [1, 1, 4, 4, 4, 8];

    /// Returns the SDK byte width, excluding the `irsdk_ETCount` sentinel.
    pub const fn byte_size(self) -> Option<usize> {
        match self {
            Self::Character => Some(1),
            Self::Boolean => Some(1),
            Self::Integer | Self::BitField | Self::Float => Some(4),
            Self::Double => Some(8),
            Self::ElementTypeCount => None,
        }
    }
}

impl TryFrom<i32> for VariableType {
    type Error = i32;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Character),
            1 => Ok(Self::Boolean),
            2 => Ok(Self::Integer),
            3 => Ok(Self::BitField),
            4 => Ok(Self::Float),
            5 => Ok(Self::Double),
            6 => Ok(Self::ElementTypeCount),
            raw => Err(raw),
        }
    }
}

impl From<VariableType> for i32 {
    fn from(value: VariableType) -> Self {
        value as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_and_sizes_match_the_sdk() {
        assert_eq!(i32::from(VariableType::Character), 0);
        assert_eq!(i32::from(VariableType::ElementTypeCount), 6);
        assert_eq!(VariableType::BYTE_SIZES, [1, 1, 4, 4, 4, 8]);
        assert_eq!(VariableType::Double.byte_size(), Some(8));
        assert_eq!(VariableType::ElementTypeCount.byte_size(), None);
    }
}
