pub unsafe trait WireType: Copy + Sized {
    const WIRE_SIZE: usize = std::mem::size_of::<Self>();

    // fn read_from_bytes(bytes: &[u8]) -> Result<Self, WireSizeError>;
    // fn read_from_prefix(bytes: &[u8]) -> Result<Self, WireSizeError>;
}
