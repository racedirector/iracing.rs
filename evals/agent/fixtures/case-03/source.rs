// This represents a dependency-free sketch of a wire decoder.
pub struct Header { count: i32, offset: i32, length: i32 }
pub fn decode(bytes: [u8; 12]) -> Header {
    Header { count: i32::from_le_bytes(bytes[0..4].try_into().unwrap()),
        offset: i32::from_le_bytes(bytes[4..8].try_into().unwrap()),
        length: i32::from_le_bytes(bytes[8..12].try_into().unwrap()) }
}
pub fn region(h: Header) -> std::ops::Range<usize> {
    (h.offset as usize)..((h.offset + h.length) as usize)
}

#[repr(i32)]
pub enum VariableType { Float = 4, Double = 5 }
pub fn variable_type(raw: i32) -> VariableType {
    unsafe { std::mem::transmute(raw) }
}
