use crate::types::*;
use anyhow::*;
use crc::{Crc, CRC_32_ISCSI};
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;

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

fn load_sb(path: &std::path::PathBuf) -> anyhow::Result<()> {
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(BTRFS_SUPER_INFO_OFFSET.try_into()?))?;
    /*
    let mut buf = [0_u8; BTRFS_SUPER_INFO_SIZE];
    f.read_exact(&mut buf)?;
    let sb:btrfs_super_block = unsafe {
      std::mem::transmute::<[u8;BTRFS_SUPER_INFO_SIZE],btrfs_super_block>(buf)
    };*/
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

    println!("sb loaded ok");
    Ok(())
}

/* the checksums range from 4-32 bytes depending on the algorithm in use. For simplicity we'll always return a 32 byte buffer, but this could be improved upon */
fn csum_data(buf: &[u8], csum_type: BtrfsCsumType) -> [u8; BTRFS_CSUM_SIZE] {
    match csum_type {
        BtrfsCsumType::CRC32 => csum_data_crc32(buf),
        _ => panic!("only crc32 checksums are implemented"),
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

pub fn dump(paths: &Vec<std::path::PathBuf>) -> anyhow::Result<()> {
    for path in paths {
        println!("checking {}", path.display());
        load_sb(path)?;
    }

    Ok(())
}
