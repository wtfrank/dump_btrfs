pub const BTRFS_CSUM_SIZE: usize = 32;
pub const BTRFS_FSID_SIZE: usize = 16;
pub const BTRFS_UUID_SIZE: usize = 16;
pub const BTRFS_SUPER_INFO_OFFSET: usize = 65536;
pub const BTRFS_SUPER_INFO_SIZE: usize = 4096;

pub const BTRFS_SUPER_MIRROR_MAX: usize = 3;
pub const BTRFS_SUPER_MIRROR_SHIFT: usize = 12;

pub const BTRFS_SYSTEM_CHUNK_ARRAY_SIZE: usize = 2048;
pub const BTRFS_LABEL_SIZE: usize = 256;

pub const BTRFS_MAGIC: u64 = 0x4D5F53665248425F;
pub const BTRFS_NUM_BACKUP_ROOTS: usize = 4;

pub const BTRFS_ROOT_TREE_OBJECTID: u64 = 1;
pub const BTRFS_EXTENT_TREE_OBJECTID: u64 = 2;
pub const BTRFS_CHUNK_TREE_OBJECTID: u64 = 3;
pub const BTRFS_DEV_TREE_OBJECTID: u64 = 4;
pub const BTRFS_FS_TREE_OBJECTID: u64 = 5;
pub const BTRFS_ROOT_TREE_DIR_OBJECTID: u64 = 6;
pub const BTRFS_CSUM_TREE_OBJECTID: u64 = 7;
pub const BTRFS_QUOTA_TREE_OBJECTID: u64 = 8;
pub const BTRFS_UUID_TREE_OBJECTID: u64 = 9;
pub const BTRFS_FREE_SPACE_TREE_OBJECTID: u64 = 10;
pub const BTRFS_BLOCK_GROUP_TREE_OBJECTID: u64 = 11;

pub const BTRFS_DEV_STATS_OBJECTID: u64 = 0;
pub const BTRFS_BALANCE_OBJECTID: u64 = -4_i64 as u64;
pub const BTRFS_ORPHAN_OBJECTID: u64 = -5_i64 as u64;
pub const BTRFS_TREE_LOG_OBJECTID: u64 = -6_i64 as u64;
pub const BTRFS_TREE_LOG_FIXUP_OBJECTID: u64 = -7_i64 as u64;
pub const BTRFS_TREE_RELOC_OBJECTID: u64 = -8_i64 as u64;
pub const BTRFS_DATA_RELOC_TREE_OBJECTID: u64 = -9_i64 as u64;
pub const BTRFS_EXTENT_CSUM_OBJECTID: u64 = -10_i64 as u64;
pub const BTRFS_FREE_SPACE_OBJECTID: u64 = -11_i64 as u64;
pub const BTRFS_FREE_INO_OBJECTID: u64 = -12_i64 as u64;
pub const BTRFS_MULTIPLE_OBJECTIDS: u64 = -255_i64 as u64;

pub const BTRFS_FIRST_CHUNK_TREE_OBJECTID: u64 = 256;

/*
  repr(u16) will not work on big-endian architectures. We could work around this with target_endian confg so that we declare these values with swapped bytes on big-endian systems. But I'm not going to write code I'm not going to test.
*/
#[repr(u16)]
#[derive(Clone, Copy)]
#[allow(dead_code, non_camel_case_types)]
pub enum BtrfsCsumType {
    CRC32 = 0,
    XXHASH = 1,
    SHA256 = 2,
    BLAKE2 = 3,
}

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[allow(dead_code, non_camel_case_types)]
pub enum BtrfsItemType {
    MIN = 0x00, //to facilitate searching through any possible byte value
    INODE_ITEM = 0x01,
    INODE_REF = 0x0c,
    INODE_EXTREF = 0x0d,
    XATTR_ITEM = 0x18,
    VERITY_DESC_ITEM = 0x24,
    VERITY_MERKLE_ITEM = 0x25,
    ORPHAN_ITEM = 0x30,
    DIR_LOG_ITEM = 0x3c,
    DIR_LOG_INDEX = 0x48,
    DIR_ITEM = 0x54,
    DIR_INDEX = 0x60,
    EXTENT_DATA = 0x6c,
    CSUM_ITEM = 0x78,
    EXTENT_CSUM = 0x80,
    ROOT_ITEM = 0x84,
    ROOT_BACKREF = 0x90,
    ROOT_REF = 0x9c,
    EXTENT_ITEM = 0xa8,
    METADATA_ITEM = 0xa9,
    TREE_BLOCK_REF = 0xb0,
    EXTENT_DATA_REF = 0xb2,
    EXTENT_REF_V0 = 0xb4,
    SHARED_BLOCK_REF = 0xb6,
    SHARED_DATA_REF = 0xb8,
    BLOCK_GROUP_ITEM = 0xc0,
    FREE_SPACE_INFO = 0xc6,
    FREE_SPACE_EXTENT = 0xc7,
    FREE_SPACE_BITMAP = 0xc8,
    DEV_EXTENT = 0xcc,
    DEV_ITEM = 0xd8,
    CHUNK_ITEM = 0xe4,
    QGROUP_STATUS = 0xf0,
    QGROUP_INFO = 0xf2,
    QGROUP_LIMIT = 0xf4,
    QGROUP_RELATION = 0xf6,
    TEMPORARY_ITEM = 0xf8,
    PERSISTENT_ITEM = 0xf9,
    DEV_REPLACE = 0xfa,
    UUID_KEY_SUBVOL = 0xfb,
    UUID_KEY_RECEIVED_SUBVOL = 0xfc,
    STRING_ITEM = 0xfd,
    MAX = 0xff, //to facilitate searching through any possible byte value
}

