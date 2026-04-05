//! Generic ID-bearing tree with cursor-based navigation.
//!
//! This crate provides two main types:
//!
//! - [`Tree<T>`] stores a rooted multi-way tree where every node carries a
//!   unique `u64` id and a user-defined value of type `T`.  An internal
//!   `HashMap` index makes id-based lookup O(1).
//! - [`CursoredTree<T>`] wraps a `Tree<T>` and tracks a *current position*
//!   via an index path from the root.  Navigation helpers (`go_parent`,
//!   `go_child`, `push`, ...) update the cursor atomically.
//!
//! Pruning, subtree removal, DFS iteration, and flat-list conversion are
//! available on both types.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::ops::Deref;

// ---------------------------------------------------------------------------
// Node
// ---------------------------------------------------------------------------

/// Single node in the tree.
///
/// Fields are private.  Use the accessor methods to read or mutate them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node<T> {
    id: u64,
    depth: usize,
    value: T,
    children: Vec<Node<T>>,
}

impl<T> Node<T> {
    fn new(id: u64, depth: usize, value: T) -> Self {
        Self {
            id,
            depth,
            value,
            children: Vec::new(),
        }
    }

    /// Unique identifier assigned at creation time.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Distance from the root (root = 0).
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Immutable reference to the stored value.
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Mutable reference to the stored value.
    pub fn value_mut(&mut self) -> &mut T {
        &mut self.value
    }

    /// Child nodes in insertion order.
    pub fn children(&self) -> &[Node<T>] {
        &self.children
    }

    /// Mutable access to the children vector.
    pub fn children_mut(&mut self) -> &mut Vec<Node<T>> {
        &mut self.children
    }

    /// Number of direct children.
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// `true` when the node has no children.
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

// ---------------------------------------------------------------------------
// ParentNotFound
// ---------------------------------------------------------------------------

/// Error returned by [`Tree::push_child`] when the specified parent id does
/// not exist in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParentNotFound(u64);

impl ParentNotFound {
    /// The parent id that was not found.
    pub fn parent_id(&self) -> u64 {
        self.0
    }
}

impl fmt::Display for ParentNotFound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "parent node {} not found in tree", self.0)
    }
}

impl Error for ParentNotFound {}

// ---------------------------------------------------------------------------
// PrunePolicy
// ---------------------------------------------------------------------------

/// Policy for [`Tree::prune`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrunePolicy {
    /// Keep the newest `n` children per node, dropping older ones.
    KeepLastN(usize),
    /// Remove all nodes at depth >= `limit`.
    KeepDepthUnder(usize),
}

// ---------------------------------------------------------------------------
// Tree
// ---------------------------------------------------------------------------

/// Rooted multi-way tree with O(1) id lookup.
///
/// Every node has a unique auto-incrementing `u64` id (root = 0).
/// An internal `HashMap<u64, Vec<usize>>` maps each id to its path
/// (sequence of child indices from the root), so [`find`](Self::find) and
/// [`find_path_to`](Self::find_path_to) are O(1) amortised.
#[derive(Debug, Clone)]
pub struct Tree<T> {
    root: Node<T>,
    next_id: u64,
    index: HashMap<u64, Vec<usize>>,
}

impl<T> Tree<T> {
    /// Create a tree with a single root node (id = 0).
    pub fn new(root_value: T) -> Self {
        let root = Node::new(0, 0, root_value);
        let mut index = HashMap::new();
        index.insert(0, Vec::new());
        Self {
            root,
            next_id: 1,
            index,
        }
    }

    /// Reference to the root node.
    pub fn root(&self) -> &Node<T> {
        &self.root
    }

    /// Mutable reference to the root node.
    pub fn root_mut(&mut self) -> &mut Node<T> {
        &mut self.root
    }

    /// The id that will be assigned to the next inserted node.
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    /// Append a child to the node identified by `parent_id`.
    ///
    /// Returns the newly assigned id, or [`ParentNotFound`] when
    /// `parent_id` does not exist in the tree.
    pub fn push_child(&mut self, parent_id: u64, value: T) -> Result<u64, ParentNotFound> {
        let parent_path = self
            .index
            .get(&parent_id)
            .cloned()
            .ok_or(ParentNotFound(parent_id))?;

        let depth = self
            .node_at_path(&parent_path)
            .expect("index points to existing node")
            .depth()
            + 1;
        let child_index = self
            .node_at_path(&parent_path)
            .expect("index points to existing node")
            .child_count();

        let child_id = self.next_id;
        let parent = self
            .node_mut_at_path(&parent_path)
            .expect("index points to existing node");
        parent.children.push(Node::new(child_id, depth, value));
        self.next_id += 1;

        let mut child_path = parent_path;
        child_path.push(child_index);
        self.index.insert(child_id, child_path);

        Ok(child_id)
    }

