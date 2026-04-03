# neco-filetree

[英語版](README.md)

ノード探索、部分木の差し替え、可視行の展開、表示位置までの展開計画計算を行う純粋なファイルツリー補助クレートです。

## ツリーの扱い

`neco-filetree` は中立なツリーノード型を定義し、すべての操作を純粋関数として保ちます。読み込み、監視連携、UI 状態は呼び出し側で持ち、このクレートは完全一致のパス探索、部分木の差し替え、折りたたみ集合を考慮した平坦化、表示対象までの祖先展開列計算を担当します。

ディレクトリノードは `materialization` と `child_count` を持つので、実行時コードはホスト固有型をツリーに混ぜずに、完全取得済みの部分木と一部だけ読んだ一覧を区別できます。

## 使い方

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

| 項目 | 説明 |
|------|-------------|
| `FileTreeNode` | `children`, `materialization`, `child_count` を持つ中立なファイルツリーノード |
| `FileTreeNodeKind` | `File` と `Directory` を区別する |
| `DirectoryMaterialization` | 完全取得済みと部分取得のディレクトリ状態を区別する |
| `FlatFileTreeRow` | 可視ツリー描画向けの平坦な行データ |
| `RevealPlan` | `found` と順序付き祖先展開列を持つ計画 |
| `find_node_by_path(root, path, policy)` | 完全一致のパスでノードを探索する |
| `merge_subtree(root, subtree, policy)` | 一致した部分木を差し替えつつ他の枝を保持する |
| `flatten_file_tree(root, collapsed_paths, include_root, policy)` | ツリースナップショットを可視行へ変換する |
| `reveal_plan_for_path(root, target_path, policy)` | 展開すべき祖先ディレクトリ群を計算する |

## ライセンス

MIT
