use crate::btrfs_node::*;
use crate::mapped_file::MappedFile;
use crate::types::*;
use anyhow::*;
use crc::{Crc, CRC_32_ISCSI};
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::PathBuf;
use std::rc::Rc;

/// on loading w
///
/// btrfsprogs does quite a lot of work when opening a btrfs filesystem.
/// It uses libblkid to scan devices and identify those that are part of
/// the same filesystem then performs a lot of checks on the validity of
/// the superblock.
///
/// This programme does none of this, requiring the user to provide a list
/// of devices, and relies on the superblock already being known to be
/// valid.
///
/// btrfs_new_fs_info
/// btrfs_scan_fs_devices
/// btrfs_open_devices
/// btrfs_read_dev_super
/// sbread
/// btrfs_check_super

fn load_sb(path: &PathBuf) -> Result<btrfs_super_block> {
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(BTRFS_SUPER_INFO_OFFSET.try_into()?))?;
    union SbBuf {
        buf: [u8; BTRFS_SUPER_INFO_SIZE],
        block: btrfs_super_block,
    }

    let mut sb: SbBuf = SbBuf {
        buf: [0_u8; BTRFS_SUPER_INFO_SIZE],
    };

    let sb = unsafe {
        f.read_exact(&mut sb.buf)?;
        if sb.block.magic != BTRFS_MAGIC {
            return Err(anyhow!("invalid magic in block"));
        };
        if csum_data(&sb.buf[BTRFS_CSUM_SIZE..], sb.block.csum_type) != sb.block.csum {
            return Err(anyhow!("invalid checksum in superblock"));
        }

        sb.block
    };

    //println!("sb loaded ok");

    dump_chunks(&sb);

    Ok(sb)
}

struct SysChunkIter<'a> {
    cursor: std::io::Cursor<&'a [u8]>,
    size: u64,
}

impl SysChunkIter<'_> {
    pub fn new(sb: &btrfs_super_block) -> SysChunkIter {
        SysChunkIter {
            cursor: std::io::Cursor::<&[u8]>::new(&sb.sys_chunk_array),
            size: sb.sys_chunk_array_size as u64,
        }
    }
}

impl Iterator for SysChunkIter<'_> {
    type Item = ChunkInfo; //(btrfs_disk_key, btrfs_chunk, Vec<btrfs_stripe>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor.position() >= self.size {
            assert_eq!(0, self.size % self.cursor.position());
            return None;
        }
        let mut extra_stripes = Vec::<btrfs_stripe>::new();

        type DiskKeyBuf = [u8; std::mem::size_of::<btrfs_disk_key>()];
        let mut buf: DiskKeyBuf = [0_u8; std::mem::size_of::<btrfs_disk_key>()];
        self.cursor.read_exact(&mut buf).ok()?;
        let key = unsafe { std::mem::transmute::<DiskKeyBuf, btrfs_disk_key>(buf) };

        type ChunkBuf = [u8; std::mem::size_of::<btrfs_chunk>()];
        let mut buf: ChunkBuf = [0_u8; std::mem::size_of::<btrfs_chunk>()];
        self.cursor.read_exact(&mut buf).ok()?;
        let chunk = unsafe { std::mem::transmute::<ChunkBuf, btrfs_chunk>(buf) };

        //TODO: in raid10, should we multiply stripes and substripes together?
        for _ in 1..chunk.num_stripes {
            type StripeBuf = [u8; std::mem::size_of::<btrfs_stripe>()];
            let mut buf: StripeBuf = [0_u8; std::mem::size_of::<btrfs_stripe>()];
            self.cursor.read_exact(&mut buf).ok()?;
            extra_stripes.push(unsafe { std::mem::transmute::<StripeBuf, btrfs_stripe>(buf) });
        }
        //println!("after chunk, seek pos is {}", self.cursor.position());

        Some(ChunkInfo(key, chunk, extra_stripes))
    }
}

