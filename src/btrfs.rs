use crate::address::*;
use crate::dump::*;
use crate::mapped_file::MappedFile;
use crate::tree::*;
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

pub struct SysChunkIter<'a> {
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
        let mut stripes = Vec::<btrfs_stripe>::new();

        type DiskKeyBuf = [u8; std::mem::size_of::<btrfs_disk_key>()];
        let mut buf: DiskKeyBuf = [0_u8; std::mem::size_of::<btrfs_disk_key>()];
        self.cursor.read_exact(&mut buf).ok()?;
        let key = unsafe { std::mem::transmute::<DiskKeyBuf, btrfs_disk_key>(buf) };

        type ChunkBuf = [u8; std::mem::size_of::<btrfs_chunk>()];
        let mut buf: ChunkBuf = [0_u8; std::mem::size_of::<btrfs_chunk>()];
        self.cursor.read_exact(&mut buf).ok()?;
        let chunk = unsafe { std::mem::transmute::<ChunkBuf, btrfs_chunk>(buf) };

        //TODO: in raid0 we have to alternate between stripes?
        //TODO: in raid10, should we multiply stripes and substripes together?
        for _ in 0..chunk.num_stripes {
            type StripeBuf = [u8; std::mem::size_of::<btrfs_stripe>()];
            let mut buf: StripeBuf = [0_u8; std::mem::size_of::<btrfs_stripe>()];
            self.cursor.read_exact(&mut buf).ok()?;
            stripes.push(unsafe { std::mem::transmute::<StripeBuf, btrfs_stripe>(buf) });
        }
        //println!("after chunk, seek pos is {}", self.cursor.position());

        Some(ChunkInfo(key, chunk, stripes))
    }
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
        ret[..cs.len()].copy_from_slice(&cs[..]);
    }
    ret
}

pub struct DeviceInfo {
    pub path: PathBuf,
    pub file: MappedFile,
    pub devid: LE64,
    pub dev_uuid: BtrfsUuid,
}

pub struct ChunkInfo(pub btrfs_disk_key, pub btrfs_chunk, pub Vec<btrfs_stripe>);

/// processed info about the filesystem
pub struct FsInfo {
    pub fsid: BtrfsFsid,
    pub devid_map: HashMap<LE64, Rc<DeviceInfo>>,
    pub devuuid_map: HashMap<BtrfsUuid, Rc<DeviceInfo>>,
    pub master_sb: btrfs_super_block,
    pub bootstrap_chunks: Vec<ChunkInfo>,
}

impl FsInfo {
    pub fn search_node(&self, tree_root: LE64, options: &NodeSearchOption) -> BtrfsTreeIter {
        BtrfsTreeIter::new(self, tree_root, *options)
    }
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
            None => fsid = Some(sb.fsid),
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
            dev_uuid: sb.dev_item.uuid,
        });
        devid_map.insert(di.devid, Rc::clone(&di));
        devuuid_map.insert(di.dev_uuid, Rc::clone(&di));
        master_sb = Some(sb);
        if initial_chunks.is_empty() {
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
    let ct_header = load_virt::<btrfs_header>(&fs, sb.chunk_root)?;
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

    //TODO: move print-related things into another module
    //TODO: mem mapped access to virtual locations
    //TODO: load all superblocks on each device and check generation (for ssds)
    //TODO: unify load_virt and load_virt_block
    //TODO: do we need log tree?
    //TODO: which trees (if any) do we keep in memory, and which do we read from disc on demand via MappedFile?
    //TODO: build root tree
    //TODO: load extent tree
    //TODO: command line argument to interpret a particular block and write it to a file
    //TODO: command line argument to replace a particular block (in all stripes) from a file
    //      and update its checksum
    Ok(())
}
