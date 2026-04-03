use std::collections::BTreeSet;

use neco_pathrel::{parent_path, path_matches_or_contains, PathPolicy};

/// Tree node kind used by runtime-facing file tree helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileTreeNodeKind {
    File,
    Directory,
}

/// Whether a directory node is fully materialized or only partially known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectoryMaterialization {
    Complete,
    Partial,
}

/// Public runtime-facing file tree node shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTreeNode {
    pub name: String,
    pub path: String,
    pub kind: FileTreeNodeKind,
    pub children: Vec<FileTreeNode>,
    pub materialization: DirectoryMaterialization,
    pub child_count: Option<usize>,
}

/// One flattened row derived from a file tree snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatFileTreeRow {
    pub depth: usize,
    pub name: String,
    pub path: String,
    pub kind: FileTreeNodeKind,
    pub is_collapsed: bool,
    pub materialization: DirectoryMaterialization,
    pub child_count: Option<usize>,
}

/// Expansion instructions needed to reveal a target path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevealPlan {
    pub expand_paths: Vec<String>,
    pub found: bool,
}

/// Find one node by exact path under the given root.
pub fn find_node_by_path<'a>(
    root: &'a FileTreeNode,
    path: &str,
    policy: &PathPolicy,
) -> Option<&'a FileTreeNode> {
    if path_eq(&root.path, path, policy) {
        return Some(root);
    }
    for child in &root.children {
        if let Some(found) = find_node_by_path(child, path, policy) {
            return Some(found);
        }
    }
    None
}

/// Replace the matching subtree while keeping non-target branches unchanged in value.
pub fn merge_subtree(
    root: &FileTreeNode,
    subtree: FileTreeNode,
    policy: &PathPolicy,
) -> FileTreeNode {
    if path_eq(&root.path, &subtree.path, policy) {
        return subtree;
    }

    let mut changed = false;
    let next_children = root
        .children
        .iter()
        .map(|child| {
            let next_child = merge_subtree(child, subtree.clone(), policy);
            if next_child != *child {
                changed = true;
            }
            next_child
        })
        .collect::<Vec<_>>();

    if !changed {
        return root.clone();
    }

    let mut next = root.clone();
    next.children = next_children;
    next
}

/// Flatten a tree into visible rows under the current collapsed set.
pub fn flatten_file_tree(
    root: &FileTreeNode,
    collapsed_paths: &BTreeSet<String>,
    include_root: bool,
    policy: &PathPolicy,
) -> Vec<FlatFileTreeRow> {
    let mut rows = Vec::new();
    flatten_into(root, collapsed_paths, include_root, 0, policy, &mut rows);
    rows
}

/// Build the ancestor expansion plan needed to reveal one target path.
pub fn reveal_plan_for_path(
    root: &FileTreeNode,
    target_path: &str,
    policy: &PathPolicy,
) -> RevealPlan {
    if !path_matches_or_contains(target_path, &root.path, policy) {
        return RevealPlan {
            expand_paths: Vec::new(),
            found: false,
        };
    }

    if find_node_by_path(root, target_path, policy).is_none() {
        return RevealPlan {
            expand_paths: Vec::new(),
            found: false,
        };
    }

    let mut expand_paths = Vec::new();
    let mut cursor = parent_path(target_path, policy);
    while let Some(path) = cursor {
        if path_eq(path, &root.path, policy) {
            if root.kind == FileTreeNodeKind::Directory {
                expand_paths.push(root.path.clone());
            }
            break;
        }

        if let Some(node) = find_node_by_path(root, path, policy) {
            if node.kind == FileTreeNodeKind::Directory {
                expand_paths.push(node.path.clone());
            }
        }
        cursor = parent_path(path, policy);
    }
    expand_paths.reverse();
    RevealPlan {
        expand_paths,
        found: true,
    }
}

fn flatten_into(
    node: &FileTreeNode,
    collapsed_paths: &BTreeSet<String>,
    include_self: bool,
    depth: usize,
    policy: &PathPolicy,
    rows: &mut Vec<FlatFileTreeRow>,
) {
    let is_collapsed = collapsed_paths
        .iter()
        .any(|path| path_eq(path, &node.path, policy));

    if include_self {
        rows.push(FlatFileTreeRow {
            depth,
            name: node.name.clone(),
            path: node.path.clone(),
            kind: node.kind,
            is_collapsed,
            materialization: node.materialization,
            child_count: node.child_count,
        });
    }

    let next_depth = if include_self { depth + 1 } else { depth };
    if include_self && is_collapsed {
        return;
    }

    for child in &node.children {
        flatten_into(child, collapsed_paths, true, next_depth, policy, rows);
    }
}