/// sys_chunk_array has members with inconsistent lengths. Each member is comprised of a btrfs_disk_key, a btrfs_chunk (which contains one btrfs_stripe) then btrfs_chunk.num_stripes -1 additional btrfs_stripes.
fn dump_chunks(sb: &btrfs_super_block) {
    //let sys_chunk_array_size = sb.sys_chunk_array_size;
    //println!("sys_chunk_array_size: {}", sys_chunk_array_size);
    let chunk_root = sb.chunk_root;
    for ChunkInfo(key, chunk, extra_stripes) in SysChunkIter::new(sb) {
        let length = chunk.length;
        let owner = chunk.owner;
        let num_stripes = chunk.num_stripes;
        let num_substripes = chunk.sub_stripes;
        let objectid = key.objectid;
        let offset = key.offset;

        assert_eq!(key.r#type, BtrfsItemType::CHUNK_ITEM);
        assert_eq!(objectid, BTRFS_FIRST_CHUNK_TREE_OBJECTID);
        assert_eq!(offset, chunk_root);
        //disk key offset is the virtual location
        //stripe devid/offset is the physical location
        println!("chunk: objectid {objectid} offset {offset} length {length} owner {owner} num_stripes: {num_stripes} substripes: {num_substripes}");
        dump_stripe(&chunk.stripe);
        for stripe in extra_stripes {
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

/* the checksums range from 4-32 bytes depending on the algorithm in use. For simplicity we'll always return a 32 byte buffer, but this could be improved upon */
fn csum_data(buf: &[u8], csum_type: BtrfsCsumType) -> BtrfsCsum {
    match csum_type {
        BtrfsCsumType::CRC32 => csum_data_crc32(buf),
        _ => panic!("only crc32 checksums are implemented - could be a small project for you?"),
    }
}

fn csum_data_crc32(buf: &[u8]) -> [u8; BTRFS_CSUM_SIZE] {
    const CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);
    let mut ret = [0_u8; BTRFS_CSUM_SIZE];
    let cs = CASTAGNOLI.checksum(buf).to_le_bytes();
    for i in 0..cs.len() {
        ret[i] = cs[i];
    }
    ret
}

pub fn dump_sb(sb: &btrfs_super_block) {
    let sectorsize = sb.sectorsize;
    let nodesize = sb.nodesize;
    let stripesize = sb.stripesize;

    println!("sector size: {sectorsize}");
    println!("node size: {nodesize}");
    println!("stripe size: {stripesize}");
}

struct DeviceInfo {
    path: PathBuf,
    file: MappedFile,
    devid: LE64,
    dev_uuid: BtrfsUuid,
}

struct ChunkInfo(btrfs_disk_key, btrfs_chunk, Vec<btrfs_stripe>);

/// processed info about the filesystem
pub struct FsInfo {
    fsid: BtrfsFsid,
    devid_map: HashMap<LE64, Rc<DeviceInfo>>,
    devuuid_map: HashMap<BtrfsUuid, Rc<DeviceInfo>>,
    master_sb: btrfs_super_block,
    bootstrap_chunks: Vec<ChunkInfo>,
}

#[derive(Clone, Copy)]
pub struct node_search_option {
    min_object_id: LE64,
    max_object_id: LE64,
    min_item_type: BtrfsItemType,
    max_item_type: BtrfsItemType,
    min_offset: LE64,
    max_offset: LE64,
}

impl FsInfo {
    pub fn search_node(&self, tree_root: LE64, options: &node_search_option) -> BtrfsNodeIter {
        BtrfsNodeIter::new(&self, tree_root, *options)
    }
}

struct BtrfsNodeIter<'a> {
    fs: &'a FsInfo,
    root: LE64,
    options: node_search_option,
    // how do we track progress - current leaf?
    // usually we are working through leaf items in a single node so we want
    // this to be fast, and it's ok if we have to do a slower operation to start
    // a new node. if we have to look up chunk addresses every next() it will be a bit
    // slow so we should save a reference to an entire block.
    cur_l0_block: Option<(&'a btrfs_header, &'a [u8])>,
    cur_leaf_index: usize,
}

impl<'a> BtrfsNodeIter<'a> {
    pub fn new(fs: &FsInfo, root: LE64, options: node_search_option) -> BtrfsNodeIter {
        BtrfsNodeIter {
            fs,
            root,
            options,
            cur_l0_block: None,
            cur_leaf_index: 0,
        }
    }
}

impl<'a> Iterator for BtrfsNodeIter<'a> {
    type Item = (btrfs_item, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if let None = self.cur_l0_block {
            let header = load_virt::<btrfs_header>(self.fs, self.root).ok()?;
        }

        /*
            let header = load_virt::<btrfs_header>(self.fs, sb.chunk_root.try_into().unwrap())?;
            assert_eq!(ct_header.fsid, fs.fsid);
            let bn = ct_header.bytenr;
            let cr = fs.master_sb.chunk_root;
            assert_eq!(bn, cr);
            //TODO: bother checking csum?
            let cto = ct_header.owner;
            let ct_gen = ct_header.generation;
            let ct_nri = ct_header.nritems;
            let ct_level = ct_header.level;
            assert_eq!(cto, BTRFS_CHUNK_TREE_OBJECTID);
            dump_node_header(&ct_header);

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
                let node_type = int_node.key.r#type;
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
            dump_node_header(&node);
            let node_items_start = block_ptr + std::mem::size_of::<btrfs_header>() as u64;
            for i in 0..node.nritems {
                let leaf_node = load_virt::<btrfs_item>(
                    &fs,
                    node_items_start + i as u64 * std::mem::size_of::<btrfs_item>() as u64,
                )?;
                let oid = leaf_node.key.objectid;
                let node_type = leaf_node.key.r#type;
                let offset = leaf_node.key.offset;
                let int_offset = leaf_node.offset;
                let size = leaf_node.size;

                println!(
                    "object id: {}, node_type: {:?}, offset: {}, itemoffset: {}, size: {}",
                    oid, node_type, offset, int_offset, size
                );
            }

        */
        return None;
    }
}

/// returns reference to the structure of a specified type at a particular virtual address
/// first check bootstrap chunks from superblock, if not found search chunk tree
fn load_virt<T>(fs: &FsInfo, virt_offset: u64) -> Result<&T> {
    for chunk in &fs.bootstrap_chunks {
        let start = chunk.0.offset;
        let length = chunk.1.length;
        if virt_offset >= start && virt_offset < start + length {
            let devid = chunk.1.stripe.devid;
            let mut di = fs.devid_map.get(&devid);
            if di.is_some() {
                return Ok(di
                    .unwrap()
                    .file
                    .at::<T>((virt_offset - start + chunk.1.stripe.offset) as usize));
            }
            for stripe in &chunk.2 {
                let devid = stripe.devid;
                di = fs.devid_map.get(&devid);
                if di.is_some() {
                    return Ok(di
                        .unwrap()
                        .file
                        .at::<T>((virt_offset - start + stripe.offset) as usize));
                }
            }
        }
    }

    /* obtain leaf node structure + data slice */
    for leaf_item in fs.search_node(
        fs.master_sb.chunk_root,
        &node_search_option {
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

pub fn load_virt_block(fs: &FsInfo, virt_offset: u64, length: u64) -> Result<&[u8]> {
    for chunk in &fs.bootstrap_chunks {
        let start = chunk.0.offset;
        let length = chunk.1.length;
        if virt_offset >= start && virt_offset < start + length {
            let devid = chunk.1.stripe.devid;
            let mut di = fs.devid_map.get(&devid);
            if di.is_some() {
                return Ok(di.unwrap().file.slice(
                    (virt_offset - start + chunk.1.stripe.offset) as usize,
                    length as usize,
                ));
            }
            for stripe in &chunk.2 {
                let devid = stripe.devid;
                di = fs.devid_map.get(&devid);
                if di.is_some() {
                    return Ok(di.unwrap().file.slice(
                        (virt_offset - start + stripe.offset) as usize,
                        length as usize,
                    ));
                }
            }
        }
    }

    /* obtain leaf node structure + data slice */
    for leaf_item in fs.search_node(
        fs.master_sb.chunk_root,
        &node_search_option {
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

fn dump_node_header(node_header: &btrfs_header) {
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

fn dump_tree(fs: &FsInfo, root: LE64) -> Result<()> {
    let node_header = load_virt::<btrfs_header>(fs, root)?;
    assert_eq!(node_header.fsid, fs.fsid);
    let bytenr = node_header.bytenr;
    assert_eq!(bytenr, root);
    //TODO: bother checking csum?
    dump_node_header(&node_header);
    //TODO: dump nodes
    Ok(())
}

pub fn dump(paths: &Vec<PathBuf>) -> Result<()> {
    let mut fsid = None;
    let mut devid_map = HashMap::<LE64, Rc<DeviceInfo>>::new();
    let mut devuuid_map = HashMap::<BtrfsUuid, Rc<DeviceInfo>>::new();
    let mut master_sb: Option<btrfs_super_block> = None;
    let mut initial_chunks = Vec::new();
    for path in paths {
        println!("checking {}", path.display());
        let sb = load_sb(path)?;
        match fsid {
            None => fsid = Some(sb.fsid.clone()),
            Some(f) => assert_eq!(sb.fsid, f),
        };
        assert_eq!(sb.dev_item.fsid, fsid.unwrap());
        if let Some(prev_sb) = master_sb {
            let prev_num_devices = prev_sb.num_devices;
            let num_devices = sb.num_devices;
            assert_eq!(prev_num_devices, num_devices);
        }

        let di = Rc::new(DeviceInfo {
            path: path.clone(),
            file: MappedFile::open(path)?,
            devid: sb.dev_item.devid,
            dev_uuid: sb.dev_item.uuid.clone(),
        });
        devid_map.insert(di.devid.clone(), Rc::clone(&di));
        devuuid_map.insert(di.dev_uuid.clone(), Rc::clone(&di));
        master_sb = Some(sb);
        if initial_chunks.len() == 0 {
            for ci in SysChunkIter::new(&sb) {
                initial_chunks.push(ci);
            }
        }
    }
    assert!(master_sb.is_some());
    let sb = master_sb.unwrap();

    let fs = FsInfo {
        fsid: fsid.unwrap(),
        devid_map,
        devuuid_map,
        master_sb: sb,
        bootstrap_chunks: initial_chunks,
    };

    dump_sb(&sb);

    for (devid, di) in fs.devid_map.iter() {
        println!("devid {} is {}", devid, di.path.display());
    }
    let num_devices = sb.num_devices;
    println!("{}/{} devices present", fs.devid_map.len(), num_devices);

    // There are two things we need to be able to do with these trees,
    // iterate through an entire tree (perhaps until a condition is met),
    // and identify a specific key (or part of a key) in a tree.
    let ct_header = load_virt::<btrfs_header>(&fs, sb.chunk_root.try_into().unwrap())?;
    assert_eq!(ct_header.fsid, fs.fsid);
    let bn = ct_header.bytenr;
    let cr = fs.master_sb.chunk_root;
    assert_eq!(bn, cr);
    //TODO: bother checking csum?
    let cto = ct_header.owner;
    let ct_gen = ct_header.generation;
    let ct_nri = ct_header.nritems;
    let ct_level = ct_header.level;
    assert_eq!(cto, BTRFS_CHUNK_TREE_OBJECTID);
    dump_node_header(&ct_header);

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
        let node_type = int_node.key.r#type;
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
    dump_node_header(&node);
    let node_items_start = block_ptr + std::mem::size_of::<btrfs_header>() as u64;
    for i in 0..node.nritems {
        let leaf_node = load_virt::<btrfs_item>(
            &fs,
            node_items_start + i as u64 * std::mem::size_of::<btrfs_item>() as u64,
        )?;
        let oid = leaf_node.key.objectid;
        let node_type = leaf_node.key.r#type;
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

    //TODO: move print-related things into another module
    //TODO: change btrfs_chunk so it doesn't have one stripe
    //      record built in (which will simplify code elsewhere)
    //TODO: mem mapped access to virtual locations
    //TODO: load all superblocks on each device and check generation (for ssds)
    //TODO: build chunk tree
    //TODO: do we need log tree?
    //TODO: which trees (if any) do we keep in memory, and which do we read from disc on demand via MappedFile?
    //TODO: build root tree
    //TODO: load extent tree
    //TODO: command line argument to interpret a particular block and write it to a file
    //TODO: command line argument to replace a particular block (in all stripes) from a file
    //      and update its checksum
    Ok(())
}
