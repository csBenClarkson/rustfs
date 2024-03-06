pub trait BlockDev {
    fn bread(&self, buf: &mut [u8], bid: usize);
    fn bwrite(&mut self, buf: &[u8], bid: usize);
}
