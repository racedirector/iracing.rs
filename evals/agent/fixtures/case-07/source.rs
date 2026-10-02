struct Disk { frames: Vec<Vec<u8>> }
impl Disk { fn frame(&self, index: usize) -> Option<&[u8]> { self.frames.get(index).map(Vec::as_slice) } }
// Owned immutable disk frames. No live connection or state transitions exist.
// Proposed replacement: Reader<Unknown> -> Reader<Validated> -> Reader<Streaming>.
