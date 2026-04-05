# neco-tree

ID 付き汎用多分木とカーソルナビゲーションを提供するクレートです。

各ノードは一意な `u64` id とユーザー定義の値 `T` を保持します。内部の `HashMap` インデックスにより id 検索は O(1) です。`CursoredTree<T>` は木の中の現在位置を追跡するカーソルを追加します。

## 機能

- `Tree<T>`: 自動採番ノード id 付きの多分木
- 内部インデックスによる O(1) id 検索 (`find`, `find_path_to`)
- `CursoredTree<T>`: カーソルナビゲーション (親、子、兄弟、ジャンプ)
- 深さ情報付き DFS イテレータ
- ポリシーベースの枝刈り (`KeepLastN`, `KeepDepthUnder`)
- サブツリー削除とインデックス自動再構築

## 使い方

ツリーの構築とカーソル移動:

```rust
use neco_tree::CursoredTree;

let mut tree = CursoredTree::new("root");

// push は現在ノードの子を追加し、カーソルをそこに移動する
let a = tree.push("a");
let a1 = tree.push("a1");

assert_eq!(tree.current_id(), a1);
assert!(tree.has_parent());

// go_parent で親に移動
tree.go_parent();
assert_eq!(tree.current_id(), a);

// go_root でルートへジャンプ
tree.go_root();
assert_eq!(tree.current_id(), 0);
```

カーソル不要な場合は `Tree<T>` を直接使用:

```rust
use neco_tree::Tree;

let mut tree = Tree::new("root");
let a = tree.push_child(0, "a").unwrap();
let b = tree.push_child(0, "b").unwrap();
tree.push_child(a, "a1").unwrap();

// 深さ付き DFS 走査
for (depth, node) in tree.dfs() {
    println!("{}{}", "  ".repeat(depth), node.value());
}

// O(1) 検索
assert_eq!(tree.find(b).unwrap().value(), &"b");
assert_eq!(tree.find_path_to(b), Some(vec![1]));
```

古いブランチの枝刈り:

```rust
use neco_tree::{CursoredTree, PrunePolicy};

let mut tree = CursoredTree::new("root");
tree.push_child(0, "old-1").unwrap();
tree.push_child(0, "old-2").unwrap();
tree.push_child(0, "keep").unwrap();

// ノードごとに最新の子を 2 つだけ保持
tree.prune(PrunePolicy::KeepLastN(2));
assert_eq!(tree.root().child_count(), 2);
```

## API

| 項目 | 説明 |
|------|------|
| `Node<T>` | id、depth、value、children を持つノード |
| `Tree<T>` | O(1) id インデックス付きの多分木 |
| `CursoredTree<T>` | 現在位置を追跡するカーソル付き `Tree<T>` |
| `DfsIter` | `(depth, &Node<T>)` を返す深さ優先イテレータ |
| `PrunePolicy` | 枝刈り方針: `KeepLastN(n)` または `KeepDepthUnder(depth)` |
| `ParentNotFound` | 存在しない親へ `push_child` した場合のエラー |

## 計算量

| 操作 | 時間 |
|------|------|
| `push_child` | O(1) 償却 |
| `find` / `find_path_to` | O(1) 償却 |
| `dfs` / `flatten` | O(n) |
| `remove_subtree` | O(n) (インデックス再構築) |
| `prune` | O(n) (インデックス再構築) |
| `go_parent` / `go_child` / `go_to` | O(1) |

## ライセンス

MIT
