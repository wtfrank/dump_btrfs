pub const BTRFS_CSUM_SIZE: usize = 32;
const BTRFS_FSID_SIZE: usize = 16;
const BTRFS_UUID_SIZE: usize = 16;
pub const BTRFS_SUPER_INFO_OFFSET: usize = 65536;
pub const BTRFS_SUPER_INFO_SIZE: usize = 4096;

const BTRFS_SYSTEM_CHUNK_ARRAY_SIZE: usize = 2048;
const BTRFS_LABEL_SIZE: usize = 256;

pub const BTRFS_MAGIC: u64 = 0x4D5F53665248425F;
const BTRFS_NUM_BACKUP_ROOTS: usize = 4;

pub const BtrfsCsumType_CRC32: u8 = 0;
pub const BtrfsCsumType_XXHASH: u8 = 1;
pub const BtrfsCsumType_SHA256: u8 = 2;
pub const BtrfsCsumType_BLAKE2: u8 = 3;

/*
  repr(u16) will not work on big-endian architectures. We could work around this with target_endian confg so that we declare these values with swapped bytes on big-endian systems. But I'm not going to write code I'm not going to test.
*/
#[repr(u16)]
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub enum BtrfsCsumType {
    CRC32 = 0,
    XXHASH = 1,
    SHA256 = 2,
    BLAKE2 = 3,
}

//type LE64 = endian_types::Endian<u64, endian_types::LittleEndian>;
/// on-disc format is little-endian
pub type LE16 = u16;
pub type LE32 = u32;
pub type LE64 = u64;

pub type BtrfsCsum = [u8; BTRFS_CSUM_SIZE];

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct btrfs_super_block {
    pub csum: BtrfsCsum,
    pub fsid: [u8; BTRFS_FSID_SIZE],
    pub bytenr: LE64,
    pub flags: LE64,
    pub magic: LE64,
    pub generation: LE64,
    pub root: LE64,
    pub chunk_root: LE64,
    pub log_root: LE64,
    __unused_log_root_transid: LE64,
    pub total_bytes: LE64,
    pub bytes_used: LE64,
    pub root_dir_object_id: LE64,
    pub num_devices: LE64,
    pub sectorsize: LE32,
    pub nodesize: LE32,
    __unused_leafsize: LE32,
    pub stripesize: LE32,
    pub sys_chunk_array_size: LE32,
    pub chunk_root_generation: LE64,
    pub compat_flags: LE64,
    pub compat_ro_flags: LE64,
    pub incompat_flags: LE64,
    //pub csum_type: LE16,
    pub csum_type: BtrfsCsumType,
    pub root_level: u8,
    pub chunk_root_level: u8,
    pub log_root_level: u8,
    pub dev_item: btrfs_dev_item,
    pub label: [u8; BTRFS_LABEL_SIZE],
    pub cache_generation: LE64,
    pub uuid_tree_generation: LE64,
    pub metadata_uuid: [u8; BTRFS_FSID_SIZE],
    pub nr_global_roots: LE64,
    reserved: [LE64; 27],
    pub sys_chunk_array: [u8; BTRFS_SYSTEM_CHUNK_ARRAY_SIZE],
    pub super_roots: [btrfs_root_backup; BTRFS_NUM_BACKUP_ROOTS],
    padding: [u8; 565],
}
static_assertions::assert_eq_size!([u8; BTRFS_SUPER_INFO_SIZE], btrfs_super_block);

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct btrfs_root_backup {
    tree_root: LE64,
    tree_root_gen: LE64,

    chunk_root: LE64,
    chunk_root_gen: LE64,

    extent_root: LE64,
    extent_root_gen: LE64,

    fs_root: LE64,
    fs_root_gen: LE64,

    dev_root: LE64,
    dev_root_gen: LE64,

    csum_root: LE64,
    csum_root_gen: LE64,

    total_bytes: LE64,
    bytes_used: LE64,
    num_devices: LE64,

    unsed_64: [LE64; 4],

    tree_root_level: u8,
    chunk_root_level: u8,
    extent_root_level: u8,
    fs_root_level: u8,
    dev_root_level: u8,
    csum_root_level: u8,
    unused_8: [u8; 10],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct btrfs_dev_item {
    devid: LE64,
    total_bytes: LE64,
    bytes_used: LE64,
    io_align: LE32,
    io_width: LE32,
    sector_size: LE32,
    r#type: LE64,
    generation: LE64,
    start_offset: LE64,
    dev_group: LE32,
    seek_speed: u8,
    bandwidth: u8,
    uuid: [u8; BTRFS_UUID_SIZE],
    fsid: [u8; BTRFS_UUID_SIZE],
}

#[repr(C, packed)]
pub struct btrfs_header {
    csum: [u8; BTRFS_CSUM_SIZE],
    fsid: [u8; BTRFS_FSID_SIZE],
    bytenr: LE64,
    flags: LE64,

    chunk_tree_uuid: [u8; BTRFS_UUID_SIZE],
    generation: LE64,
    owner: LE64,
    nritems: LE32,
    level: u8,
}

#[repr(C, packed)]
pub struct btrfs_disk_key {
    objectid: LE64,
    r#type: u8,
    offset: LE64,
}