use crate::address::*;
use crate::btrfs::*;
use crate::structures::*;
use crate::tree::*;

use anyhow::*;
use more_asserts::*;
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

pub fn fmt_treeid(treeid: u64) -> String {
    match treeid {
        BTRFS_ROOT_TREE_OBJECTID => String::from("ROOT_TREE"),
        BTRFS_EXTENT_TREE_OBJECTID => String::from("EXTENT_TREE"),
        BTRFS_CHUNK_TREE_OBJECTID => String::from("CHUNK_TREE"),
        BTRFS_DEV_TREE_OBJECTID => String::from("DEV_TREE"),
        BTRFS_FS_TREE_OBJECTID => String::from("FS_TREE"),
        BTRFS_ROOT_TREE_DIR_OBJECTID => String::from("ROOT_TREE_DIR"),
        BTRFS_CSUM_TREE_OBJECTID => String::from("CSUM_TREE"),
        BTRFS_QUOTA_TREE_OBJECTID => String::from("QUOTA_TREE"),
        BTRFS_UUID_TREE_OBJECTID => String::from("UUID_TREE"),
        BTRFS_FREE_SPACE_TREE_OBJECTID => String::from("FREE_SPACE_TREE"),
        BTRFS_BLOCK_GROUP_TREE_OBJECTID => String::from("BLOCK_GROUP_TREE"),
        BTRFS_DEV_STATS_OBJECTID => String::from("DEV_STATS"),
        BTRFS_BALANCE_OBJECTID => String::from("BALANCE"),
        BTRFS_ORPHAN_OBJECTID => String::from("ORPHAN"),
        BTRFS_TREE_LOG_OBJECTID => String::from("TREE_LOG"),
        BTRFS_TREE_LOG_FIXUP_OBJECTID => String::from("TREE_LOG_FIXUP"),
        BTRFS_TREE_RELOC_OBJECTID => String::from("TREE_RELOC"),
        BTRFS_DATA_RELOC_TREE_OBJECTID => String::from("DATA_RELOC_TREE"),
        BTRFS_EXTENT_CSUM_OBJECTID => String::from("EXTENT_CSUM"),
        BTRFS_FREE_SPACE_OBJECTID => String::from("FREE_SPACE"),
        BTRFS_FREE_INO_OBJECTID => String::from("FREE_INO"),
        BTRFS_MULTIPLE_OBJECTIDS => String::from("MULTIPLE_OBJECTIDS"),

        _ => format!("{treeid}"),
    }
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
        min_key: btrfs_disk_key {
            objectid: 0,
            item_type: BtrfsItemType::MIN,
            offset: 0,
        },
        max_key: btrfs_disk_key {
            objectid: u64::MAX,
            item_type: BtrfsItemType::MAX,
            offset: u64::MAX,
        },
        min_match: std::cmp::Ordering::Less,
        max_match: std::cmp::Ordering::Greater,
    };
    for (leaf, _data) in BtrfsTreeIter::new(fs, root, search) {
        let btrfs_disk_key {
            objectid,
            item_type,
            offset,
        } = leaf.key;
        let size = leaf.size;

        println!(
            "leaf {} {item_type:?} {offset} data size {}",
            fmt_treeid(objectid),
            size
        );
    }
    Ok(())
}

pub fn dump_root_tree(fs: &FsInfo) -> Result<()> {
    let root = fs.master_sb.root;
    let node_header = load_virt::<btrfs_header>(fs, root)?;
    assert_eq!(node_header.fsid, fs.fsid);
    let bytenr = node_header.bytenr;
    assert_eq!(bytenr, root);
    let node = &load_virt_block(fs, root)?[BTRFS_CSUM_SIZE..];
    assert_eq!(node_header.csum, csum_data(node, fs.master_sb.csum_type));
    dump_node_header(node_header);
    //TODO: dump nodes
    let search = NodeSearchOption {
        min_key: btrfs_disk_key {
            objectid: 0,
            item_type: BtrfsItemType::MIN,
            offset: 0,
        },
        max_key: btrfs_disk_key {
            objectid: u64::MAX,
            item_type: BtrfsItemType::MAX,
            offset: u64::MAX,
        },
        min_match: std::cmp::Ordering::Less,
        max_match: std::cmp::Ordering::Greater,
    };
    for (leaf, data) in BtrfsTreeIter::new(fs, root, search) {
        let btrfs_disk_key {
            objectid,
            item_type,
            offset,
        } = leaf.key;
        let size = leaf.size;

        match item_type {
            BtrfsItemType::ROOT_ITEM => {
                assert_eq!(size as usize, std::mem::size_of::<btrfs_root_item>());
                let root_item = unsafe { &*((data.as_ptr()) as *const btrfs_root_item) };
                let tree_root = root_item.bytenr;
                println!(
                    "leaf {} {item_type:?} {offset} data size {} tree root {tree_root}",
                    fmt_treeid(objectid),
                    size
                );
            }
            BtrfsItemType::ROOT_REF => {
                assert_ge!(size as usize, std::mem::size_of::<btrfs_root_ref>());

                let root_ref = unsafe { &*((data.as_ptr()) as *const btrfs_root_ref) };
                let name_len = root_ref.name_len;
                let dirid = root_ref.dirid;
                assert_eq!(
                    size as usize,
                    name_len as usize + std::mem::size_of::<btrfs_root_ref>()
                );
                println!(
                    "root ref {} {item_type:?} {offset} dirid {dirid} name: {}",
                    fmt_treeid(objectid),
                    std::str::from_utf8(&data[std::mem::size_of::<btrfs_root_ref>()..])?
                );
            }
            _ => {}
        }
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
    dump_root_tree(&fs)?;
    //dump_tree(&fs, fs.master_sb.root)?;

    //TODO: do we need log tree?
    //TODO: build root tree
    //TODO: function to obtain offset of a particular tree root
    //TODO: load extent tree
    //TODO: command line argument to interpret a particular block and write it to a file
    //TODO: command line argument to replace a particular block (in all stripes) from a file
    //      and update its checksum
    //TODO: probably edge cases in tree iteration, so write tests
    Ok(())
}