    /// Look up a node by id.  O(1) amortised.
    pub fn find(&self, id: u64) -> Option<&Node<T>> {
        let path = self.index.get(&id)?;
        self.node_at_path(path)
    }

    /// Mutable look up by id.  O(1) amortised.
    pub fn find_mut(&mut self, id: u64) -> Option<&mut Node<T>> {
        let path = self.index.get(&id)?.clone();
        self.node_mut_at_path(&path)
    }

    /// Return the index path from the root to the given id.  O(1) amortised.
    pub fn find_path_to(&self, id: u64) -> Option<Vec<usize>> {
        self.index.get(&id).cloned()
    }

    /// Depth-first iterator yielding `(depth, &Node<T>)` pairs.
    pub fn dfs(&self) -> DfsIter<'_, T> {
        DfsIter {
            stack: vec![(0, &self.root)],
        }
    }

    /// Collect [`dfs`](Self::dfs) into a `Vec`.
    pub fn flatten(&self) -> Vec<(usize, &Node<T>)> {
        self.dfs().collect()
    }

    /// Remove a subtree rooted at `id` and return its root node.
    ///
    /// Returns `None` when `id` is the tree root (cannot remove) or does not
    /// exist.  Remaining sibling indices are compacted and the internal index
    /// is rebuilt.
    pub fn remove_subtree(&mut self, id: u64) -> Option<Node<T>> {
        let path = self.find_path_to(id)?;
        if path.is_empty() {
            return None; // root
        }
        let mut parent_path = path;
        let child_index = parent_path.pop()?;
        let parent = self.node_mut_at_path(&parent_path)?;
        let removed = parent.children.remove(child_index);
        self.rebuild_index();
        Some(removed)
    }

    /// Apply a pruning policy to the entire tree.
    ///
    /// The internal index is rebuilt after pruning.  No nodes are protected:
    /// if you need to preserve a specific path (e.g. the current cursor),
    /// use [`CursoredTree::prune`] which automatically repairs the cursor.
    pub fn prune(&mut self, policy: PrunePolicy) {
        match policy {
            PrunePolicy::KeepLastN(n) => Self::prune_keep_last_n(&mut self.root, n),
            PrunePolicy::KeepDepthUnder(d) => Self::prune_keep_depth_under(&mut self.root, d),
        }
        self.rebuild_index();
    }

    // -- internal helpers ---------------------------------------------------

    fn node_at_path(&self, path: &[usize]) -> Option<&Node<T>> {
        let mut node = &self.root;
        for &i in path {
            node = node.children.get(i)?;
        }
        Some(node)
    }

    fn node_mut_at_path(&mut self, path: &[usize]) -> Option<&mut Node<T>> {
        let mut node = &mut self.root;
        for &i in path {
            node = node.children.get_mut(i)?;
        }
        Some(node)
    }

    fn rebuild_index(&mut self) {
        let mut index = HashMap::new();
        let mut path = Vec::new();
        Self::rebuild_from_node(&mut self.root, &mut index, &mut path, 0);
        self.index = index;
    }

    fn rebuild_from_node(
        node: &mut Node<T>,
        index: &mut HashMap<u64, Vec<usize>>,
        path: &mut Vec<usize>,
        depth: usize,
    ) {
        node.depth = depth;
        index.insert(node.id, path.clone());
        for (i, child) in node.children.iter_mut().enumerate() {
            path.push(i);
            Self::rebuild_from_node(child, index, path, depth + 1);
            path.pop();
        }
    }

    fn prune_keep_last_n(node: &mut Node<T>, limit: usize) {
        if node.children.len() > limit {
            let drop = node.children.len() - limit;
            node.children.drain(0..drop);
        }
        for child in &mut node.children {
            Self::prune_keep_last_n(child, limit);
        }
    }

    fn prune_keep_depth_under(node: &mut Node<T>, limit: usize) {
        if limit == 0 || node.depth + 1 >= limit {
            node.children.clear();
            return;
        }
        for child in &mut node.children {
            Self::prune_keep_depth_under(child, limit);
        }
    }
}

