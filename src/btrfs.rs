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
/// It uses libblkid to scan devices that are part of the same filesystem
/// then performs a lot of checks on the validity of the superblock.
///
/// This programme does none of this, requiring the user to provide a list
/// of devices, and relies on the superblock already being known to be
/// valid.
///
/// FIXME: to support ssds, check all superblocks and load the one with
/// the highest generation
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
    type Item = (btrfs_disk_key, btrfs_chunk, Vec<btrfs_stripe>);

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

        for _ in 1..chunk.num_stripes {
            type StripeBuf = [u8; std::mem::size_of::<btrfs_stripe>()];
            let mut buf: StripeBuf = [0_u8; std::mem::size_of::<btrfs_stripe>()];
            self.cursor.read_exact(&mut buf).ok()?;
            extra_stripes.push(unsafe { std::mem::transmute::<StripeBuf, btrfs_stripe>(buf) });
        }
        //println!("after chunk, seek pos is {}", self.cursor.position());

        Some((key, chunk, extra_stripes))
    }
}

/// sys_chunk_array has members with inconsistent lengths. Each member is comprised of a btrfs_disk_key, a btrfs_chunk (which contains one btrfs_stripe) then btrfs_chunk.num_stripes -1 additional btrfs_stripes.
fn dump_chunks(sb: &btrfs_super_block) {
    //let sys_chunk_array_size = sb.sys_chunk_array_size;
    //println!("sys_chunk_array_size: {}", sys_chunk_array_size);
    let chunk_root = sb.chunk_root;
    for (key, chunk, extra_stripes) in SysChunkIter::new(sb) {
        let length = chunk.length;
        let owner = chunk.owner;
        let num_stripes = chunk.num_stripes;
        let objectid = key.objectid;
        let offset = key.offset;

        assert_eq!(key.r#type, BTRFS_CHUNK_ITEM_KEY);
        assert_eq!(objectid, BTRFS_FIRST_CHUNK_TREE_OBJECTID);
        assert_eq!(offset, chunk_root);
        println!("chunk: objectid {objectid} offset {offset} length {length} owner {owner} num_stripes: {num_stripes}");
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

struct DeviceInfo {
    path: PathBuf,
    file: File,
    devid: LE64,
    dev_uuid: BtrfsUuid,
}

pub fn dump(paths: &Vec<PathBuf>) -> Result<()> {
    let mut fsid = None;
    let mut devid_map = HashMap::<LE64, Rc<DeviceInfo>>::new();
    let mut devuuid_map = HashMap::<BtrfsUuid, Rc<DeviceInfo>>::new();
    let mut master_sb = None;
    for path in paths {
        println!("checking {}", path.display());
        let sb = load_sb(path)?;
        match fsid {
            None => fsid = Some(sb.fsid.clone()),
            Some(f) => assert_eq!(sb.fsid, f),
        };
        assert_eq!(sb.dev_item.fsid, fsid.unwrap());
        let di = Rc::new(DeviceInfo {
            path: path.clone(),
            file: File::open(path)?,
            devid: sb.dev_item.devid,
            dev_uuid: sb.dev_item.uuid.clone(),
        });
        devid_map.insert(di.devid.clone(), Rc::clone(&di));
        devuuid_map.insert(di.dev_uuid.clone(), Rc::clone(&di));
        master_sb = Some(sb);
    }
    assert!(master_sb.is_some());
    let sb = master_sb.unwrap();

    for (devid, di) in devid_map.iter() {
        println!("devid {} is {}", devid, di.path.display());
    }
    let num_devices = sb.num_devices;
    println!("{}/{} devices present", devid_map.len(), num_devices);

    //TODO: load all superblocks on each device and check generation
    //TODO: check superblocks agree about the number of devices
    //TODO: check a device is available containing the chunk tree root
    //TODO: build chunk tree
    //TODO: do we need log tree?
    //TODO: build root tree
    //TODO: load extent tree
    //TODO: command line argument to interpret a particular block and write it to a file
    //TODO: command line argument to replace a particular block (in all stripes) from a file
    //      and update its checksum
    Ok(())
}