fn path_eq(left: &str, right: &str, policy: &PathPolicy) -> bool {
    path_matches_or_contains(left, right, policy) && path_matches_or_contains(right, left, policy)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use neco_pathrel::{PathCaseSensitivity, PathPolicy};

    use super::{
        find_node_by_path, flatten_file_tree, merge_subtree, reveal_plan_for_path,
        DirectoryMaterialization, FileTreeNode, FileTreeNodeKind,
    };

    fn posix() -> PathPolicy {
        PathPolicy::posix()
    }

    fn insensitive() -> PathPolicy {
        PathPolicy::new('/', PathCaseSensitivity::Insensitive)
    }

    fn file(name: &str, path: &str) -> FileTreeNode {
        FileTreeNode {
            name: name.to_string(),
            path: path.to_string(),
            kind: FileTreeNodeKind::File,
            children: Vec::new(),
            materialization: DirectoryMaterialization::Complete,
            child_count: None,
        }
    }

    fn dir(
        name: &str,
        path: &str,
        materialization: DirectoryMaterialization,
        child_count: Option<usize>,
        children: Vec<FileTreeNode>,
    ) -> FileTreeNode {
        FileTreeNode {
            name: name.to_string(),
            path: path.to_string(),
            kind: FileTreeNodeKind::Directory,
            children,
            materialization,
            child_count,
        }
    }

    fn sample_tree() -> FileTreeNode {
        dir(
            "workspace",
            "/workspace",
            DirectoryMaterialization::Complete,
            Some(3),
            vec![
                dir(
                    "src",
                    "/workspace/src",
                    DirectoryMaterialization::Complete,
                    Some(2),
                    vec![
                        file("lib.rs", "/workspace/src/lib.rs"),
                        file("main.rs", "/workspace/src/main.rs"),
                    ],
                ),
                dir(
                    "docs",
                    "/workspace/docs",
                    DirectoryMaterialization::Partial,
                    Some(5),
                    vec![file("guide.md", "/workspace/docs/guide.md")],
                ),
                file("Cargo.toml", "/workspace/Cargo.toml"),
            ],
        )
    }

    #[test]
    fn find_node_by_path_returns_exact_match() {
        let tree = sample_tree();
        let found =
            find_node_by_path(&tree, "/workspace/src/lib.rs", &posix()).expect("node should exist");
        assert_eq!(found.name, "lib.rs");
    }

    #[test]
    fn find_node_by_path_can_use_case_insensitive_policy() {
        let tree = sample_tree();
        let found = find_node_by_path(&tree, "/WORKSPACE/SRC/LIB.RS", &insensitive())
            .expect("node should exist");
        assert_eq!(found.path, "/workspace/src/lib.rs");
    }

    #[test]
    fn merge_subtree_replaces_exact_match_path() {
        let tree = sample_tree();
        let replacement = dir(
            "src",
            "/workspace/src",
            DirectoryMaterialization::Complete,
            Some(1),
            vec![file("mod.rs", "/workspace/src/mod.rs")],
        );
        let merged = merge_subtree(&tree, replacement, &posix());
        let src = find_node_by_path(&merged, "/workspace/src", &posix()).expect("src should exist");
        assert_eq!(src.children.len(), 1);
        assert_eq!(src.children[0].name, "mod.rs");
    }

    #[test]
    fn merge_subtree_keeps_non_target_branches() {
        let tree = sample_tree();
        let replacement = dir(
            "src",
            "/workspace/src",
            DirectoryMaterialization::Complete,
            Some(1),
            vec![file("mod.rs", "/workspace/src/mod.rs")],
        );
        let merged = merge_subtree(&tree, replacement, &posix());
        let docs =
            find_node_by_path(&merged, "/workspace/docs", &posix()).expect("docs should exist");
        assert_eq!(docs.materialization, DirectoryMaterialization::Partial);
    }

    #[test]
    fn merge_subtree_preserves_partial_metadata_from_subtree() {
        let tree = sample_tree();
        let replacement = dir(
            "docs",
            "/workspace/docs",
            DirectoryMaterialization::Partial,
            Some(9),
            vec![file("reference.md", "/workspace/docs/reference.md")],
        );
        let merged = merge_subtree(&tree, replacement, &posix());
        let docs =
            find_node_by_path(&merged, "/workspace/docs", &posix()).expect("docs should exist");
        assert_eq!(docs.child_count, Some(9));
        assert_eq!(docs.children[0].name, "reference.md");
    }

    #[test]
    fn flatten_file_tree_respects_collapsed_paths() {
        let tree = sample_tree();
        let mut collapsed = BTreeSet::new();
        collapsed.insert("/workspace/src".to_string());
        let rows = flatten_file_tree(&tree, &collapsed, true, &posix());
        assert!(rows
            .iter()
            .any(|row| row.path == "/workspace/src" && row.is_collapsed));
        assert!(!rows.iter().any(|row| row.path == "/workspace/src/lib.rs"));
    }

    #[test]
    fn flatten_file_tree_can_skip_root_row() {
        let tree = sample_tree();
        let rows = flatten_file_tree(&tree, &BTreeSet::new(), false, &posix());
        assert_eq!(
            rows.first().map(|row| row.path.as_str()),
            Some("/workspace/src")
        );
        assert!(rows.iter().all(|row| row.path != "/workspace"));
    }

    #[test]
    fn flatten_file_tree_keeps_partial_directory_rows() {
        let tree = sample_tree();
        let rows = flatten_file_tree(&tree, &BTreeSet::new(), true, &posix());
        let docs = rows
            .iter()
            .find(|row| row.path == "/workspace/docs")
            .expect("docs row should exist");
        assert_eq!(docs.materialization, DirectoryMaterialization::Partial);
        assert_eq!(docs.child_count, Some(5));
    }

    #[test]
    fn reveal_plan_returns_ancestor_directories_in_order() {
        let tree = sample_tree();
        let plan = reveal_plan_for_path(&tree, "/workspace/src/lib.rs", &posix());
        assert!(plan.found);
        assert_eq!(
            plan.expand_paths,
            vec!["/workspace".to_string(), "/workspace/src".to_string()]
        );
    }

    #[test]
    fn reveal_plan_is_empty_when_target_is_missing() {
        let tree = sample_tree();
        let plan = reveal_plan_for_path(&tree, "/workspace/missing.txt", &posix());
        assert!(!plan.found);
        assert!(plan.expand_paths.is_empty());
    }

    #[test]
    fn reveal_plan_for_root_has_no_expansion_steps() {
        let tree = sample_tree();
        let plan = reveal_plan_for_path(&tree, "/workspace", &posix());
        assert!(plan.found);
        assert!(plan.expand_paths.is_empty());
    }
}
