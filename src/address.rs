use crate::btrfs::*;
use crate::tree::*;
use crate::types::*;

use anyhow::*;

struct ChunkStripeIter<'a> {
    index: usize,
    total: usize,
    data: &'a [u8],
}

impl<'a> ChunkStripeIter<'a> {
    pub fn new(buffer: &'a [u8], num_stripes: usize) -> ChunkStripeIter {
        ChunkStripeIter {
            index: 0,
            total: num_stripes,
            data: buffer,
        }
    }
}

impl<'a> Iterator for ChunkStripeIter<'a> {
    type Item = &'a btrfs_stripe;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.total {
            None
        } else {
            let stripe = unsafe {
                &*std::mem::transmute::<*const u8, *const btrfs_stripe>(
                    self.data.as_ptr().offset(
                        (self.index * std::mem::size_of::<btrfs_stripe>())
                            .try_into()
                            .unwrap(),
                    ),
                )
            };
            self.index += 1;
            Some(stripe)
        }
    }
}

/// returns reference to the structure of a specified type at a particular virtual address
/// first check bootstrap chunks from superblock, if not found search chunk tree
pub fn load_virt<T>(fs: &FsInfo, virt_offset: u64) -> Result<&T> {
    println!("load_virt: {virt_offset}");
    // ChunkInfo is btrfs_disk_key, btrfs_chunk, stripes
    for chunk in &fs.bootstrap_chunks {
        let start = chunk.0.offset;
        let length = chunk.1.length;
        //don't think we need to handle requesting an object that goes past a chunk boundary
        if virt_offset >= start && virt_offset < start + length {
            for stripe in &chunk.2 {
                let devid = stripe.devid;
                if let Some(dev) = fs.devid_map.get(&devid) {
                    return Ok(dev
                        .file
                        .at::<T>((virt_offset - start + stripe.offset) as usize));
                }
            }

            return Err(anyhow!("no device containing stripe copy is present"));
        }
    }

    /* obtain leaf node structure + data slice */
    for leaf_item in fs.search_node(
        fs.master_sb.chunk_root,
        &NodeSearchOption {
            min_object_id: BTRFS_FIRST_CHUNK_TREE_OBJECTID,
            max_object_id: BTRFS_FIRST_CHUNK_TREE_OBJECTID,
            min_item_type: BtrfsItemType::CHUNK_ITEM,
            max_item_type: BtrfsItemType::CHUNK_ITEM,
            min_offset: virt_offset,
            max_offset: virt_offset,
        },
    ) {
        let size = leaf_item.0.size;
        let chunk =
            unsafe { &*std::mem::transmute::<*const u8, *const btrfs_chunk>(leaf_item.1.as_ptr()) };
        let length = chunk.length;
        let owner = chunk.owner;
        let num_stripes = chunk.num_stripes;
        let start = leaf_item.0.key.offset;
        println!(
            "Found leaf chunk item: length: {}, owner: {}, num_stripes {}",
            length, owner, num_stripes
        );
        assert_eq!(
            size as usize,
            std::mem::size_of::<btrfs_chunk>()
                + chunk.num_stripes as usize * std::mem::size_of::<btrfs_stripe>()
        );
        for stripe in ChunkStripeIter::new(
            unsafe {
                std::slice::from_raw_parts::<'_, u8>(
                    leaf_item.1.as_ptr().add(std::mem::size_of::<btrfs_chunk>()),
                    size as usize,
                )
            },
            num_stripes.into(),
        ) {
            let devid = stripe.devid;
            let offset = stripe.offset;

            println!(
                "stripe devid {devid} offset {offset}, virt_offset {virt_offset}, start {start}"
            );
            if let Some(dev) = fs.devid_map.get(&devid) {
                return Ok(dev.file.at::<T>((virt_offset - start + offset) as usize));
            }
        }
    }

    Err(anyhow!(
        "virt address {virt_offset} not found among available chunks/devices"
    ))
}

pub fn load_virt_block(fs: &FsInfo, virt_offset: u64, length: u64) -> Result<&[u8]> {
    println!("load_virt_block: {virt_offset} length {length}");
    for chunk in &fs.bootstrap_chunks {
        let start = chunk.0.offset;
        let length = chunk.1.length;
        if virt_offset >= start && virt_offset < start + length {
            for stripe in &chunk.2 {
                let devid = stripe.devid;
                if let Some(dev) = fs.devid_map.get(&devid) {
                    return Ok(dev.file.slice(
                        (virt_offset - start + stripe.offset) as usize,
                        length as usize,
                    ));
                }
            }
            return Err(anyhow!("no device containing stripe copy is present"));
        }
    }

    /* obtain leaf node structure + data slice */
    for leaf_item in fs.search_node(
        fs.master_sb.chunk_root,
        &NodeSearchOption {
            min_object_id: BTRFS_FIRST_CHUNK_TREE_OBJECTID,
            max_object_id: BTRFS_FIRST_CHUNK_TREE_OBJECTID,
            min_item_type: BtrfsItemType::CHUNK_ITEM,
            max_item_type: BtrfsItemType::CHUNK_ITEM,
            min_offset: virt_offset,
            max_offset: virt_offset,
        },
    ) {
        println!("Found leaf item");
    }

    Err(anyhow!(
        "virt address {virt_offset} not found among available chunks/devices"
    ))
}