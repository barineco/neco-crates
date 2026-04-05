# neco-tree

Generic ID-bearing multi-way tree with cursor-based navigation.

Each node carries a unique `u64` id and a user-defined value of type `T`. An internal `HashMap` index makes id-based lookup O(1). `CursoredTree<T>` adds a movable cursor that tracks the current position in the tree.

## Features

- `Tree<T>`: rooted multi-way tree with auto-incrementing node ids
- O(1) id lookup via internal index (`find`, `find_path_to`)
- `CursoredTree<T>`: cursor navigation (parent, child, sibling, jump)
- DFS iterator with depth information
- Policy-based pruning (`KeepLastN`, `KeepDepthUnder`)
- Subtree removal with automatic index rebuild

## Usage

Build a tree and navigate with a cursor:

```rust
use neco_tree::CursoredTree;

let mut tree = CursoredTree::new("root");

// push adds a child to the current node and moves the cursor there
let a = tree.push("a");
let a1 = tree.push("a1");

assert_eq!(tree.current_id(), a1);
assert!(tree.has_parent());

// go_parent moves up
tree.go_parent();
assert_eq!(tree.current_id(), a);

// go_root jumps to root
tree.go_root();
assert_eq!(tree.current_id(), 0);
```

Use `Tree<T>` directly when no cursor is needed:

```rust
use neco_tree::Tree;

let mut tree = Tree::new("root");
let a = tree.push_child(0, "a").unwrap();
let b = tree.push_child(0, "b").unwrap();
tree.push_child(a, "a1").unwrap();

// DFS traversal with depth
for (depth, node) in tree.dfs() {
    println!("{}{}", "  ".repeat(depth), node.value());
}

// O(1) lookup
assert_eq!(tree.find(b).unwrap().value(), &"b");
assert_eq!(tree.find_path_to(b), Some(vec![1]));
```

Prune old branches:

```rust
use neco_tree::{CursoredTree, PrunePolicy};

let mut tree = CursoredTree::new("root");
tree.push_child(0, "old-1").unwrap();
tree.push_child(0, "old-2").unwrap();
tree.push_child(0, "keep").unwrap();

// Keep only the 2 newest children per node
tree.prune(PrunePolicy::KeepLastN(2));
assert_eq!(tree.root().child_count(), 2);
```

## API

| Item | Description |
|------|-------------|
| `Node<T>` | Single tree node with id, depth, value, and children |
| `Tree<T>` | Rooted multi-way tree with O(1) id index |
| `CursoredTree<T>` | `Tree<T>` with a movable cursor tracking the current position |
| `DfsIter` | Depth-first iterator yielding `(depth, &Node<T>)` |
| `PrunePolicy` | Pruning strategy: `KeepLastN(n)` or `KeepDepthUnder(depth)` |
| `ParentNotFound` | Error returned when `push_child` targets a nonexistent parent |

## Complexity

| Operation | Time |
|-----------|------|
| `push_child` | O(1) amortised |
| `find` / `find_path_to` | O(1) amortised |
| `dfs` / `flatten` | O(n) |
| `remove_subtree` | O(n) (index rebuild) |
| `prune` | O(n) (index rebuild) |
| `go_parent` / `go_child` / `go_to` | O(1) |

## License

MIT
