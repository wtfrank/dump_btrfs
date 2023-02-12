use crate::address::*;
use crate::btrfs::*;
use crate::btrfs_node::*;
use crate::structures::*;

use log::debug;
use std::cmp::Ordering;

/// Functions/structures to search or iterate through a btrfs tree

#[derive(Clone, Copy)]
pub struct NodeSearchOption {
    pub min_key: btrfs_disk_key,
    pub max_key: btrfs_disk_key,
    /*
    pub min_object_id: LE64,
    pub max_object_id: LE64,
    pub min_item_type: BtrfsItemType,
    pub max_item_type: BtrfsItemType,
    pub min_offset: LE64,
    pub max_offset: LE64,*/
    // where there is no node exactly matching the key, if Ordering is Less, then the last node to the left
    // of the search key will match. If Ordering is Greater, than the first node to the right of the search
    // key will match.
    // TODO:
    pub min_match: Ordering,
    pub max_match: Ordering,
}

/// TODO a simplified comparison which just checks for matches against min and ignores max
/// Separating the search into finding start node, then deciding whether or not to iterate to the
/// following node may change what's needed from this function.
fn cmp_key_option(left: &btrfs_disk_key, right: &btrfs_disk_key) -> Ordering {
    if left.objectid < right.objectid {
        Ordering::Less
    } else if left.objectid > right.objectid {
        Ordering::Greater
    } else if left.item_type < right.item_type {
        Ordering::Less
    } else if left.item_type > right.item_type {
        Ordering::Greater
    } else if left.offset < right.offset {
        Ordering::Less
    } else if left.offset > right.offset {
        Ordering::Greater
    } else {
        Ordering::Equal
    }
}

pub struct BtrfsTreeIter<'a> {
    fs: &'a FsInfo,
    root: LE64,
    options: NodeSearchOption,
    // how do we track progress - current leaf?
    // usually we are working through leaf items in a single node so we want
    // this to be fast, and it's ok if we have to do a slower operation to start
    // a new node. if we have to look up chunk addresses every next() it will be a bit
    // slow so we should save a reference to an entire block.
    cur_leaf_node: Option<BtrfsLeafNodeIter<'a>>,
    cur_leaf_index: usize,
    internal_node_stack: Vec<BtrfsInternalNodeIter<'a>>,
}

impl<'a> BtrfsTreeIter<'a> {
    /// TODO this looks for leaf entry that exactly matches the min value of options,
    /// while the max value of options are ignored.
    pub fn new(fs: &FsInfo, root: LE64, options: NodeSearchOption) -> BtrfsTreeIter {
        let objectid = options.min_key.objectid;
        let item_type = options.min_key.item_type;
        let offset = options.min_key.offset;
        debug!(
            "new iterator: root {}, oid {}, type {:?}, offset {}",
            root, objectid, item_type, offset
        );
        //do we know at this point whether we're a leaf node or not? It would be helpful
        //if we did.
        BtrfsTreeIter {
            fs,
            root,
            options,
            cur_leaf_node: None,
            cur_leaf_index: 0,
            internal_node_stack: Vec::new(),
        }
    }

    //Iterator trait helper function (maybe useful outside iterator with a bit of rework)
    fn find_key(&self) -> Option<(Vec<BtrfsInternalNodeIter<'a>>, BtrfsLeafNodeIter<'a>)> {
        let mut internal_block = load_virt_block(self.fs, self.root).ok()?;
        let mut internal_node = btrfs_internal_node(internal_block);
        let mut node_stack = Vec::new();
        debug!("starting search at depth {}", internal_node.header().level);
        //let header = load_virt::<btrfs_header>(self.fs, self.root).ok()?;
        //TODO: binary search would be more efficient than iterating over every element in a node as
        //btrfs nodes are wide in order to reduce tree depth.
        while internal_node.header().level != 0 {
            // if our key is to the left of all we skip (nothing in this node)
            // if our key is between we go down
            // if our key is to the right of all we also go down
            //
            // if we are only searching for a single item, this is easy
            // TODO: search for a range which probably means we need to store a
            // stack of node iterators that we're working through.
            let mut left_key;
            let mut right_key = internal_node.next();
            while right_key.is_some() {
                left_key = right_key;
                right_key = internal_node.next();

                let lk = left_key.unwrap();
                let cmp = cmp_key_option(&lk.key, &self.options.min_key);

                if cmp == Ordering::Greater {
                    debug!("no match in internal node");
                    return None;
                }
                if cmp == Ordering::Equal {
                    node_stack.push(internal_node);
                    internal_block = load_virt_block(self.fs, lk.blockptr).ok()?;
                    internal_node = btrfs_internal_node(internal_block);
                    break;
                }
                match right_key {
                    None => {
                        node_stack.push(internal_node);
                        internal_block = load_virt_block(self.fs, lk.blockptr).ok()?;
                        internal_node = btrfs_internal_node(internal_block);
                        break;
                    }
                    Some(rk) => {
                        if cmp_key_option(&rk.key, &self.options.min_key) == Ordering::Greater {
                            node_stack.push(internal_node);
                            internal_block = load_virt_block(self.fs, lk.blockptr).ok()?;
                            internal_node = btrfs_internal_node(internal_block);
                            break;
                        }
                        //otherwise we try the next key in the node
                    }
                }
            }
        }

        //we now have reached the leaf (TODO: a leaf in the range)

        debug!("reached leaf node with path length {}", node_stack.len());
        let leaf_node =
            btrfs_leaf_node(load_virt_block(self.fs, internal_node.header().bytenr).ok()?);
        //TODO: find leaf based on options
        Some((node_stack, leaf_node))
    }
}

/* TODO: need to split up the search:
 * 1) find key, either exact match, last before or first after
 * 2) given key + node_stack + index, find next key
 *
 * use cases:
 * - I want to find the leaf matching this exact key
 * - I want to find the leaf containing the range that contains the offset in this key
 * - I want to iterate from the key I've found to the last one less than or equal to the max
 */

impl<'a> Iterator for BtrfsTreeIter<'a> {
    type Item = (&'a btrfs_item, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_leaf_node.is_none() {
            let (path, leaf_node) = self.find_key()?;
            self.cur_leaf_node = Some(leaf_node);
            self.cur_leaf_index = 0;
            self.internal_node_stack = path;
        }

        if let Some(ln) = self.cur_leaf_node.as_mut() {
            let mut left_leaf;
            let mut right_leaf = ln.next();
            while right_leaf.is_some() {
                left_leaf = right_leaf;
                right_leaf = ln.next();
                let ll = left_leaf.unwrap();
                let ordering = cmp_key_option(&ll.0.key, &self.options.min_key);
                if ordering == Ordering::Greater {
                    return None;
                }
                if ordering == Ordering::Equal {
                    return Some(ll);
                }
                match right_leaf {
                    None => {
                        return Some(ll);
                    }
                    Some(rl) => {
                        if cmp_key_option(&rl.0.key, &self.options.min_key) == Ordering::Greater {
                            return Some(ll);
                        }
                    }
                }
            }
        }

        None
    }
}
