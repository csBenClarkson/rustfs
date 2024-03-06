const NAME_LEN: usize = 255;
struct Dirent {
    inode: u32,
    entry_length: u16,
    name_length: u8,
    file_type: u8,
    name: [u8; NAME_LEN],
}