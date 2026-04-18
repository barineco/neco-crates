use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use neco_json::{AccessError, FromJson, JsonValue, ToJson};

use crate::edge::Edge;
use crate::error::GraphError;
use crate::graph::NodeGraph;
use crate::id::{EdgeId, NodeId, PortId};
use crate::node::Node;
use crate::port::{Port, PortDirection};

impl PortDirection {
    /// Converts the direction into a JSON string.
    pub fn to_json(&self) -> JsonValue {
        <Self as ToJson>::to_json(self)
    }

    /// Decodes a direction from JSON and wraps access failures.
    pub fn from_json(value: &JsonValue) -> Result<Self, GraphError> {
        <Self as FromJson>::from_json(value).map_err(GraphError::Json)
    }
}

impl ToJson for PortDirection {
    fn to_json(&self) -> JsonValue {
        JsonValue::String(
            match self {
                Self::Input => "Input",
                Self::Output => "Output",
            }
            .into(),
        )
    }
}

impl FromJson for PortDirection {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        match value.as_str() {
            Some("Input") => Ok(Self::Input),
            Some("Output") => Ok(Self::Output),
            _ => Err(AccessError::TypeMismatch {
                field: "direction".into(),
                expected: "\"Input\" or \"Output\"",
            }),
        }
    }
}

impl PortId {
    /// Converts the identifier into a JSON string.
    pub fn to_json(&self) -> JsonValue {
        <Self as ToJson>::to_json(self)
    }

    /// Decodes an identifier from JSON and wraps access failures.
    pub fn from_json(value: &JsonValue) -> Result<Self, GraphError> {
        <Self as FromJson>::from_json(value).map_err(GraphError::Json)
    }
}

impl ToJson for PortId {
    fn to_json(&self) -> JsonValue {
        JsonValue::String(self.as_str().into())
    }
}

impl FromJson for PortId {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        let raw = value.as_str().ok_or_else(|| AccessError::TypeMismatch {
            field: String::new(),
            expected: "string",
        })?;
        crate::id::PortId::new(raw).map_err(|_| AccessError::TypeMismatch {
            field: "id".into(),
            expected: "non-empty string",
        })
    }
}

impl ToJson for NodeId {
    fn to_json(&self) -> JsonValue {
        self.as_u64().to_json()
    }
}

impl FromJson for NodeId {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        Ok(Self::new(u64::from_json(value)?))
    }
}

impl ToJson for EdgeId {
    fn to_json(&self) -> JsonValue {
        self.as_u64().to_json()
    }
}

impl FromJson for EdgeId {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        Ok(Self::new(u64::from_json(value)?))
    }
}

impl Port {
    /// Converts the port into a JSON object.
    pub fn to_json(&self) -> JsonValue {
        <Self as ToJson>::to_json(self)
    }

    /// Decodes a port from JSON and wraps access failures.
    pub fn from_json(value: &JsonValue) -> Result<Self, GraphError> {
        <Self as FromJson>::from_json(value).map_err(GraphError::Json)
    }
}

impl ToJson for Port {
    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("id".into(), self.id().to_json()),
            ("direction".into(), self.direction().to_json()),
            ("type_tag".into(), self.type_tag().to_json()),
        ])
    }
}

impl FromJson for Port {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        let id = <PortId as FromJson>::from_json(required_field(value, "id")?)?;
        let direction =
            <PortDirection as FromJson>::from_json(required_field(value, "direction")?)?;
        let type_tag = value.required_str("type_tag")?;
        Port::new(id, direction, type_tag).map_err(|_| AccessError::TypeMismatch {
            field: "type_tag".into(),
            expected: "non-empty string",
        })
    }
}

impl<N: ToJson> ToJson for Node<N> {
    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("id".into(), self.id().to_json()),
            ("payload".into(), self.payload().to_json()),
            ("ports".into(), self.ports().to_vec().to_json()),
        ])
    }
}

impl<N: FromJson> FromJson for Node<N> {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        let id = NodeId::from_json(required_field(value, "id")?)?;
        let payload = N::from_json(required_field(value, "payload")?)?;
        let ports = Vec::<Port>::from_json(required_field(value, "ports")?)?;
        Ok(Node::new(id, payload, ports))
    }
}

impl<E: ToJson> ToJson for Edge<E> {
    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("id".into(), self.id().to_json()),
            ("from".into(), endpoint_to_json(self.from())),
            ("to".into(), endpoint_to_json(self.to())),
            ("payload".into(), self.payload().to_json()),
        ])
    }
}

impl<E: FromJson> FromJson for Edge<E> {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        let id = EdgeId::from_json(required_field(value, "id")?)?;
        let from = endpoint_from_json(required_field(value, "from")?, "from")?;
        let to = endpoint_from_json(required_field(value, "to")?, "to")?;
        let payload = E::from_json(required_field(value, "payload")?)?;
        Ok(Edge::new(id, from, to, payload))
    }
}

