pub struct Stable { pointer: *const u8, length: usize }
impl Stable {
    pub fn new(bytes: &[u8]) -> Self { Self { pointer: bytes.as_ptr(), length: bytes.len() } }
    pub fn bytes(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.pointer, self.length) }
    }
}
// Caller guidance: retain the original storage while Stable is in use.
