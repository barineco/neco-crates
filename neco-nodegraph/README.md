# neco-nodegraph

[日本語](README-ja.md)

necosystems series node graph data model with port-typed nodes and edges

## Usage

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

| Item | Description |
|------|-------------|
| `NodeGraph<N, E>` | Graph container with typed node payloads, edge payloads, and port validation |
| `Node<N>` | Node record with id, payload, and declared ports |
| `Edge<E>` | Directed edge between an output port and an input port |
| `Port` | Port definition with id, direction, and application-defined `type_tag` |
| `PortDirection` | `Input` or `Output` direction used during edge validation |
| `NodeId`, `EdgeId`, `PortId` | Opaque identifiers for nodes, edges, and ports |
| `GraphError` | Failure type for missing nodes, invalid endpoints, and JSON decode errors |

## Features

| Feature | Description |
|---------|-------------|
| `default` | No optional dependencies. Pure `no_std` + `alloc` graph model |
| `json` | Enables `neco-json` based `to_json` / `from_json` round-trip support |

## License

MIT