// ---------------------------------------------------------------------------
// DfsIter
// ---------------------------------------------------------------------------

/// Depth-first iterator over `(depth, &Node<T>)` pairs.
///
/// Created by [`Tree::dfs`].
pub struct DfsIter<'a, T> {
    stack: Vec<(usize, &'a Node<T>)>,
}

impl<'a, T> Iterator for DfsIter<'a, T> {
    type Item = (usize, &'a Node<T>);

    fn next(&mut self) -> Option<Self::Item> {
        let (depth, node) = self.stack.pop()?;
        for child in node.children.iter().rev() {
            self.stack.push((depth + 1, child));
        }
        Some((depth, node))
    }
}

// ---------------------------------------------------------------------------
// CursoredTree
// ---------------------------------------------------------------------------

/// Tree with a movable cursor that tracks the current position.
///
/// Wraps a [`Tree<T>`] and maintains a cursor as a `Vec<usize>` path from
/// the root.  Navigation methods return `bool` to indicate whether the move
/// succeeded; the cursor is never left in an invalid state.
///
/// `CursoredTree<T>` implements `Deref<Target = Tree<T>>`, so all read-only
/// `Tree` methods are available directly.  For mutation through the inner
/// tree, use [`tree_mut`](Self::tree_mut).
#[derive(Debug, Clone)]
pub struct CursoredTree<T> {
    tree: Tree<T>,
    cursor: Vec<usize>,
}

impl<T> CursoredTree<T> {
    /// Create a new tree with a single root node.  The cursor starts at root.
    pub fn new(root_value: T) -> Self {
        Self {
            tree: Tree::new(root_value),
            cursor: Vec::new(),
        }
    }

    // -- cursor read --------------------------------------------------------

    /// Index path from the root to the current node.
    pub fn cursor_path(&self) -> &[usize] {
        &self.cursor
    }

    /// Alias for [`cursor_path`](Self::cursor_path).
    pub fn cursor(&self) -> &[usize] {
        &self.cursor
    }

    /// Reference to the node at the cursor.
    pub fn current(&self) -> &Node<T> {
        self.tree
            .node_at_path(&self.cursor)
            .expect("cursor points to existing node")
    }

    /// Mutable reference to the node at the cursor.
    pub fn current_mut(&mut self) -> &mut Node<T> {
        let path = self.cursor.clone();
        self.tree
            .node_mut_at_path(&path)
            .expect("cursor points to existing node")
    }

    /// Id of the node at the cursor.
    pub fn current_id(&self) -> u64 {
        self.current().id()
    }

    /// `true` when the cursor is not at the root.
    pub fn has_parent(&self) -> bool {
        !self.cursor.is_empty()
    }

    /// `true` when the current node has at least one child.
    pub fn has_children(&self) -> bool {
        !self.current().is_leaf()
    }

    // -- cursor mutation ----------------------------------------------------

    /// Add a child to the current node and move the cursor to it.
    ///
    /// Always succeeds because the parent is the current node.
    pub fn push(&mut self, value: T) -> u64 {
        let parent_id = self.current().id();
        let child_id = self
            .tree
            .push_child(parent_id, value)
            .expect("current node exists");
        let child_index = self.current().child_count() - 1;
        self.cursor.push(child_index);
        child_id
    }

    /// Move the cursor to the parent.  Returns `false` at the root.
    pub fn go_parent(&mut self) -> bool {
        self.cursor.pop().is_some()
    }

    /// Move the cursor to the child at `index`.  Returns `false` if out of
    /// bounds.
    pub fn go_child(&mut self, index: usize) -> bool {
        if self.current().children().get(index).is_none() {
            return false;
        }
        self.cursor.push(index);
        true
    }

    /// Move the cursor to the last (newest) child.  Returns `false` when
    /// there are no children.
    pub fn go_child_last(&mut self) -> bool {
        let count = self.current().child_count();
        if count == 0 {
            return false;
        }
        self.cursor.push(count - 1);
        true
    }

    /// Move the cursor to the next sibling.  Returns `false` when there is
    /// no next sibling or the cursor is at the root.
    pub fn go_sibling_next(&mut self) -> bool {
        let Some(current_index) = self.cursor.last().copied() else {
            return false;
        };
        let parent_child_count = {
            let parent_path = &self.cursor[..self.cursor.len() - 1];
            match self.tree.node_at_path(parent_path) {
                Some(p) => p.children().len(),
                None => return false,
            }
        };
        if current_index + 1 >= parent_child_count {
            return false;
        }
        *self.cursor.last_mut().expect("checked above") += 1;
        true
    }

