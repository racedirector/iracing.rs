//! Variable data parsing trait and implementations

use super::{BitField, VariableInfo, VariableType};

/// Trait for types that can be parsed from binary telemetry data.
pub trait VarData: Sized {
    /// Parse this type from binary data at the given offset.
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self>;
}

// Implement VarData for basic types
impl VarData for f32 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if info.data_type != VariableType::Float32 {
            return Err(crate::TelemetryError::TypeConversion {
                details: format!("Expected Float32, got {:?}", info.data_type),
            });
        }

        let bytes = data
            .get(info.offset..info.offset + 4)
            .ok_or(crate::TelemetryError::Memory { offset: info.offset, source: None })?;

        Ok(f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

impl VarData for i32 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if info.data_type != VariableType::Int32 {
            return Err(crate::TelemetryError::TypeConversion {
                details: format!("Expected Int32, got {:?}", info.data_type),
            });
        }

        let bytes = data
            .get(info.offset..info.offset + 4)
            .ok_or(crate::TelemetryError::Memory { offset: info.offset, source: None })?;

        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

impl VarData for bool {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if info.data_type != VariableType::Bool {
            return Err(crate::TelemetryError::TypeConversion {
                details: format!("Expected Bool, got {:?}", info.data_type),
            });
        }

        let byte = data
            .get(info.offset)
            .ok_or(crate::TelemetryError::Memory { offset: info.offset, source: None })?;

        Ok(*byte != 0)
    }
}

impl VarData for BitField {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if info.data_type != VariableType::BitField {
            return Err(crate::TelemetryError::TypeConversion {
                details: format!("Expected BitField, got {:?}", info.data_type),
            });
        }

        let bytes = data
            .get(info.offset..info.offset + 4)
            .ok_or(crate::TelemetryError::Memory { offset: info.offset, source: None })?;

        Ok(BitField(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])))
    }
}

// Additional VarData implementations for all iRacing SDK types
impl VarData for u8 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if !matches!(info.data_type, VariableType::UInt8 | VariableType::Char) {
            return Err(crate::TelemetryError::TypeConversion {
                details: format!("Expected UInt8 or Char, got {:?}", info.data_type),
            });
        }

        let byte = data
            .get(info.offset)
            .ok_or(crate::TelemetryError::Memory { offset: info.offset, source: None })?;

        Ok(*byte)
    }
}

impl VarData for i8 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if info.data_type != VariableType::Int8 {
            return Err(crate::TelemetryError::TypeConversion {
                details: format!("Expected Int8, got {:?}", info.data_type),
            });
        }

        let byte = data
            .get(info.offset)
            .ok_or(crate::TelemetryError::Memory { offset: info.offset, source: None })?;

        Ok(*byte as i8)
    }
}

impl VarData for u16 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if info.data_type != VariableType::UInt16 {
            return Err(crate::TelemetryError::TypeConversion {
                details: format!("Expected UInt16, got {:?}", info.data_type),
            });
        }

        let bytes = data
            .get(info.offset..info.offset + 2)
            .ok_or(crate::TelemetryError::Memory { offset: info.offset, source: None })?;

        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }
}

impl VarData for i16 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if info.data_type != VariableType::Int16 {
            return Err(crate::TelemetryError::TypeConversion {
                details: format!("Expected Int16, got {:?}", info.data_type),
            });
        }

        let bytes = data
            .get(info.offset..info.offset + 2)
            .ok_or(crate::TelemetryError::Memory { offset: info.offset, source: None })?;

        Ok(i16::from_le_bytes([bytes[0], bytes[1]]))
    }
}

impl VarData for u32 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if info.data_type != VariableType::UInt32 {
            return Err(crate::TelemetryError::TypeConversion {
                details: format!("Expected UInt32, got {:?}", info.data_type),
            });
        }

        let bytes = data
            .get(info.offset..info.offset + 4)
            .ok_or(crate::TelemetryError::Memory { offset: info.offset, source: None })?;

        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

impl VarData for f64 {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if info.data_type != VariableType::Float64 {
            return Err(crate::TelemetryError::TypeConversion {
                details: format!("Expected Float64, got {:?}", info.data_type),
            });
        }

        let bytes = data
            .get(info.offset..info.offset + 8)
            .ok_or(crate::TelemetryError::Memory { offset: info.offset, source: None })?;

        Ok(f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }
}

// Array support for VarData
impl<T: VarData> VarData for Vec<T> {
    fn from_bytes(data: &[u8], info: &VariableInfo) -> crate::Result<Self> {
        if info.count == 0 {
            return Ok(Vec::new());
        }

        let element_size = info.data_type.size();
        let mut result = Vec::with_capacity(info.count);

        for i in 0..info.count {
            let element_offset = info.offset + (i * element_size);
            let element_info = VariableInfo {
                name: info.name.clone(),
                data_type: info.data_type,
                offset: element_offset,
                count: 1,
                count_as_time: info.count_as_time,
                units: info.units.clone(),
                description: info.description.clone(),
            };

            let element = T::from_bytes(data, &element_info)?;
            result.push(element);
        }

        Ok(result)
    }
}
