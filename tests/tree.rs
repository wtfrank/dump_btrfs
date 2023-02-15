use btrfs_kit::structures::*;

fn default_btrfs_dev_item() -> btrfs_dev_item {
    btrfs_dev_item {
        devid: 0,
        total_bytes: 0,
        bytes_used: 0,
        io_align: 0,
        io_width: 0,
        sector_size: 512,
        r#type: 0,
        generation: 0,
        start_offset: 0,
        dev_group: 0,
        seek_speed: 0,
        bandwidth: 9,
        uuid: [0; BTRFS_UUID_SIZE],
        fsid: [0; BTRFS_FSID_SIZE],
    }
}

fn default_btrfs_root_backup() -> btrfs_root_backup {
    btrfs_root_backup {
        tree_root: 0,
        tree_root_gen: 0,
        chunk_root: 0,
        chunk_root_gen: 0,
        extent_root: 0,
        extent_root_gen: 0,
        fs_root: 0,
        fs_root_gen: 0,
        dev_root: 0,
        dev_root_gen: 0,
        csum_root: 0,
        csum_root_gen: 0,
        total_bytes: 0,
        bytes_used: 0,
        num_devices: 0,
        unused_64: [0; 4],
        tree_root_level: 0,
        chunk_root_level: 0,
        extent_root_level: 0,
        fs_root_level: 0,
        dev_root_level: 0,
        csum_root_level: 0,
        unused_8: [0; 10],
    }
}

fn default_btrfs_superblock() -> btrfs_super_block {
    btrfs_super_block {
        csum: [0; BTRFS_CSUM_SIZE],
        fsid: [0; BTRFS_FSID_SIZE],
        bytenr: 0,
        flags: 0,
        magic: 0,
        generation: 0,
        root: 0,
        chunk_root: 0,
        log_root: 0,
        __unused_log_root_transid: 0,
        total_bytes: 0,
        bytes_used: 0,
        root_dir_object_id: 0,
        num_devices: 0,
        sectorsize: 0,
        nodesize: 0,
        __unused_leafsize: 0,
        stripesize: 0,
        sys_chunk_array_size: 0,
        chunk_root_generation: 0,
        compat_flags: 0,
        compat_ro_flags: 0,
        incompat_flags: 0,
        csum_type: BtrfsCsumType::CRC32,
        root_level: 0,
        chunk_root_level: 0,
        log_root_level: 0,
        dev_item: default_btrfs_dev_item(),
        label: [0; BTRFS_LABEL_SIZE],
        cache_generation: 0,
        uuid_tree_generation: 0,
        metadata_uuid: [0; BTRFS_FSID_SIZE],
        nr_global_roots: 0,
        reserved: [0; 27],
        sys_chunk_array: [0; BTRFS_SYSTEM_CHUNK_ARRAY_SIZE],
        super_roots: [default_btrfs_root_backup(); BTRFS_NUM_BACKUP_ROOTS],
        padding: [0_u8; 565],
    }
}

#[test]
fn zero_depth_tree() {
    let mut sb = default_btrfs_superblock();
    sb.num_devices = 1;
}
