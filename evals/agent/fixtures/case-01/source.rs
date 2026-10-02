use std::ptr::NonNull;
pub struct View { base: NonNull<u8>, length: usize }
// An external simulator continuously writes this mapping.
impl View {
    pub unsafe fn from_mapping(base: NonNull<u8>, length: usize) -> Self { Self { base, length } }
    pub fn copy(&self, destination: &mut [u8]) {
        assert!(destination.len() <= self.length);
        for (i, byte) in destination.iter_mut().enumerate() {
            *byte = unsafe { self.base.as_ptr().add(i).read_volatile() };
        }
    }
}
unsafe impl Send for View {}
unsafe impl Sync for View {}