    /// Move the cursor to the previous sibling.  Returns `false` when there
    /// is no previous sibling or the cursor is at the root.
    pub fn go_sibling_prev(&mut self) -> bool {
        let Some(last) = self.cursor.last_mut() else {
            return false;
        };
        if *last == 0 {
            return false;
        }
        *last -= 1;
        true
    }

    /// Jump the cursor to the node with the given id.  Returns `false` when
    /// the id does not exist.
    pub fn go_to(&mut self, id: u64) -> bool {
        match self.tree.find_path_to(id) {
            Some(path) => {
                self.cursor = path;
                true
            }
            None => false,
        }
    }

    /// Move the cursor back to the root.  Returns `false` if already there.
    pub fn go_root(&mut self) -> bool {
        if self.cursor.is_empty() {
            return false;
        }
        self.cursor.clear();
        true
    }

    // -- delegated tree mutations -------------------------------------------

    /// Append a child to the node identified by `parent_id`.
    ///
    /// The cursor is not moved.  See [`push`](Self::push) for the
    /// push-and-move variant.
    pub fn push_child(&mut self, parent_id: u64, value: T) -> Result<u64, ParentNotFound> {
        self.tree.push_child(parent_id, value)
    }

    /// Remove a subtree.  If the cursor was inside the removed subtree, it is
    /// repaired to the nearest surviving ancestor.
    pub fn remove_subtree(&mut self, id: u64) -> Option<Node<T>> {
        let removed = self.tree.remove_subtree(id);
        self.repair_cursor();
        removed
    }

    /// Prune the tree.  If the cursor was on a pruned node, it is repaired to
    /// the nearest surviving ancestor.
    pub fn prune(&mut self, policy: PrunePolicy) {
        let current_id = self.current().id();
        self.tree.prune(policy);
        if let Some(path) = self.tree.find_path_to(current_id) {
            self.cursor = path;
        } else {
            self.repair_cursor();
        }
    }

    // -- inner tree access --------------------------------------------------

    /// Immutable reference to the inner [`Tree`].
    pub fn tree(&self) -> &Tree<T> {
        &self.tree
    }

    /// Mutable reference to the inner [`Tree`].
    ///
    /// Use with care: structural changes may invalidate the cursor.  Call
    /// methods on `CursoredTree` instead when possible.
    pub fn tree_mut(&mut self) -> &mut Tree<T> {
        &mut self.tree
    }

    // -- private ------------------------------------------------------------

    fn repair_cursor(&mut self) {
        while self.tree.node_at_path(&self.cursor).is_none() {
            if self.cursor.pop().is_none() {
                break;
            }
        }
    }
}

impl<T> Deref for CursoredTree<T> {
    type Target = Tree<T>;

