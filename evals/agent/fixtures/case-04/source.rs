// Both mappings are opened FILE_MAP_READ in this process.
// A: completed disk file; provider controls storage and prevents mutation/truncation.
// B: simulator shared memory; the simulator updates buffers while readers run.
// Proposed common method:
pub unsafe fn as_bytes<'a>(pointer: *const u8, length: usize) -> &'a [u8] {
    unsafe { std::slice::from_raw_parts(pointer, length) }
}
