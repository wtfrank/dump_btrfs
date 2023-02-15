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

fn write_block_to_file(data: &Vec<u8>, path: &Path) -> anyhow::Result<()> {
    let mut file = File::create(&path)?;

    file.write_all(data)?;

    Ok(())
}

/* this function expects a specific leaf node block from the extent tree.
  it fixes the offset of a specific key.
*/
fn fix_block(csum_type: BtrfsCsumType, data: &mut Vec<u8>) {
    let ptr: *mut u8 = data.as_mut_ptr();

    unsafe {
        let header = ptr as *mut btrfs_header;
        let owner = (*header).owner;
        assert_eq!(owner, BTRFS_EXTENT_TREE_OBJECTID);
        let nritems = (*header).nritems;
        let start = ptr.add(std::mem::size_of::<btrfs_header>());
        println!("header: nritems {nritems} header {header:x?} start {start:x?}");

        for i in 0..nritems {
            let leaf = start.add(i as usize * std::mem::size_of::<btrfs_item>()) as *mut btrfs_item;
            let objectid = (*leaf).key.objectid;
            let item_type = (*leaf).key.item_type;
            let key_offset = (*leaf).key.offset;
            let offset = (*leaf).offset;
            let size = (*leaf).size;
            println!("0x{leaf:x?} {objectid} {item_type:?} {key_offset} {offset} {size}");
            if objectid == 21866556112896
                && item_type == BtrfsItemType::EXTENT_ITEM
                && key_offset == 4503599627378688
            {
                println!("BINGO. Found our bad offset: 0x{key_offset:x}");
                (*leaf).key.offset = key_offset - 0x10000000000000;
                println!("de-flipped bit");
            }
        }

        let slice =
            &std::slice::from_raw_parts::<u8>(ptr, data.len())[std::mem::size_of::<BtrfsCsum>()..];

        (*header).csum = csum_data(slice, csum_type);
    }
}

/* this is a unit test really but final opportunity to prevent messups
   before data changes are made.
   checks that the data at the locations matches the data we think is there.
*/
fn check_physical_locations_match(
    physical_locations: &Vec<(u64, &Path)>,
    corrupt_data: &Vec<u8>,
) -> anyhow::Result<()> {
    for (offset, path) in physical_locations {
        println!("checking {}...", path.display());
        let mut file = File::open(path)?;

        let mut phys_data = vec![0_u8; corrupt_data.len()];

        println!("seeking to {offset}...");
        file.seek(std::io::SeekFrom::Start(*offset))?;
        println!("reading...");
        file.read(&mut phys_data)?;

        assert_eq!(*corrupt_data, phys_data);
    }
    println!("physical data as expected. ✔️");
    Ok(())
}

fn write_block_to_physical(
    data: &Vec<u8>,
    physical_locations: &Vec<(u64, &Path)>,
) -> anyhow::Result<()> {
    Err(anyhow!("TODO"))
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
    let mut corrupt_vec = Vec::new();
    corrupt_vec.extend_from_slice(corrupt_block);
    assert_eq!(corrupt_vec.len(), fs.master_sb.nodesize as usize);

    let backup_filename = format!("offset_{corrupt_offset}_backup.bin");
    write_block_to_file(&corrupt_vec, Path::new(&backup_filename))?;

    // this function is very specific to my fault - rewrites a leaf entry then recalculates the checksum
    let mut fixed_vec = corrupt_vec.clone();
    fix_block(fs.master_sb.csum_type, &mut fixed_vec);

    let fixed_filename = format!("offset_{corrupt_offset}_fixed.bin");
    write_block_to_file(&fixed_vec, Path::new(&fixed_filename))?;

    println!("original block at virtual {corrupt_offset} saved at {backup_filename}. fixed 🤞 block saved at {fixed_filename}");

    //find devices/offsets that the block's virtual address maps to
    let physical_locations: Vec<(u64, &Path)> = virtual_offset_to_physical(&fs, corrupt_offset)?;

    println!(
        "found {} physical locs: {:?}",
        physical_locations.len(),
        physical_locations
    );

    //safety first
    check_physical_locations_match(&physical_locations, &corrupt_vec)?;
    //write block back to all copies.

    // EXTREMELY IMPORTANT NOTE: Rather than applying the fix directly to the filesystem, do this frst with a qcow2 backed disc under KVM, so that the fix can be tested, and reverted if there's a problem
    write_block_to_physical(&fixed_vec, &physical_locations)?;

    println!("correct block written to physical location(s)");
    Ok(())
}
