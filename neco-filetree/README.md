# neco-filetree

[日本語](README-ja.md)

Pure file tree snapshot helpers for lookup, subtree replacement, visible-row flattening, and reveal planning.

## Tree snapshots

`neco-filetree` defines a neutral tree node shape and keeps all operations pure. Callers own loading, watcher integration, and UI state, while this crate handles exact node lookup, subtree replacement, flattening under a collapsed set, and ancestor expansion planning for reveal behavior.

Directory nodes carry both `materialization` and optional `child_count`, so a runtime can distinguish a fully loaded subtree from a partial listing without introducing host-specific types into the tree itself.

## Usage

```rust
use std::collections::BTreeSet;

use neco_filetree::{
    flatten_file_tree, reveal_plan_for_path, DirectoryMaterialization, FileTreeNode,
    FileTreeNodeKind,
};
use neco_pathrel::PathPolicy;

let tree = FileTreeNode {
    name: "workspace".into(),
    path: "/workspace".into(),
    kind: FileTreeNodeKind::Directory,
    children: vec![FileTreeNode {
        name: "src".into(),
        path: "/workspace/src".into(),
        kind: FileTreeNodeKind::Directory,
        children: vec![FileTreeNode {
            name: "lib.rs".into(),
            path: "/workspace/src/lib.rs".into(),
            kind: FileTreeNodeKind::File,
            children: Vec::new(),
            materialization: DirectoryMaterialization::Complete,
            child_count: None,
        }],
        materialization: DirectoryMaterialization::Complete,
        child_count: Some(1),
    }],
    materialization: DirectoryMaterialization::Complete,
    child_count: Some(1),
};

let rows = flatten_file_tree(&tree, &BTreeSet::new(), true, &PathPolicy::posix());
let plan = reveal_plan_for_path(&tree, "/workspace/src/lib.rs", &PathPolicy::posix());

assert_eq!(rows.len(), 3);
assert_eq!(plan.expand_paths, vec!["/workspace", "/workspace/src"]);
```

## API

| Item | Description |
|------|-------------|
| `FileTreeNode` | Neutral file tree node with `children`, `materialization`, and optional `child_count` |
| `FileTreeNodeKind` | Distinguish `File` and `Directory` nodes |
| `DirectoryMaterialization` | Distinguish fully loaded and partial directory snapshots |
| `FlatFileTreeRow` | Flattened row shape for visible tree rendering |
| `RevealPlan` | Ordered ancestor expansion plan with `found` status |
| `find_node_by_path(root, path, policy)` | Find one node by exact path |
| `merge_subtree(root, subtree, policy)` | Replace the matching subtree while preserving other branches |
| `flatten_file_tree(root, collapsed_paths, include_root, policy)` | Convert a tree snapshot into visible rows |
| `reveal_plan_for_path(root, target_path, policy)` | Compute which ancestor directories should be expanded |

## License

MIT
