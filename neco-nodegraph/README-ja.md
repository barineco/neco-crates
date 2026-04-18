# neco-nodegraph

[English](README.md)

描画非依存のノードグラフモデルです。`json` feature で `neco-json` による読み書きに対応します。

## 使い方

```rust
use neco_nodegraph::{NodeGraph, Port, PortDirection, PortId};

let mut graph = NodeGraph::<&str, &str>::new();
let source = graph.add_node_with_ports(
    "source",
    vec![Port::new(PortId::new("out")?, PortDirection::Output, "Fact")?],
);
let target = graph.add_node_with_ports(
    "target",
    vec![Port::new(PortId::new("in")?, PortDirection::Input, "Fact")?],
);
let edge = graph.add_edge(
    (source, PortId::new("out")?),
    (target, PortId::new("in")?),
    "edge",
)?;

assert_eq!(graph.connected((source, PortId::new("out")?)), vec![edge]);
assert_eq!(graph.remove_node(target)?.payload(), &"target");
# Ok::<(), neco_nodegraph::GraphError>(())
```

## API

| 項目 | 説明 |
|------|------|
| `NodeGraph<N, E>` | グラフ本体。`N` がノード、`E` がエッジの付加データ |
| `Node<N>` | 識別子、付加データ、宣言済みポートを持つノード |
| `Edge<E>` | 出力ポートから入力ポートへの有向接続 |
| `Port` | 識別子、方向、型タグを持つポート定義 |
| `PortDirection` | `Input` / `Output` の方向 |
| `NodeId`, `EdgeId`, `PortId` | 各要素の不透明な識別子 |
| `GraphError` | ノード不在、不正な接続先、JSON 復号失敗を返すエラー型 |

## Feature

| Feature | 説明 |
|---------|------|
| `default` | 追加依存なし。`no_std` + `alloc` のグラフモデルのみ |
| `json` | `neco-json` による `to_json` / `from_json` を有効化 |

## ライセンス

MIT