    fn deref(&self) -> &Self::Target {
        &self.tree
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Tree ---------------------------------------------------------------

    #[test]
    fn tree_new_creates_root_with_id_zero() {
        let tree = Tree::new("root");
        assert_eq!(tree.root().id(), 0);
        assert_eq!(tree.root().depth(), 0);
        assert_eq!(tree.root().value(), &"root");
        assert!(tree.root().is_leaf());
        assert_eq!(tree.root().child_count(), 0);
        assert_eq!(tree.next_id(), 1);
        assert_eq!(tree.find_path_to(0), Some(vec![]));
    }

    #[test]
    fn push_child_assigns_incrementing_ids_and_updates_index() {
        let mut tree = Tree::new("root");
        let a = tree.push_child(0, "a").unwrap();
        let b = tree.push_child(0, "b").unwrap();
        let a1 = tree.push_child(a, "a1").unwrap();

        assert_eq!(a, 1);
        assert_eq!(b, 2);
        assert_eq!(a1, 3);

        assert_eq!(tree.find(a).unwrap().depth(), 1);
        assert_eq!(tree.find(a1).unwrap().depth(), 2);
        assert_eq!(tree.find_path_to(b), Some(vec![1]));
        assert_eq!(tree.find_path_to(a1), Some(vec![0, 0]));
    }

    #[test]
    fn push_child_returns_error_for_missing_parent() {
        let mut tree = Tree::new("root");
        let err = tree.push_child(99, "x").unwrap_err();
        assert_eq!(err.parent_id(), 99);
        assert_eq!(err.to_string(), "parent node 99 not found in tree");
    }

    #[test]
    fn find_mut_allows_value_mutation() {
        let mut tree = Tree::new(String::from("root"));
        let a = tree.push_child(0, String::from("a")).unwrap();
        tree.find_mut(a).unwrap().value_mut().push_str("_updated");
        assert_eq!(tree.find(a).unwrap().value(), "a_updated");
    }

    #[test]
    fn find_returns_none_for_nonexistent_id() {
        let tree = Tree::new("root");
        assert!(tree.find(42).is_none());
        assert!(tree.find_path_to(42).is_none());
    }

    #[test]
    fn dfs_visits_nodes_in_preorder() {
        let mut tree = Tree::new("r");
        let a = tree.push_child(0, "a").unwrap();
        let b = tree.push_child(0, "b").unwrap();
        tree.push_child(a, "a1").unwrap();
        tree.push_child(b, "b1").unwrap();

        let order: Vec<(&str, usize)> = tree.dfs().map(|(d, n)| (*n.value(), d)).collect();
        assert_eq!(
            order,
            vec![("r", 0), ("a", 1), ("a1", 2), ("b", 1), ("b1", 2)]
        );
    }

    #[test]
    fn flatten_matches_dfs_output() {
        let mut tree = Tree::new("r");
        let a = tree.push_child(0, "a").unwrap();
        tree.push_child(a, "a1").unwrap();

        let dfs: Vec<_> = tree.dfs().map(|(d, n)| (d, n.id())).collect();
        let flat: Vec<_> = tree.flatten().iter().map(|(d, n)| (*d, n.id())).collect();
        assert_eq!(dfs, flat);
    }

    #[test]
    fn remove_subtree_compacts_siblings() {
        let mut tree = Tree::new("root");
        let a = tree.push_child(0, "a").unwrap();
        let b = tree.push_child(0, "b").unwrap();
        let c = tree.push_child(0, "c").unwrap();

        let removed = tree.remove_subtree(b).unwrap();
        assert_eq!(removed.id(), b);
        assert!(tree.find(b).is_none());
        // c was at index 2, now compacted to index 1
        assert_eq!(tree.find_path_to(c), Some(vec![1]));
        assert_eq!(tree.root().children().len(), 2);
        assert_eq!(tree.root().children()[0].id(), a);
        assert_eq!(tree.root().children()[1].id(), c);
    }

    #[test]
    fn remove_subtree_returns_none_for_root() {
        let mut tree = Tree::new("root");
        assert!(tree.remove_subtree(0).is_none());
    }

    #[test]
    fn prune_keep_last_n_drops_oldest_children() {
        let mut tree = Tree::new("root");
        tree.push_child(0, "a").unwrap();
        let b = tree.push_child(0, "b").unwrap();
        let c = tree.push_child(0, "c").unwrap();
        tree.push_child(c, "c1").unwrap();
        let c2 = tree.push_child(c, "c2").unwrap();
        let c3 = tree.push_child(c, "c3").unwrap();

        tree.prune(PrunePolicy::KeepLastN(2));

        let root_ids: Vec<u64> = tree.root().children().iter().map(Node::id).collect();
        assert_eq!(root_ids, vec![b, c]);
        let c_ids: Vec<u64> = tree
            .find(c)
            .unwrap()
            .children()
            .iter()
            .map(Node::id)
            .collect();
        assert_eq!(c_ids, vec![c2, c3]);
    }

    #[test]
    fn prune_keep_depth_under_truncates_deep_branches() {
        let mut tree = Tree::new("root");
        let a = tree.push_child(0, "a").unwrap();
        let a1 = tree.push_child(a, "a1").unwrap();
        tree.push_child(a1, "a2").unwrap();
        let b = tree.push_child(0, "b").unwrap();

        tree.prune(PrunePolicy::KeepDepthUnder(2));

        assert!(tree.find(a).is_some());
        assert!(tree.find(b).is_some());
        assert!(tree.find(a1).is_none());
    }

    // -- CursoredTree -------------------------------------------------------

    #[test]
    fn cursored_push_adds_child_and_moves_cursor() {
        let mut ct = CursoredTree::new("root");
        let a = ct.push("a");
        assert_eq!(ct.current_id(), a);
        assert_eq!(ct.cursor_path(), &[0]);
        assert!(ct.has_parent());

        let a1 = ct.push("a1");
        assert_eq!(ct.current_id(), a1);
        assert_eq!(ct.cursor_path(), &[0, 0]);
    }

    #[test]
    fn cursored_go_parent_stops_at_root() {
        let mut ct = CursoredTree::new("root");
        assert!(!ct.go_parent());

        ct.push("child");
        assert!(ct.go_parent());
        assert_eq!(ct.current_id(), 0);
    }

    #[test]
    fn cursored_go_child_and_go_child_last() {
        let mut ct = CursoredTree::new("root");
        ct.push_child(0, "a").unwrap();
        let b = ct.push_child(0, "b").unwrap();
        let b1 = ct.push_child(b, "b1").unwrap();

        assert!(ct.go_child_last());
        assert_eq!(ct.current_id(), b);
        assert!(ct.go_child_last());
        assert_eq!(ct.current_id(), b1);
        assert!(!ct.go_child_last()); // leaf
    }

    #[test]
    fn cursored_go_to_jumps_to_any_node() {
        let mut ct = CursoredTree::new("root");
        let a = ct.push_child(0, "a").unwrap();
        let b = ct.push_child(0, "b").unwrap();
        ct.push_child(b, "b1").unwrap();

        assert!(ct.go_to(a));
        assert_eq!(ct.current_id(), a);
        assert!(ct.go_to(0));
        assert_eq!(ct.current_id(), 0);
        assert!(!ct.go_to(999));
    }

    #[test]
    fn cursored_sibling_navigation() {
        let mut ct = CursoredTree::new("root");
        ct.push_child(0, "a").unwrap();
        let b = ct.push_child(0, "b").unwrap();
        ct.push_child(0, "c").unwrap();

        assert!(ct.go_child(1)); // b
        assert_eq!(ct.current_id(), b);
        assert!(ct.go_sibling_next()); // c
        assert!(!ct.go_sibling_next()); // end
        assert!(ct.go_sibling_prev()); // b
        assert_eq!(ct.current_id(), b);
        assert!(ct.go_sibling_prev()); // a
        assert!(!ct.go_sibling_prev()); // start
    }

    #[test]
    fn cursored_go_root() {
        let mut ct = CursoredTree::new("root");
        assert!(!ct.go_root());
        ct.push("a");
        ct.push("a1");
        assert!(ct.go_root());
        assert_eq!(ct.current_id(), 0);
    }

    #[test]
    fn cursored_has_parent_and_has_children() {
        let mut ct = CursoredTree::new("root");
        assert!(!ct.has_parent());
        assert!(!ct.has_children());

        ct.push("child");
        assert!(ct.has_parent());
        assert!(!ct.has_children());

        ct.go_parent();
        assert!(!ct.has_parent());
        assert!(ct.has_children());
    }

    #[test]
    fn cursored_prune_repairs_cursor() {
        let mut ct = CursoredTree::new("root");
        let a = ct.push_child(0, "a").unwrap();
        ct.push_child(a, "a1").unwrap();

        assert!(ct.go_child(0));
        assert!(ct.go_child(0)); // a1
        ct.prune(PrunePolicy::KeepDepthUnder(2));
        assert_eq!(ct.current_id(), a);
        assert_eq!(ct.cursor(), &[0]);
    }

    #[test]
    fn cursored_remove_subtree_repairs_cursor() {
        let mut ct = CursoredTree::new("root");
        let a = ct.push_child(0, "a").unwrap();
        let b = ct.push_child(0, "b").unwrap();

        assert!(ct.go_child(1)); // b
        let removed = ct.remove_subtree(b).unwrap();
        assert_eq!(removed.id(), b);
        assert_eq!(ct.current_id(), 0);
        assert!(ct.find(a).is_some());
        assert!(ct.find(b).is_none());
    }

    #[test]
    fn cursored_tree_mut_gives_inner_access() {
        let mut ct = CursoredTree::new("root");
        ct.push("a");
        ct.tree_mut().root_mut().value_mut();
        assert_eq!(ct.root().child_count(), 1);
    }

    #[test]
    fn deref_exposes_tree_methods() {
        let ct = CursoredTree::new("root");
        // Through Deref we can call Tree::root, Tree::find, etc.
        assert_eq!(ct.root().id(), 0);
        assert!(ct.find(0).is_some());
    }
}
