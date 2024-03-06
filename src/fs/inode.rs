const N_DIR_BLOCKS: usize = 10;
const INDIR_BLOCK: usize = N_DIR_BLOCKS;
const N_BLOCKS: usize = INDIR_BLOCK + 1;

const IFLNK: u16 = 0xA000;
const IFREG: u16 = 0x8000;
const IFDIR: u16 = 0x4000;

#[allow(unused)]
macro_rules! is_regular_file {
    ($mode: expr) => { (((mode) & IFREG) != 0) as bool };
}

#[allow(unused)]
macro_rules! is_directory {
    ($mode: expr) => { (((mode) & IFDIR) != 0) as bool };
}

#[allow(unused)]
macro_rules! is_symbolic_link {
    ($mode: expr) => { (((mode) & IFLNK) != 0) as bool };
}
pub struct Inode {
    i_mode:        u16,
    i_size:        u64,
    i_atime:       u64,
    i_ctime:       u64,
    i_mtime:       u64,
    i_links_count: u16,
    i_blocks:      u16,
    i_flags:       u32,
    i_block:       [u16; N_BLOCKS],
    // 64 bytes
}


impl Inode {
    pub fn new_dir(time: u64, flags: u32, first_block: u16) -> Inode {
        let mut blocks = [0; N_BLOCKS];
        blocks[0] = first_block;
        Inode {
            i_mode: IFDIR,
            i_size: 0,
            i_atime: time,
            i_ctime: time,
            i_mtime: time,
            i_links_count: 1,
            i_blocks: 1,
            i_flags: flags,
            i_block: blocks.clone()
        }
    }
}