//! 一意な識別子を持つ根付き木と、カーソルによる走査を提供する。
//!
//! `Tree` は挿入順の子を保持し、識別子からノードを検索する。`CursoredTree` は木と現在位置を保持する。

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::ops::Deref;

/// 木のノード。フィールドは private であり、アクセサメソッドで参照または変更する。
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

    /// 作成時に割り当てる一意な識別子を返す。
    pub fn id(&self) -> u64 {
        self.id
    }

    /// 根を 0 とする深さを返す。
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// 格納値への不変参照を返す。
    pub fn value(&self) -> &T {
        &self.value
    }

    /// 格納値への可変参照を返す。
    pub fn value_mut(&mut self) -> &mut T {
        &mut self.value
    }

    /// 挿入順の子ノードを返す。
    pub fn children(&self) -> &[Node<T>] {
        &self.children
    }

    /// 子ノード列への可変参照を返す。
    pub fn children_mut(&mut self) -> &mut Vec<Node<T>> {
        &mut self.children
    }

    /// 直接の子ノード数を返す。
    pub fn child_count(&self) -> usize {
        self.children.len()
    }

    /// 子ノードがなければ `true` を返す。
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }
}

/// [`Tree::push_child`] の親識別子が木にない場合のエラー。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParentNotFound(u64);

impl ParentNotFound {
    /// 見つからなかった親識別子を返す。
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

/// [`Tree::prune`] の間引き方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrunePolicy {
    /// 各ノードで新しい `n` 個の子を残す。
    KeepLastN(usize),
    /// 深さが `limit` 以上のノードを取り除く。
    KeepDepthUnder(usize),
}

/// 一意な `u64` 識別子を持つ根付き多分木。根の識別子は 0 である。識別子から経路を引く処理は償却 O(1) であり、ノード参照はその経路の深さに比例する。
#[derive(Debug, Clone)]
pub struct Tree<T> {
    root: Node<T>,
    next_id: u64,
    index: HashMap<u64, Vec<usize>>,
}

impl<T> Tree<T> {
    /// 識別子 0 の根ノードだけを持つ木を作る。
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

    /// 根ノードへの不変参照を返す。
    pub fn root(&self) -> &Node<T> {
        &self.root
    }

    /// 根ノードへの可変参照を返す。
    pub fn root_mut(&mut self) -> &mut Node<T> {
        &mut self.root
    }

    /// 次に挿入するノードの識別子を返す。
    pub fn next_id(&self) -> u64 {
        self.next_id
    }

    /// `parent_id` で識別されるノードに子を追加する。
    /// 割り当てた識別子を返し、`parent_id` が木にない場合は
    /// [`ParentNotFound`] を返す。
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

    /// 識別子からノードを返す。存在しない場合は `None` を返す。
    pub fn find(&self, id: u64) -> Option<&Node<T>> {
        let path = self.index.get(&id)?;
        self.node_at_path(path)
    }

    /// 識別子からノードへの可変参照を返す。存在しない場合は `None` を返す。
    pub fn find_mut(&mut self, id: u64) -> Option<&mut Node<T>> {
        let path = self.index.get(&id)?.clone();
        self.node_mut_at_path(&path)
    }

    /// 根から識別子までの子インデックス列を返す。存在しない場合は `None` を返す。
    pub fn find_path_to(&self, id: u64) -> Option<Vec<usize>> {
        self.index.get(&id).cloned()
    }

    /// 深さ優先順の `(深さ, ノード)` を返す反復子を作る。
    pub fn dfs(&self) -> DfsIter<'_, T> {
        DfsIter {
            stack: vec![(0, &self.root)],
        }
    }

    /// [`dfs`](Self::dfs) の要素を `Vec` に集める。
    pub fn flatten(&self) -> Vec<(usize, &Node<T>)> {
        self.dfs().collect()
    }

    /// `id` を根とする部分木を削除し、その根ノードを返す。
    /// 根は削除できず、存在しない識別子の場合も `None` を返す。
    /// 残った兄弟のインデックスを詰め、内部インデックスを再構築する。
    pub fn remove_subtree(&mut self, id: u64) -> Option<Node<T>> {
        let path = self.find_path_to(id)?;
        if path.is_empty() {
            return None;
        }
        let mut parent_path = path;
        let child_index = parent_path.pop()?;
        let parent = self.node_mut_at_path(&parent_path)?;
        let removed = parent.children.remove(child_index);
        self.rebuild_index();
        Some(removed)
    }

    /// 木全体に間引き方法を適用する。
    /// 間引き後に内部インデックスを再構築する。ノードは保護しないため、
    /// カーソルなど特定の経路を維持する場合は [`CursoredTree::prune`] を使う。
    pub fn prune(&mut self, policy: PrunePolicy) {
        match policy {
            PrunePolicy::KeepLastN(n) => Self::prune_keep_last_n(&mut self.root, n),
            PrunePolicy::KeepDepthUnder(d) => Self::prune_keep_depth_under(&mut self.root, d),
        }
        self.rebuild_index();
    }

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

/// [`Tree::dfs`] が作る深さ優先反復子。
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

/// 現在位置を根からの子インデックス列として保持する木。移動操作は成功時に `true` を返し、カーソルは有効なノードを指し続ける。
#[derive(Debug, Clone)]
pub struct CursoredTree<T> {
    tree: Tree<T>,
    cursor: Vec<usize>,
}

