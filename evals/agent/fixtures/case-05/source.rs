trait Source {
    fn read_into(&self, destination: &mut [u8]);
    fn read(&self, n: usize) -> Vec<u8> { let mut v=vec![0;n]; self.read_into(&mut v);v }
}
struct Owned(Vec<u8>);
impl Source for Owned { fn read_into(&self, dst: &mut [u8]) { dst.copy_from_slice(&self.0[..dst.len()]); } }
fn deliver(source: &dyn Source, reused: &mut [u8]) { source.read_into(reused); }