//type LE64 = endian_types::Endian<u64, endian_types::LittleEndian>;
/// on-disc format is little-endian
pub type LE16 = u16;
pub type LE32 = u32;
pub type LE64 = u64;

pub type BtrfsCsum = [u8; BTRFS_CSUM_SIZE];
pub type BtrfsUuid = [u8; BTRFS_UUID_SIZE];
pub type BtrfsFsid = [u8; BTRFS_FSID_SIZE];

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct btrfs_super_block {
    pub csum: BtrfsCsum,
    pub fsid: BtrfsFsid,
    pub bytenr: LE64,
    pub flags: LE64,
    pub magic: LE64,
    pub generation: LE64,
    pub root: LE64,
    pub chunk_root: LE64,
    pub log_root: LE64,
    pub __unused_log_root_transid: LE64,
    pub total_bytes: LE64,
    pub bytes_used: LE64,
    pub root_dir_object_id: LE64,
    pub num_devices: LE64,
    pub sectorsize: LE32,
    pub nodesize: LE32,
    pub __unused_leafsize: LE32,
    pub stripesize: LE32,
    pub sys_chunk_array_size: LE32,
    pub chunk_root_generation: LE64,
    pub compat_flags: LE64,
    pub compat_ro_flags: LE64,
    pub incompat_flags: LE64,
    pub csum_type: BtrfsCsumType,
    pub root_level: u8,
    pub chunk_root_level: u8,
    pub log_root_level: u8,
    pub dev_item: btrfs_dev_item,
    pub label: [u8; BTRFS_LABEL_SIZE],
    pub cache_generation: LE64,
    pub uuid_tree_generation: LE64,
    pub metadata_uuid: BtrfsFsid, //fsid vs uuid as per ctree.h
    pub nr_global_roots: LE64,
    pub reserved: [LE64; 27],
    pub sys_chunk_array: [u8; BTRFS_SYSTEM_CHUNK_ARRAY_SIZE],
    pub super_roots: [btrfs_root_backup; BTRFS_NUM_BACKUP_ROOTS],
    pub padding: [u8; 565],
}
static_assertions::assert_eq_size!([u8; BTRFS_SUPER_INFO_SIZE], btrfs_super_block);

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct btrfs_root_backup {
    pub tree_root: LE64,
    pub tree_root_gen: LE64,

    pub chunk_root: LE64,
    pub chunk_root_gen: LE64,

    pub extent_root: LE64,
    pub extent_root_gen: LE64,

    pub fs_root: LE64,
    pub fs_root_gen: LE64,

    pub dev_root: LE64,
    pub dev_root_gen: LE64,

    pub csum_root: LE64,
    pub csum_root_gen: LE64,

    pub total_bytes: LE64,
    pub bytes_used: LE64,
    pub num_devices: LE64,

    pub unused_64: [LE64; 4],

    pub tree_root_level: u8,
    pub chunk_root_level: u8,
    pub extent_root_level: u8,
    pub fs_root_level: u8,
    pub dev_root_level: u8,
    pub csum_root_level: u8,
    pub unused_8: [u8; 10],
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct btrfs_dev_item {
    pub devid: LE64,
    pub total_bytes: LE64,
    pub bytes_used: LE64,
    pub io_align: LE32,
    pub io_width: LE32,
    pub sector_size: LE32,
    pub r#type: LE64,
    pub generation: LE64,
    pub start_offset: LE64,
    pub dev_group: LE32,
    pub seek_speed: u8,
    pub bandwidth: u8,
    pub uuid: BtrfsUuid,
    pub fsid: BtrfsFsid,
}

/* header is stored at the start of every tree node */
#[repr(C, packed)]
pub struct btrfs_header {
    pub csum: BtrfsCsum,
    pub fsid: BtrfsFsid,
    pub bytenr: LE64,
    pub flags: LE64,

    pub chunk_tree_uuid: BtrfsUuid,
    pub generation: LE64,
    pub owner: LE64,
    pub nritems: LE32,
    pub level: u8,
}

/* leaf nodes are full of btrfs_items, and data */
#[repr(C, packed)]
pub struct btrfs_item {
    pub key: btrfs_disk_key,
    pub offset: LE32, //counting starts at end of btrfs_header
    pub size: LE32,
}

/* non-leaf nodes are full of btrfs_key_ptrs */
#[repr(C, packed)]
pub struct btrfs_key_ptr {
    pub key: btrfs_disk_key,
    pub blockptr: LE64,
    pub generation: LE64,
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct btrfs_disk_key {
    pub objectid: LE64,
    pub item_type: BtrfsItemType,
    pub offset: LE64,
}

impl std::fmt::Debug for btrfs_disk_key {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let objectid = self.objectid;
        let item_type = self.item_type;
        let offset = self.offset;
        write!(f, "{:?} {:?} {:?}", objectid, item_type, offset)
    }
}

#[repr(C, packed)]
pub struct btrfs_stripe {
    pub devid: LE64,
    pub offset: LE64,
    pub dev_uuid: BtrfsUuid,
}

#[repr(C, packed)]
pub struct btrfs_chunk {
    pub length: LE64,
    pub owner: LE64,
    pub stripe_len: LE64,
    pub r#type: LE64,
    pub io_align: LE32,
    pub io_width: LE32,
    pub sector_size: LE32,
    pub num_stripes: LE16,
    pub sub_stripes: LE16,
}

#[repr(C, packed)]
pub struct btrfs_timespec {
    pub sec: LE64,
    pub nsec: LE32,
}

#[repr(C, packed)]
pub struct btrfs_inode_item {
    pub generation: LE64,
    pub transid: LE64,
    pub size: LE64,
    pub nbytes: LE64,
    pub block_group: LE64,
    pub nlink: LE32,
    pub uid: LE32,
    pub gid: LE32,
    pub mode: LE32,
    pub rdev: LE64,
    pub flags: LE64,

    pub sequence: LE64,
    pub __reserved: [LE64; 4],
    pub atime: btrfs_timespec,
    pub ctime: btrfs_timespec,
    pub mtime: btrfs_timespec,
    pub otime: btrfs_timespec,
}

/* there was an older version of this structure which I'm ignoring */
#[repr(C, packed)]
pub struct btrfs_root_item {
    pub inode: btrfs_inode_item,
    pub generation: LE64,
    pub root_dirid: LE64,
    pub bytenr: LE64,
    pub byte_limit: LE64,
    pub bytes_used: LE64,
    pub last_snapshot: LE64,
    pub flags: LE64,
    pub refs: LE32,
    pub drop_progress: btrfs_disk_key,
    pub drop_level: u8,
    pub level: u8,
    pub generation_v2: LE64,
    pub uuid: BtrfsUuid,
    pub parent_uuid: BtrfsUuid,
    pub received_uuid: BtrfsUuid,
    pub ctransid: LE64,
    pub otransid: LE64,
    pub stransid: LE64,
    pub rtransid: LE64,
    pub ctime: btrfs_timespec,
    pub otime: btrfs_timespec,
    pub stime: btrfs_timespec,
    pub rtime: btrfs_timespec,
    pub global_tree_id: LE64,
    pub __reserved: [LE64; 7],
}

#[repr(C, packed)]
pub struct btrfs_root_ref {
    pub dirid: LE64,
    pub sequence: LE64,
    pub name_len: LE16,
    /* the name follows here */
}

#[repr(C, packed)]
pub struct btrfs_extent_item {
	pub refs: LE64,
	pub generation: LE64,
	pub flags: LE64,
}
