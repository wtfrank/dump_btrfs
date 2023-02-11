use crate::address::*;
use crate::btrfs::*;
use crate::structures::*;
use crate::tree::*;

use anyhow::*;
use std::path::PathBuf;

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

pub fn dump_fs(paths: &Vec<PathBuf>) -> Result<()> {
    let fs = load_fs(paths)?;
    let sb = fs.master_sb;
    dump_sb(&sb);

    dump_chunks(&sb);

    for (devid, di) in fs.devid_map.iter() {
        println!("devid {} is {}", devid, di.path.display());
    }
    let num_devices = sb.num_devices;
    println!("{}/{} devices present", fs.devid_map.len(), num_devices);

    // There are two things we need to be able to do with these trees,
    // iterate through an entire tree (perhaps until a condition is met),
    // and identify a specific key (or part of a key) in a tree.
    let ct_header = load_virt::<btrfs_header>(&fs, sb.chunk_root)?;
    assert_eq!(ct_header.fsid, fs.fsid);
    let bn = ct_header.bytenr;
    let cr = fs.master_sb.chunk_root;
    assert_eq!(bn, cr);
    //TODO: bother checking csum?
    let cto = ct_header.owner;
    //let ct_gen = ct_header.generation;
    let ct_nri = ct_header.nritems;
    //let ct_level = ct_header.level;
    assert_eq!(cto, BTRFS_CHUNK_TREE_OBJECTID);
    dump_node_header(ct_header);

    // for levels != 0 we have internal nodes
    // https://btrfs.wiki.kernel.org/index.php/On-disk_Format#Internal_Node

    //the first level of the tree looks like this. After the header there is  random DEV_ITEM
    //then a number of chunk_items. not clear what offset refers to.
    //chunk tree header: uuid ab00c287-f8de-4fe1-b463-61cfc5c6814c, generation: 4756888, nritems: 76, level: 1
    //object id: 1, node_type: DEV_ITEM, offset: 7, blockptr: 22093116751872, generation: 4756888
    //object id: 256, node_type: CHUNK_ITEM, offset: 21264188047360, blockptr: 22093116882944, generation: 3409876
    //...
    let key_ptr_start: u64 = sb.chunk_root + std::mem::size_of::<btrfs_header>() as u64;
    for i in 0..ct_nri {
        let int_node = load_virt::<btrfs_key_ptr>(
            &fs,
            key_ptr_start + i as u64 * std::mem::size_of::<btrfs_key_ptr>() as u64,
        )?;
        let oid = int_node.key.objectid;
        let node_type = int_node.key.item_type;
        let offset = int_node.key.offset;
        let blockptr = int_node.blockptr;
        let generation = int_node.generation;
        println!(
            "object id: {}, node_type: {:?}, offset: {}, blockptr: {}, generation: {}",
            oid, node_type, offset, blockptr, generation
        );
    }

    //let's look at one chunk item.
    let block_ptr = load_virt::<btrfs_key_ptr>(
        &fs,
        key_ptr_start + std::mem::size_of::<btrfs_key_ptr>() as u64,
    )?
    .blockptr;
    let node = load_virt::<btrfs_header>(&fs, block_ptr)?;
    dump_node_header(node);
    let node_items_start = block_ptr + std::mem::size_of::<btrfs_header>() as u64;
    for i in 0..node.nritems {
        let leaf_node = load_virt::<btrfs_item>(
            &fs,
            node_items_start + i as u64 * std::mem::size_of::<btrfs_item>() as u64,
        )?;
        let oid = leaf_node.key.objectid;
        let node_type = leaf_node.key.item_type;
        let offset = leaf_node.key.offset;
        let int_offset = leaf_node.offset;
        let size = leaf_node.size;

        println!(
            "object id: {}, node_type: {:?}, offset: {}, itemoffset: {}, size: {}",
            oid, node_type, offset, int_offset, size
        );
    }

    println!("root tree");
    dump_tree(&fs, fs.master_sb.root)?;

    //TODO: do we need log tree?
    //TODO: build root tree
    //TODO: load extent tree
    //TODO: command line argument to interpret a particular block and write it to a file
    //TODO: command line argument to replace a particular block (in all stripes) from a file
    //      and update its checksum
    Ok(())
}
