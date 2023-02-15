use crate::address::*;
use crate::btrfs::*;
use crate::structures::*;

pub struct BtrfsLeafNodeIter<'a> {
    block: &'a [u8],
    cur_item: u32,
    pub block_offset: u64,
}

/// iterator through btrfs nodes
/// accepts a slice to a block, then returns an Iterator object
/// with methods to return a reference to the block header,
/// and iterate through the key pointers/items, or perform
/// binary search to locate a key pointer/item matching a spec

pub fn block_as_leaf_node(block: &[u8], block_offset: u64) -> BtrfsLeafNodeIter {
    BtrfsLeafNodeIter {
        block,
        cur_item: 0,
        block_offset,
    }
}

/// block_offset is the virtual address of the block, which will be
/// loaded then interpreted as a leaf node
pub fn btrfs_leaf_node(fs: &FsInfo, block_offset: u64) -> anyhow::Result<BtrfsLeafNodeIter> {
    let block = load_virt_block(fs, block_offset)?;
    Ok(BtrfsLeafNodeIter {
        block,
        cur_item: 0,
        block_offset,
    })
}

impl<'a> BtrfsLeafNodeIter<'a> {
    pub fn header(&self) -> &btrfs_header {
        unsafe { &*(self.block.as_ptr() as *const btrfs_header) }
    }

    pub fn peek(&self) -> Option<<Self as Iterator>::Item> {
        if self.cur_item >= self.header().nritems {
            return None;
        }

        let offset = std::mem::size_of::<btrfs_header>()
            + self.cur_item as usize * std::mem::size_of::<btrfs_item>();
        let item = unsafe { &*((self.block.as_ptr() as usize + offset) as *const btrfs_item) };
        let data_offset = std::mem::size_of::<btrfs_header>() + item.offset as usize;
        Some((
            item,
            &self.block[data_offset..(data_offset + item.size as usize)],
            self.block_offset,
            self.cur_item,
        ))
    }

    //TODO: pub fn search(&self, btrfs_search_options)
}

impl<'a> Iterator for BtrfsLeafNodeIter<'a> {
    type Item = (&'a btrfs_item, &'a [u8], u64, u32);

    fn next(&mut self) -> Option<Self::Item> {
        match self.peek() {
            None => None,
            Some(s) => {
                self.cur_item += 1;
                Some(s)
            }
        }
    }
}
//////////////////////////////////////////////////////////////////////
pub struct BtrfsInternalNodeIter<'a> {
    block: &'a [u8],
    cur_item: u32,
    pub block_offset: u64,
}

impl<'a> BtrfsInternalNodeIter<'a> {
    /// reinterpret this internal node as a leaf node
    /// any iteration progress is reset.
    pub fn as_leaf_node(&self) -> BtrfsLeafNodeIter<'a> {
        let leaf = block_as_leaf_node(self.block, self.block_offset);
        leaf
    }
}

/// iterator through btrfs nodes
/// accepts a slice to a block, then returns an Iterator object
/// with methods to return a reference to the block header,
/// and iterate through the key pointers/items, or perform
/// binary search to locate a key pointer/item matching a spec

pub fn block_as_internal_node(block: &[u8], block_offset: u64) -> BtrfsInternalNodeIter {
    BtrfsInternalNodeIter {
        block,
        cur_item: 0,
        block_offset,
    }
}

pub fn btrfs_internal_node(
    fs: &FsInfo,
    block_offset: u64,
) -> anyhow::Result<BtrfsInternalNodeIter> {
    let block = load_virt_block(fs, block_offset)?;
    Ok(BtrfsInternalNodeIter {
        block,
        cur_item: 0,
        block_offset,
    })
}

impl<'a> BtrfsInternalNodeIter<'a> {
    pub fn header(&self) -> &btrfs_header {
        unsafe { &*(self.block.as_ptr() as *const btrfs_header) }
    }

    pub fn peek(&self) -> Option<<Self as Iterator>::Item> {
        if self.cur_item >= self.header().nritems {
            return None;
        }

        let offset = std::mem::size_of::<btrfs_header>()
            + self.cur_item as usize * std::mem::size_of::<btrfs_key_ptr>();
        let item = unsafe { &*((self.block.as_ptr() as usize + offset) as *const btrfs_key_ptr) };
        Some(item)
    }

    //TODO: pub fn search(&self, btrfs_search_options)
}

impl<'a> Iterator for BtrfsInternalNodeIter<'a> {
    type Item = &'a btrfs_key_ptr;

    fn next(&mut self) -> Option<Self::Item> {
        match self.peek() {
            None => None,
            Some(s) => {
                self.cur_item += 1;
                Some(s)
            }
        }
    }
}
