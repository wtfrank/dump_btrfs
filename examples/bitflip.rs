use anyhow::*;
use clap::Parser;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use btrfs_kit::address::*;
use btrfs_kit::btrfs::*;
use btrfs_kit::dump::*;
use btrfs_kit::structures::*;
use btrfs_kit::tree::*;

/// a particular leaf node entry in the extent tree is known to have suffered a bitflip
/// leading to an invalid extent length being written to disc.
/// The corrupted key is (21866556112896 EXTENT_ITEM 4503599627378688 )
/// That last value (btrfs_disk_key.offset) is 4PiB + 8KiB.
/// Or in hexadecimal: 0x10000000002000.
/// Our goal is to flip the 52nd bit back to 0, so that the offset is returned to the correct
/// value of 8 KiB.
/// This will change that node's checksum so we must recalculate that, and then write the
/// corrected block back to the filesystem (possibly in more than one location if certain raid modes are in use).
///
/// Each available block device in the filesystem should be specified on the command line.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
struct Params {
    #[clap(required = true)]
    paths: Vec<std::path::PathBuf>,
}

fn write_backup(data: &Vec<u8>, path: &Path) -> anyhow::Result<()> {
    let mut file = File::create(&path)?;

    file.write_all(data)?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::init();
    let args = Params::parse();

    /* add specified devices to some interal structures and read superblock */
    let fs = btrfs_kit::btrfs::load_fs(&args.paths)?;

    /* report how many of the filesystem devices have been provided */
    for (devid, di) in fs.devid_map.iter() {
        println!("devid {} is {}", devid, di.path.display());
    }
    let num_devices = fs.master_sb.num_devices;
    println!("{}/{} devices present", fs.devid_map.len(), num_devices);

    /* scan the root tree for the extent tree root */
    let extent_tree_root = tree_root_offset(&fs, BTRFS_EXTENT_TREE_OBJECTID)
        .ok_or_else(|| anyhow!("couldn't find extent tree root"))?;
    println!("root of extent tree: {}", extent_tree_root);

    /* find the address of the block containing the key we know to be bad */
    let bad_key = btrfs_disk_key {
        objectid: 21866556112896,
        item_type: BtrfsItemType::EXTENT_ITEM,
        offset: 4503599627378688,
    };

    let search = NodeSearchOption {
        min_key: bad_key,
        max_key: bad_key,
        min_match: std::cmp::Ordering::Less,
        max_match: std::cmp::Ordering::Greater,
    };
    let corrupt_offset;
    if let Some((leaf, _data, block_offset, leaf_number)) =
        BtrfsTreeIter::new(&fs, extent_tree_root, search).next()
    {
        let btrfs_disk_key {
            objectid,
            item_type,
            offset,
        } = leaf.key;
        let size = leaf.size;

        println!(
            "leaf #{leaf_number} {} {item_type:?} {offset} data size {} at block offset {block_offset}",
            fmt_treeid(objectid),
            size
        );
        corrupt_offset = block_offset;
    } else {
        panic!("Didn't find leaf block containing key");
    }

    println!("corrupt block virtual address: {corrupt_offset}");

    //obtain a read-only slice of this block in memory
    let corrupt_block = load_virt_block(&fs, corrupt_offset)?;
    let mut v = Vec::new();
    v.extend_from_slice(corrupt_block);
    assert_eq!(v.len(), fs.master_sb.nodesize as usize);

    let backup_filename = format!("offset_{corrupt_offset}_backup.bin");
    write_backup(&v, Path::new(&backup_filename))?;

    //TODO: edit the block, update checksum, write block back to all copies

    Ok(())
}
