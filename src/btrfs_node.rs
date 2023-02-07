use crate::types::*;

pub struct BtrfsLeafNodeIter<'a> {
    block: &'a [u8],
    cur_item: u32,
}

/// iterator through btrfs nodes
/// accepts a slice to a block, then returns an Iterator object
/// with methods to return a reference to the block header,
/// and iterate through the key pointers/items, or perform
/// binary search to locate a key pointer/item matching a spec

pub fn btrfs_leaf_node(block: &[u8]) -> BtrfsLeafNodeIter {
    BtrfsLeafNodeIter { block, cur_item: 0 }
}

impl<'a> BtrfsLeafNodeIter<'a> {
    pub fn header(&self) -> &btrfs_header {
        unsafe { &*(self.block.as_ptr() as *const btrfs_header) }
    }

    //TODO: pub fn search(&self, btrfs_search_options)
}

impl<'a> Iterator for BtrfsLeafNodeIter<'a> {
    type Item = (&'a btrfs_item, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_item >= self.header().nritems {
            return None;
        }

        let offset = std::mem::size_of::<btrfs_header>()
            + self.cur_item as usize * std::mem::size_of::<btrfs_item>();
        self.cur_item += 1;
        let item = unsafe { &*((self.block.as_ptr() as usize + offset) as *const btrfs_item) };
        let data_offset = std::mem::size_of::<btrfs_header>() + item.offset as usize;
        Some((
            item,
            &self.block[data_offset..(data_offset + item.size as usize)],
        ))
    }
}
//////////////////////////////////////////////////////////////////////
pub struct BtrfsInternalNodeIter<'a> {
    block: &'a [u8],
    cur_item: u32,
}

/// iterator through btrfs nodes
/// accepts a slice to a block, then returns an Iterator object
/// with methods to return a reference to the block header,
/// and iterate through the key pointers/items, or perform
/// binary search to locate a key pointer/item matching a spec

pub fn btrfs_internal_node(block: &[u8]) -> BtrfsInternalNodeIter {
    BtrfsInternalNodeIter { block, cur_item: 0 }
}

impl<'a> BtrfsInternalNodeIter<'a> {
    pub fn header(&self) -> &btrfs_header {
        unsafe { &*(self.block.as_ptr() as *const btrfs_header) }
    }

    //TODO: pub fn search(&self, btrfs_search_options)
}

impl<'a> Iterator for BtrfsInternalNodeIter<'a> {
    type Item = &'a btrfs_key_ptr;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_item >= self.header().nritems {
            return None;
        }

        let offset = std::mem::size_of::<btrfs_header>()
            + self.cur_item as usize * std::mem::size_of::<btrfs_key_ptr>();
        self.cur_item += 1;
        let item = unsafe { &*((self.block.as_ptr() as usize + offset) as *const btrfs_key_ptr) };
        Some(item)
    }
}
