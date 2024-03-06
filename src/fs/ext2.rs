use std::mem;
use crate::fs::bio::BlockDev;
use crate::fs::inode::Inode;
use mem::size_of;
use mem::transmute;
use std::slice;
use std::time::{ SystemTime, UNIX_EPOCH };
use crate::fs::ext2::Error::FormatError;

use anyhow::Result;
use thiserror::Error;

const BLOCK_SZ: usize = 1024;   // 1 KB
const MAX_FILE_COUNT: usize = 1024;
const SUPER_BLOCK: usize = 0;
const SUPER_BLOCK_NUM: usize = 1;
const FREE_BITMAP_BLOCK: usize = SUPER_BLOCK + SUPER_BLOCK_NUM;
const FREE_BITMAP_BLOCK_SZ: usize = 1;
const INODE_BITMAP_BLOCK: usize = FREE_BITMAP_BLOCK + FREE_BITMAP_BLOCK_SZ;
const INODE_BITMAP_BLOCK_NUM: usize = 1;
const INODE_TABLE_BLOCKS: usize = INODE_BITMAP_BLOCK + INODE_BITMAP_BLOCK_NUM;
const INODE_TABLE_BLOCKS_SZ: usize = 60;
const DATA_BLOCKS: usize = INODE_TABLE_BLOCKS + INODE_TABLE_BLOCKS_SZ;
const META_BLOCKS_SZ: usize = DATA_BLOCKS;

#[allow(unused)]
macro_rules! word_set_at {
    ($word: expr, $index: expr) => { ($word) |= (1u64 << (63 - ($index))) };
}

#[allow(unused)]
macro_rules! word_clear_at {
    ($word: expr, $index: expr) => { ($word) &= ~(1u64 << (63 - ($index))) };
}

#[allow(unused)]
macro_rules! one_block_from {
    ($bid: expr) => { ($bid) * BLOCK_SZ .. ($bid + 1) * BLOCK_SZ };
}
struct SuperBlk {
    s_inodes_count:      u16,   /* Number of inodes in the image */
    s_blocks_count:      u16,   /* Number of blocks available in the image */
    s_free_blocks_count: u16,   /* Number of free blocks */
    s_free_inodes_count: u16,   /* Number of unused inodes */
    s_first_data_block:  u16,   /* Block ID of the first data block */
    s_block_size:        u16,   /* Size of one block */
    s_last_allocate:     u16,   /* Block ID of last allocated block + 1 */
    s_magic:             u16    /* Magic signature number */
}

#[derive(Error, Debug)]
enum Error {
    #[error("Error when formatting: code: {0}")]
    FormatError(usize),
}



struct Ext2Fs {
    image: Box<[u8]>,
}

impl BlockDev for Ext2Fs {
    fn bread(&self, buf: &mut [u8], bid: usize) {
        buf.copy_from_slice(&self.image[one_block_from!(bid)]);
    }

    fn bwrite(&mut self, buf: &[u8], bid: usize) {
        self.image[one_block_from!(bid)].copy_from_slice(buf);
    }
}

unsafe fn any_as_u8_slice<T: Sized>(p: &T) -> &[u8] {
    slice::from_raw_parts((p as *const T) as *const u8, size_of::<T>())
}

impl Ext2Fs {
    pub fn new(image: Box<[u8]>) -> Ext2Fs {
        Ext2Fs{ image }
    }

    /// Find the first 0 bit in bitmap block specified by bitmap_bid and set it to 1.
    /// Return bit offset from 0th bit if found, or None if not found.
    fn first_match(&mut self, bitmap_bid: usize) -> Option<usize> {
        let mut buf = [0u8; BLOCK_SZ];
        self.bread(&mut buf, bitmap_bid);
        let block_ref: &mut [u64; BLOCK_SZ / 8] = unsafe { transmute(&mut buf) };
        // find a word that is not all 1, and find the position of first 0 bit in the word.
        if let Some((word_idx, mut word)) = block_ref.iter_mut().enumerate().find(|(_, &mut x)| x != u64::MAX) {
            word_set_at!(*word, word.leading_ones());
            return Some(word_idx * 64 + word.leading_ones() as usize);
        }
        None
    }

    /// Allocate a free inode using first match algorithm
    /// Return an inode number as u16 on success, None on failure
    fn ialloc(&mut self) -> Option<u16> {
        let i = self.first_match(INODE_BITMAP_BLOCK)?;
        Some(i as u16)
    }

    /// Allocate a free data block using first match algorithm
    /// Return a block id as u16 on success, None on failure
    fn balloc(&mut self) -> Option<u16> {
        let bid = self.first_match(FREE_BITMAP_BLOCK)?;
        Some(bid as u16)
    }

    /// Format the disk image to Ext2 Filesystem.
    /// Super Block:       1 block
    /// Free Bitmap:       1 blocks
    /// Free Inode Bitmap: 1 block
    /// Inode Table:       64 blocks
    /// Data:              remaining blocks
    ///
    /// Root directory is allocated as the first inode initially
    pub fn format(&mut self) -> Result<()> {
        let image_size = self.image.len();
        let block_count = image_size / BLOCK_SZ;
        let super_blk = SuperBlk {
            s_inodes_count: 1,
            s_blocks_count: block_count as u16,
            s_free_blocks_count: (block_count - META_BLOCKS_SZ - 1) as u16,
            s_free_inodes_count: (MAX_FILE_COUNT - 1) as u16,
            s_first_data_block: META_BLOCKS_SZ as u16,
            s_block_size: BLOCK_SZ as u16,
            s_last_allocate: (META_BLOCKS_SZ + 1) as u16,
            s_magic: 0xEF53,
        };
        assert!(size_of::<SuperBlk>() <= BLOCK_SZ);
        let mut super_block = [0; BLOCK_SZ];
        super_block[SUPER_BLOCK .. SUPER_BLOCK + size_of::<SuperBlk>()]
            .copy_from_slice(unsafe { transmute::<&SuperBlk, &[u8; size_of::<SuperBlk>()]>(&super_blk) } );
        // writing super block
        self.bwrite(&super_block, SUPER_BLOCK);

        let free_bitmap = [0u8; BLOCK_SZ];
        // writing free bitmap block
        self.bwrite(&free_bitmap, FREE_BITMAP_BLOCK);

        let inode_bitmap = [0u8; BLOCK_SZ];
        // writing inode bitmap block
        self.bwrite(&inode_bitmap, INODE_BITMAP_BLOCK);

        // prepare root directory inode
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // allocate the inode number for root directory
        let ino = self.ialloc().ok_or(FormatError(1))?;
        // allocate the first data block for root directory
        let bid = self.balloc().ok_or(FormatError(2))?;
        let inode = Inode::new_dir(time, 0u32, bid);
        let mut inode_block = [0u8; BLOCK_SZ];
        inode_block[ino as usize * size_of::<Inode>() .. (ino as usize + 1) * size_of::<Inode>()]
            .copy_from_slice( unsafe { transmute::<&Inode, &[u8; size_of::<Inode>()]>(&inode)} );
        self.bwrite(&inode_block, bid.into());
        Ok(())
    }
}