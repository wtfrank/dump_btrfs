an under-featured rust library to retrive and interpret parts of btrfs trees

While btrfsprogs has some ergonomic features which allow the user to specify mount points of mounted filesystems, block devices or fsids of a single device in the filesystem, dump\_btrfs is unergonomic and requires you to specify every block device in the filesystem.

REFERENCES
https://btrfs.wiki.kernel.org/index.php/Btrfs\_design
https://btrfs.wiki.kernel.org/index.php/Btree\_Items
https://btrfs.wiki.kernel.org/index.php/Data\_Structures
https://btrfs.wiki.kernel.org/index.php/On-disk\_Format

KNOWN ISSUES
* chunk stripe code is probably incorrect for raid0, raid10 etc. Tested in raid1 only.
* probably breaks on big-endian systems
* cannot handle filesystems larger than isize::MAX (i.e. on 32-bit linux only handles 2GB filesystems) due to simple memory-mapping
