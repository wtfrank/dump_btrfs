use crate::address::*;
use crate::btrfs::*;
use crate::structures::*;
use crate::tree::*;

use anyhow::*;

fn uuid_str(uuid: &BtrfsUuid) -> String {
    std::format!(
        "{}-{}-{}-{}-{}",
        hex::encode(&uuid[0..4]),
        hex::encode(&uuid[4..6]),
        hex::encode(&uuid[6..8]),
        hex::encode(&uuid[8..10]),
        hex::encode(&uuid[10..])
    )
}

pub fn dump_sb(sb: &btrfs_super_block) {
    let sectorsize = sb.sectorsize;
    let nodesize = sb.nodesize;
    let stripesize = sb.stripesize;

    println!("sector size: {sectorsize}");
    println!("node size: {nodesize}");
    println!("stripe size: {stripesize}");
}

/// sys_chunk_array has members with inconsistent lengths. Each member is comprised of a btrfs_disk_key, a btrfs_chunk (which contains one btrfs_stripe) then btrfs_chunk.num_stripes -1 additional btrfs_stripes.
pub fn dump_chunks(sb: &btrfs_super_block) {
    //let sys_chunk_array_size = sb.sys_chunk_array_size;
    //println!("sys_chunk_array_size: {}", sys_chunk_array_size);
    let chunk_root = sb.chunk_root;
    for ChunkInfo(key, chunk, stripes) in SysChunkIter::new(sb) {
        let length = chunk.length;
        let owner = chunk.owner;
        let num_stripes = chunk.num_stripes;
        let num_substripes = chunk.sub_stripes;
        let objectid = key.objectid;
        let offset = key.offset;

        assert_eq!(key.item_type, BtrfsItemType::CHUNK_ITEM);
        assert_eq!(objectid, BTRFS_FIRST_CHUNK_TREE_OBJECTID);
        assert_eq!(offset, chunk_root);
        //disk key offset is the virtual location
        //stripe devid/offset is the physical location
        println!("chunk: objectid {objectid} offset {offset} length {length} owner {owner} num_stripes: {num_stripes} substripes: {num_substripes}");
        for stripe in stripes {
            dump_stripe(&stripe);
        }
    }
}

fn dump_stripe(stripe: &btrfs_stripe) {
    let devid = stripe.devid;
    let offset = stripe.offset;
    println!(
        "devid: {}, offset: {}, dev_uuid: {}",
        devid,
        offset,
        uuid_str(&stripe.dev_uuid)
    );
}

pub fn dump_node_header(node_header: &btrfs_header) {
    let owner = node_header.owner;
    let gen = node_header.generation;
    let nri = node_header.nritems;
    let level = node_header.level;

    println!(
        "node header: owner {}, uuid {}, generation: {}, nritems: {}, level: {}",
        owner,
        uuid_str(&node_header.chunk_tree_uuid),
        gen,
        nri,
        level
    );
}

pub fn dump_tree(fs: &FsInfo, root: LE64) -> Result<()> {
    let node_header = load_virt::<btrfs_header>(fs, root)?;
    assert_eq!(node_header.fsid, fs.fsid);
    let bytenr = node_header.bytenr;
    assert_eq!(bytenr, root);
    let node = &load_virt_block(fs, root)?[BTRFS_CSUM_SIZE..];
    assert_eq!(node_header.csum, csum_data(node, fs.master_sb.csum_type));
    dump_node_header(node_header);
    //TODO: dump nodes
    let search = NodeSearchOption {
        min_object_id: 0,
        max_object_id: u64::MAX,
        min_item_type: BtrfsItemType::MIN,
        max_item_type: BtrfsItemType::MAX,
        min_offset: 0,
        max_offset: u64::MAX,
    };
    for _leaf in BtrfsTreeIter::new(fs, root, search) {
        println!("leaf");
    }
    Ok(())
}
