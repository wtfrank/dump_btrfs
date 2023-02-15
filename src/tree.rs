use crate::btrfs::*;
use crate::btrfs_node::*;
use crate::structures::*;

use log::{debug, trace};
use std::cmp::Ordering;

/// Functions/structures to search or iterate through a btrfs tree

#[derive(Clone, Copy)]
pub struct NodeSearchOption {
    pub min_key: btrfs_disk_key,
    pub max_key: btrfs_disk_key,
    // where there is no node exactly matching the key, if Ordering is Less, then the last node to the left
    // of the search key will match. If Ordering is Greater, than the first node to the right of the search
    // key will match.
    // TODO:
    pub min_match: Ordering,
    pub max_match: Ordering,
}

fn cmp_key(left: &btrfs_disk_key, right: &btrfs_disk_key) -> Ordering {
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
    pub fn new(fs: &FsInfo, root: LE64, options: NodeSearchOption) -> BtrfsTreeIter {
        let objectid = options.min_key.objectid;
        let item_type = options.min_key.item_type;
        let offset = options.min_key.offset;
        assert_ne!(
            cmp_key(&options.min_key, &options.max_key),
            Ordering::Greater
        );
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
        let mut internal_node = btrfs_internal_node(self.fs, self.root).ok()?;
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
            let mut left_key;
            let mut right_key;
            loop {
                left_key = internal_node.next();
                if left_key.is_none() {
                    break;
                }
                right_key = internal_node.peek();

                let lk = left_key.unwrap();
                let btrfs_disk_key {
                    objectid: lk_oid,
                    item_type: lk_type,
                    offset: lk_offset,
                } = lk.key;
                let cmp_min = cmp_key(&lk.key, &self.options.min_key);
                let cmp_max = cmp_key(&lk.key, &self.options.max_key);

                trace!(
                    "Evaluating internal node key {} {:?} {}. min_key {:?}, max_key {:?}",
                    lk_oid,
                    lk_type,
                    lk_offset,
                    cmp_min,
                    cmp_max,
                );

                match cmp_min {
                    Ordering::Greater => match cmp_max {
                        Ordering::Greater => {
                            debug!("internal node is greater than search range");
                            return None;
                        }
                        _ => {
                            node_stack.push(internal_node);
                            internal_node = btrfs_internal_node(self.fs, lk.blockptr).ok()?;
                            break;
                        }
                    },
                    Ordering::Equal => {
                        node_stack.push(internal_node);
                        internal_node = btrfs_internal_node(self.fs, lk.blockptr).ok()?;
                        break;
                    }
                    Ordering::Less => match right_key {
                        None => {
                            trace!("right key is None");
                            //if there is no key to the right then our key could be within the child nodes
                            node_stack.push(internal_node);
                            internal_node = btrfs_internal_node(self.fs, lk.blockptr).ok()?;
                            break;
                        }
                        Some(rk) => {
                            let cmp_rk = cmp_key(&rk.key, &self.options.min_key);
                            trace!(
                                "right {:?} is {cmp_rk:?} min_key {:?}",
                                rk.key,
                                self.options.min_key
                            );
                            if cmp_rk == Ordering::Greater {
                                node_stack.push(internal_node);
                                internal_node = btrfs_internal_node(self.fs, lk.blockptr).ok()?;
                                break;
                            }
                            //otherwise we try the next key in the node
                        }
                    },
                }
            }
        }

        debug!("reached leaf node with path length {}", node_stack.len());
        let leaf_node = internal_node.as_leaf_node();
        Some((node_stack, leaf_node))
    }
}

/*
 * use cases:
 * - I want to find the leaf matching this exact key
 * - I want to find the leaf containing the range that contains the offset in this key
 * - I want to iterate from the key I've found to the last one less than or equal to the max
 */

impl<'a> Iterator for BtrfsTreeIter<'a> {
    type Item = (&'a btrfs_item, &'a [u8], u64, u32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_leaf_node.is_none() {
            let (path, leaf_node) = self.find_key()?;
            self.cur_leaf_node = Some(leaf_node);
            self.cur_leaf_index = 0;
            self.internal_node_stack = path;
        }

        if let Some(ln) = self.cur_leaf_node.as_mut() {
            let mut left_leaf;
            let mut right_leaf;
            loop {
                left_leaf = ln.next();
                if left_leaf.is_none() {
                    break;
                }
                right_leaf = ln.peek();

                let ll = left_leaf.unwrap();
                let cmp_min = cmp_key(&ll.0.key, &self.options.min_key);
                let cmp_max = cmp_key(&ll.0.key, &self.options.max_key);
                trace!(
                    "ll {:?} cmp_min: {:?} cmp_max: {:?}",
                    ll.0.key,
                    cmp_min,
                    cmp_max
                );
                match cmp_min {
                    Ordering::Greater => match cmp_max {
                        Ordering::Greater => return None,
                        _ => return Some(ll),
                    },
                    Ordering::Equal => return Some(ll),
                    _ => match right_leaf {
                        None => {
                            trace!("right leaf was None");
                            return Some(ll);
                        }
                        Some(rl) => {
                            let cmp_rk = cmp_key(&rl.0.key, &self.options.min_key);
                            trace!(
                                "rk {:?} was {:?} min_key {:?}",
                                rl.0.key,
                                cmp_rk,
                                self.options.min_key
                            );
                            if cmp_rk == Ordering::Greater {
                                return Some(ll);
                            }
                        }
                    },
                }
            }
        }
        debug!("reached end of leaf nodes - opening parent node");
        //go up to parent internal node and continue
        //this is pretty easy because we're after the left-most leaf_node
        let mut subtree_start = None;
        while !self.internal_node_stack.is_empty() {
            let parent_internal = self.internal_node_stack.pop()?;
            let next_child = parent_internal.peek();
            if next_child.is_none() {
                continue; //try the parent's parent if it exists
            }
            subtree_start = Some(parent_internal);
            break;
        }
        if subtree_start.is_none() {
            debug!("reached end of leaf nodes - no parent nodes available");
            return None;
        }

        //we now descend the subtree, pushing each node onto the stack
        //(probably pushing back the node we just popped off, with iterator incremented)
        let mut internal_node = subtree_start.unwrap();
        while internal_node.header().level != 0 {
            let child = internal_node.next()?; //every internal node has at least 1 entry
            self.internal_node_stack.push(internal_node);
            internal_node = btrfs_internal_node(self.fs, child.blockptr).ok()?;
        }
        let leaf_node = internal_node.as_leaf_node();
        self.cur_leaf_node = Some(leaf_node);
        self.cur_leaf_index = 0;
        //we recurse to continue iterating from the leaf node we just set up
        self.next()
    }
}