impl<T> CursoredTree<T> {
    /// 根ノードだけを持つ木を作り、カーソルを根に置く。
    pub fn new(root_value: T) -> Self {
        Self {
            tree: Tree::new(root_value),
            cursor: Vec::new(),
        }
    }

    /// 根から現在のノードまでの子インデックス列を返す。
    pub fn cursor_path(&self) -> &[usize] {
        &self.cursor
    }

    /// [`cursor_path`](Self::cursor_path) の別名。
    pub fn cursor(&self) -> &[usize] {
        &self.cursor
    }

    /// カーソル位置のノードへの不変参照を返す。
    pub fn current(&self) -> &Node<T> {
        self.tree
            .node_at_path(&self.cursor)
            .expect("cursor points to existing node")
    }

    /// カーソル位置のノードへの可変参照を返す。
    pub fn current_mut(&mut self) -> &mut Node<T> {
        let path = self.cursor.clone();
        self.tree
            .node_mut_at_path(&path)
            .expect("cursor points to existing node")
    }

    /// カーソル位置のノードの識別子を返す。
    pub fn current_id(&self) -> u64 {
        self.current().id()
    }

    /// カーソルが根にない場合に `true` を返す。
    pub fn has_parent(&self) -> bool {
        !self.cursor.is_empty()
    }

    /// 現在のノードに子があれば `true` を返す。
    pub fn has_children(&self) -> bool {
        !self.current().is_leaf()
    }

    /// 現在のノードに子を追加し、カーソルを追加した子へ移動する。
    /// 現在のノードが親であるため、常に成功する。
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

    /// カーソルを親へ移動する。根にいる場合は `false` を返す。
    pub fn go_parent(&mut self) -> bool {
        self.cursor.pop().is_some()
    }

    /// カーソルを `index` 番目の子へ移動する。範囲外の場合は `false` を返す。
    pub fn go_child(&mut self, index: usize) -> bool {
        if self.current().children().get(index).is_none() {
            return false;
        }
        self.cursor.push(index);
        true
    }

    /// カーソルを最後の子 ( 最も新しい子 ) へ移動する。子がない場合は `false` を返す。
    pub fn go_child_last(&mut self) -> bool {
        let count = self.current().child_count();
        if count == 0 {
            return false;
        }
        self.cursor.push(count - 1);
        true
    }

    /// カーソルを次の兄弟へ移動する。次の兄弟がない場合や根にいる場合は `false` を返す。
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

    /// カーソルを前の兄弟へ移動する。前の兄弟がない場合や根にいる場合は `false` を返す。
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

    /// 指定した識別子のノードへカーソルを移動する。識別子がない場合は `false` を返す。
    pub fn go_to(&mut self, id: u64) -> bool {
        match self.tree.find_path_to(id) {
            Some(path) => {
                self.cursor = path;
                true
            }
            None => false,
        }
    }

    /// カーソルを根へ戻す。すでに根にいる場合は `false` を返す。
    pub fn go_root(&mut self) -> bool {
        if self.cursor.is_empty() {
            return false;
        }
        self.cursor.clear();
        true
    }

    /// `parent_id` で識別されるノードに子を追加する。カーソルは移動しない。
    /// 子の追加後にカーソルを移動する場合は [`push`](Self::push) を使う。
    pub fn push_child(&mut self, parent_id: u64, value: T) -> Result<u64, ParentNotFound> {
        self.tree.push_child(parent_id, value)
    }

    /// 部分木を削除する。カーソルが削除対象の内部にあれば、残った最も近い祖先へ戻す。
    pub fn remove_subtree(&mut self, id: u64) -> Option<Node<T>> {
        let removed = self.tree.remove_subtree(id);
        self.repair_cursor();
        removed
    }

    /// 木を間引く。カーソルが間引かれたノードにあれば、残った最も近い祖先へ戻す。
    pub fn prune(&mut self, policy: PrunePolicy) {
        let current_id = self.current().id();
        self.tree.prune(policy);
        if let Some(path) = self.tree.find_path_to(current_id) {
            self.cursor = path;
        } else {
            self.repair_cursor();
        }
    }

    /// 内部の [`Tree`] への不変参照を返す。
    pub fn tree(&self) -> &Tree<T> {
        &self.tree
    }

    /// 内部の [`Tree`] への可変参照を返す。
    /// 構造を変更するとカーソルが無効になる場合があるため、可能なら
    /// `CursoredTree` のメソッドを使う。
    pub fn tree_mut(&mut self) -> &mut Tree<T> {
        &mut self.tree
    }

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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(!ct.go_child_last());
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

        assert!(ct.go_child(1));
        assert_eq!(ct.current_id(), b);
        assert!(ct.go_sibling_next());
        assert!(!ct.go_sibling_next());
        assert!(ct.go_sibling_prev());
        assert_eq!(ct.current_id(), b);
        assert!(ct.go_sibling_prev());
        assert!(!ct.go_sibling_prev());
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
        assert!(ct.go_child(0));
        ct.prune(PrunePolicy::KeepDepthUnder(2));
        assert_eq!(ct.current_id(), a);
        assert_eq!(ct.cursor(), &[0]);
    }

    #[test]
    fn cursored_remove_subtree_repairs_cursor() {
        let mut ct = CursoredTree::new("root");
        let a = ct.push_child(0, "a").unwrap();
        let b = ct.push_child(0, "b").unwrap();

        assert!(ct.go_child(1));
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
        assert_eq!(ct.root().id(), 0);
        assert!(ct.find(0).is_some());
    }
}
