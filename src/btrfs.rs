use crate::dump::fmt_treeid;
use crate::mapped_file::MappedFile;
use crate::structures::*;
use crate::tree::*;
use anyhow::*;
use crc::{Crc, CRC_32_ISCSI};
use log::*;
use more_asserts::*;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::rc::Rc;

/// btrfs-kit is a library that provides tools to help with recovery of
/// corrupted btrfs filesystems.
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

fn load_sb_at(mf: &MappedFile, offset: usize) -> Result<btrfs_super_block> {
    let sb = mf.at::<btrfs_super_block>(offset);

    if sb.magic != BTRFS_MAGIC {
        return Err(anyhow!("invalid magic in block"));
    }
    unsafe {
        let ptr: *const btrfs_super_block = sb;
        let ptr_u8 = ptr as *const u8;
        let slice = std::slice::from_raw_parts(
            ptr_u8.add(BTRFS_CSUM_SIZE),
            BTRFS_SUPER_INFO_SIZE - BTRFS_CSUM_SIZE,
        );
        if csum_data(slice, sb.csum_type) != sb.csum {
            return Err(anyhow!("invalid checksum in superblock"));
        }
    }

    if sb.total_bytes == 0 {
        return Err(anyhow!("zero length filesystem"));
    }

    if sb.num_devices == 0 {
        return Err(anyhow!("no devices in filesystem"));
    }

    if sb.sectorsize == 0 {
        return Err(anyhow!("zero sector size"));
    }

    if sb.nodesize == 0 {
        return Err(anyhow!("zero node size"));
    }

    if sb.stripesize == 0 {
        return Err(anyhow!("zero stripe size"));
    }

    Ok(*sb)
}

/* read all superblocks in mapped file, then choose the one with the highest generation (as only one is updated at a time on ssds) */
fn load_sb(mf: &MappedFile) -> Result<btrfs_super_block> {
    assert_ge!(mf.len(), BTRFS_SUPER_INFO_OFFSET + BTRFS_SUPER_INFO_SIZE);
    let mut master_sb = load_sb_at(mf, BTRFS_SUPER_INFO_OFFSET)?;

    for mirror in 1..BTRFS_SUPER_MIRROR_MAX {
        let next_sb_offset = 0x4000 << (BTRFS_SUPER_MIRROR_SHIFT * mirror);
        debug!("reading superblock at {next_sb_offset}");
        if mf.len() >= next_sb_offset + BTRFS_SUPER_INFO_SIZE {
            let sb = load_sb_at(mf, next_sb_offset);
            match sb {
                Result::Err(e) => println!("superblock #{} invalid: {}", mirror + 1, e),
                Result::Ok(s) => {
                    if s.generation > master_sb.generation {
                        let sg = s.generation;
                        let msg = master_sb.generation;
                        debug!("sb #{} had higher generation {} vs {}", mirror + 1, sg, msg);
                        master_sb = s;
                    }
                }
            }
        }
    }

    Ok(master_sb)
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
pub fn csum_data(buf: &[u8], csum_type: BtrfsCsumType) -> BtrfsCsum {
    match csum_type {
        BtrfsCsumType::CRC32 => csum_data_crc32(buf),
        _ => panic!("only crc32 checksums are implemented - could be a small project for you?"),
    }
}

fn csum_data_crc32(buf: &[u8]) -> [u8; BTRFS_CSUM_SIZE] {
    const CASTAGNOLI: Crc<u32> = Crc::<u32>::new(&CRC_32_ISCSI);
    let mut ret = [0_u8; BTRFS_CSUM_SIZE];
    let cs = CASTAGNOLI.checksum(buf).to_le_bytes();
    ret[..cs.len()].copy_from_slice(&cs[..]);
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

pub fn load_fs(paths: &Vec<PathBuf>) -> Result<FsInfo> {
    let mut fsid = None;
    let mut devid_map = HashMap::<LE64, Rc<DeviceInfo>>::new();
    let mut devuuid_map = HashMap::<BtrfsUuid, Rc<DeviceInfo>>::new();
    let mut master_sb: Option<btrfs_super_block> = None;
    let mut initial_chunks = Vec::new();
    for path in paths {
        println!("checking {}", path.display());
        let mf = MappedFile::open(path)?;
        let sb = load_sb(&mf)?;

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
            file: mf,
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

    Ok(FsInfo {
        fsid: fsid.unwrap(),
        devid_map,
        devuuid_map,
        master_sb: sb,
        bootstrap_chunks: initial_chunks,
    })
}

pub fn tree_root_offset(fs: &FsInfo, tree_id: u64) -> Option<u64> {
    let root = fs.master_sb.root;
    let search = NodeSearchOption {
        min_key: btrfs_disk_key {
            objectid: tree_id,
            item_type: BtrfsItemType::ROOT_ITEM,
            offset: 0,
        },
        max_key: btrfs_disk_key {
            objectid: tree_id,
            item_type: BtrfsItemType::ROOT_ITEM,
            offset: u64::MAX,
        },
        min_match: std::cmp::Ordering::Less,
        max_match: std::cmp::Ordering::Greater,
    };

    if let Some((leaf, data, _block_offset, _leaf_pos)) =
        BtrfsTreeIter::new(fs, root, search).next()
    {
        let btrfs_disk_key {
            objectid,
            item_type,
            offset,
        } = leaf.key;
        let size = leaf.size;

        assert_eq!(item_type, BtrfsItemType::ROOT_ITEM);
        assert_eq!(size as usize, std::mem::size_of::<btrfs_root_item>());
        let root_item = unsafe { &*((data.as_ptr()) as *const btrfs_root_item) };
        let tree_root = root_item.bytenr;
        println!(
            "leaf {} {item_type:?} {offset} data size {} tree root {tree_root}",
            fmt_treeid(objectid),
            size
        );
        return Some(tree_root);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32() {
        let input: [u8; 9] = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09];
        //checksum is little-endian order
        let expected: [u8; 4] = [0xf9, 0xb9, 0x14, 0x5a];
        let result = csum_data_crc32(&input);
        println!("{result:x?}");
        assert_eq!(expected, result[0..4]);
    }
}