impl<N: ToJson, E: ToJson> NodeGraph<N, E> {
    /// Converts the graph into a stable JSON object.
    pub fn to_json(&self) -> JsonValue {
        <Self as ToJson>::to_json(self)
    }
}

impl<N: FromJson, E: FromJson> NodeGraph<N, E> {
    /// Decodes a graph from JSON and wraps access failures.
    pub fn from_json(value: &JsonValue) -> Result<Self, GraphError> {
        <Self as FromJson>::from_json(value).map_err(GraphError::Json)
    }
}

impl<N: ToJson, E: ToJson> ToJson for NodeGraph<N, E> {
    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "nodes".into(),
                JsonValue::Array(self.nodes.values().map(ToJson::to_json).collect()),
            ),
            (
                "edges".into(),
                JsonValue::Array(self.edges.values().map(ToJson::to_json).collect()),
            ),
            ("next_node_id".into(), self.next_node_id.to_json()),
            ("next_edge_id".into(), self.next_edge_id.to_json()),
        ])
    }
}

impl<N: FromJson, E: FromJson> FromJson for NodeGraph<N, E> {
    fn from_json(value: &JsonValue) -> Result<Self, AccessError> {
        let nodes = Vec::<Node<N>>::from_json(required_field(value, "nodes")?)?;
        let edges = Vec::<Edge<E>>::from_json(required_field(value, "edges")?)?;
        let next_node_id = u64::from_json(required_field(value, "next_node_id")?)?;
        let next_edge_id = u64::from_json(required_field(value, "next_edge_id")?)?;

        Ok(Self {
            nodes: nodes
                .into_iter()
                .map(|node| (node.id(), node))
                .collect::<BTreeMap<_, _>>(),
            edges: edges
                .into_iter()
                .map(|edge| (edge.id(), edge))
                .collect::<BTreeMap<_, _>>(),
            next_node_id,
            next_edge_id,
        })
    }
}

fn required_field<'a>(value: &'a JsonValue, key: &str) -> Result<&'a JsonValue, AccessError> {
    value
        .get(key)
        .ok_or_else(|| AccessError::MissingField(key.to_string()))
}

fn endpoint_to_json(endpoint: &(NodeId, PortId)) -> JsonValue {
    JsonValue::Array(vec![endpoint.0.to_json(), endpoint.1.to_json()])
}

fn endpoint_from_json(value: &JsonValue, field: &str) -> Result<(NodeId, PortId), AccessError> {
    let items = value.as_array().ok_or_else(|| AccessError::TypeMismatch {
        field: field.into(),
        expected: "array",
    })?;

    if items.len() != 2 {
        return Err(AccessError::TypeMismatch {
            field: field.into(),
            expected: "[node_id, port_id]",
        });
    }

    Ok((
        <NodeId as FromJson>::from_json(&items[0])?,
        <PortId as FromJson>::from_json(&items[1])?,
    ))
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use neco_json::JsonValue;

    use crate::{GraphError, NodeGraph, Port, PortDirection, PortId};

    fn port(id: &str, direction: PortDirection, type_tag: &str) -> Port {
        Port::new(PortId::new(id).expect("port id"), direction, type_tag).expect("port")
    }

    fn sample_graph() -> NodeGraph<String, u64> {
        let mut graph = NodeGraph::new();
        let source = graph.add_node_with_ports(
            "source".into(),
            vec![port("out", PortDirection::Output, "Fact")],
        );
        let target = graph.add_node_with_ports(
            "target".into(),
            vec![port("in", PortDirection::Input, "Fact")],
        );
        graph
            .add_edge(
                (source, PortId::new("out").unwrap()),
                (target, PortId::new("in").unwrap()),
                7,
            )
            .unwrap();
        graph
    }

    #[test]
    fn json_roundtrip_preserves_graph() {
        let graph = sample_graph();

        let json = graph.to_json();
        let restored = NodeGraph::<String, u64>::from_json(&json).expect("from_json");

        assert_eq!(restored, graph);
    }

    #[test]
    fn port_direction_serializes_as_string() {
        assert_eq!(
            PortDirection::Input.to_json(),
            JsonValue::String("Input".into())
        );
        assert_eq!(
            PortDirection::Output.to_json(),
            JsonValue::String("Output".into())
        );
    }

    #[test]
    fn missing_field_is_reported_as_graph_error() {
        let json = JsonValue::Object(vec![
            ("nodes".into(), JsonValue::Array(vec![])),
            ("edges".into(), JsonValue::Array(vec![])),
            ("next_node_id".into(), JsonValue::Number(0.0)),
        ]);

        let error = NodeGraph::<String, u64>::from_json(&json).expect_err("missing field");

        assert!(matches!(error, GraphError::Json(_)));
    }

    #[test]
    fn invalid_port_direction_is_reported() {
        let json = JsonValue::String("Sideways".into());

        let error = PortDirection::from_json(&json).expect_err("invalid direction");

        assert!(matches!(error, GraphError::Json(_)));
    }
}
