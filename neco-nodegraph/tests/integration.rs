use neco_nodegraph::{GraphError, NodeGraph, Port, PortDirection, PortId};

fn port(id: &str, direction: PortDirection, type_tag: &str) -> Port {
    Port::new(PortId::new(id).expect("port id"), direction, type_tag).expect("port")
}

#[test]
fn add_and_query_edges() {
    let mut graph = NodeGraph::<&'static str, &'static str>::new();
    let source =
        graph.add_node_with_ports("source", vec![port("out", PortDirection::Output, "Fact")]);
    let target =
        graph.add_node_with_ports("target", vec![port("in", PortDirection::Input, "Fact")]);

    let edge_id = graph
        .add_edge(
            (source, PortId::new("out").expect("out id")),
            (target, PortId::new("in").expect("in id")),
            "edge",
        )
        .expect("edge must be added");

    assert_eq!(
        graph.connected((source, PortId::new("out").unwrap())),
        vec![edge_id]
    );
    assert_eq!(graph.incoming(target), vec![edge_id]);
    assert_eq!(graph.outgoing(source), vec![edge_id]);
    assert_eq!(graph.edge(edge_id).expect("edge").payload(), &"edge");
}

#[test]
fn remove_node_cascades_connected_edges() {
    let mut graph = NodeGraph::<u64, u64>::new();
    let source = graph.add_node_with_ports(1, vec![port("out", PortDirection::Output, "Fact")]);
    let target = graph.add_node_with_ports(2, vec![port("in", PortDirection::Input, "Fact")]);
    let edge_id = graph
        .add_edge(
            (source, PortId::new("out").unwrap()),
            (target, PortId::new("in").unwrap()),
            99,
        )
        .unwrap();

    let removed = graph.remove_node(target).expect("remove node");

    assert_eq!(removed.payload(), &2);
    assert!(graph.edge(edge_id).is_none());
    assert!(graph
        .connected((source, PortId::new("out").unwrap()))
        .is_empty());
}

#[test]
fn rejects_invalid_edges() {
    let mut graph = NodeGraph::<(), ()>::new();
    let a = graph.add_node_with_ports((), vec![port("out", PortDirection::Output, "Fact")]);
    let b = graph.add_node_with_ports((), vec![port("in", PortDirection::Input, "Other")]);

    let mismatch = graph
        .add_edge(
            (a, PortId::new("out").unwrap()),
            (b, PortId::new("in").unwrap()),
            (),
        )
        .expect_err("type mismatch must fail");
    assert!(matches!(mismatch, GraphError::TypeTagMismatch { .. }));

    let self_loop = graph
        .add_edge(
            (a, PortId::new("out").unwrap()),
            (a, PortId::new("out").unwrap()),
            (),
        )
        .expect_err("self loop must fail");
    assert_eq!(self_loop, GraphError::SelfLoop);
}

#[test]
fn rejects_duplicate_edge() {
    let mut graph = NodeGraph::<(), ()>::new();
    let source = graph.add_node_with_ports((), vec![port("out", PortDirection::Output, "Fact")]);
    let target = graph.add_node_with_ports((), vec![port("in", PortDirection::Input, "Fact")]);

    graph
        .add_edge(
            (source, PortId::new("out").unwrap()),
            (target, PortId::new("in").unwrap()),
            (),
        )
        .unwrap();

    let duplicate = graph
        .add_edge(
            (source, PortId::new("out").unwrap()),
            (target, PortId::new("in").unwrap()),
            (),
        )
        .expect_err("duplicate must fail");

    assert_eq!(duplicate, GraphError::DuplicateEdge);
}
