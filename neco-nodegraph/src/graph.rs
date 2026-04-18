use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::edge::Edge;
use crate::error::GraphError;
use crate::id::{EdgeId, NodeId, PortId};
use crate::node::Node;
use crate::port::{Port, PortDirection};

/// Pure node graph data model with typed ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeGraph<N, E> {
    pub(crate) nodes: BTreeMap<NodeId, Node<N>>,
    pub(crate) edges: BTreeMap<EdgeId, Edge<E>>,
    pub(crate) next_node_id: u64,
    pub(crate) next_edge_id: u64,
}

impl<N, E> Default for NodeGraph<N, E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<N, E> NodeGraph<N, E> {
    /// Creates an empty graph.
    pub fn new() -> Self {
        Self {
            nodes: BTreeMap::new(),
            edges: BTreeMap::new(),
            next_node_id: 0,
            next_edge_id: 0,
        }
    }

    /// Adds a node without ports and returns its identifier.
    pub fn add_node(&mut self, payload: N) -> NodeId {
        self.add_node_with_ports(payload, Vec::new())
    }

    /// Adds a node with declared ports and returns its identifier.
    pub fn add_node_with_ports(&mut self, payload: N, ports: Vec<Port>) -> NodeId {
        let id = NodeId::new(self.next_node_id);
        self.next_node_id += 1;
        let node = Node::new(id, payload, ports);
        self.nodes.insert(id, node);
        id
    }

    /// Removes a node and all connected edges.
    pub fn remove_node(&mut self, id: NodeId) -> Result<Node<N>, GraphError> {
        let connected = self.connected_edge_ids_for_node(id)?;
        let removed = self.nodes.remove(&id).ok_or(GraphError::NodeNotFound(id))?;
        for edge_id in connected {
            let _ = self.edges.remove(&edge_id);
        }
        Ok(removed)
    }

    /// Adds a validated directed edge between two ports.
    pub fn add_edge(
        &mut self,
        from: (NodeId, PortId),
        to: (NodeId, PortId),
        payload: E,
    ) -> Result<EdgeId, GraphError> {
        if from.0 == to.0 {
            return Err(GraphError::SelfLoop);
        }

        let from_port = self.resolve_port(&from)?;
        let to_port = self.resolve_port(&to)?;

        if from_port.direction() != PortDirection::Output
            || to_port.direction() != PortDirection::Input
        {
            return Err(GraphError::PortDirectionMismatch);
        }

        if from_port.type_tag() != to_port.type_tag() {
            return Err(GraphError::TypeTagMismatch {
                expected: from_port.type_tag().into(),
                actual: to_port.type_tag().into(),
            });
        }

        if self
            .edges
            .values()
            .any(|edge| edge.from() == &from && edge.to() == &to)
        {
            return Err(GraphError::DuplicateEdge);
        }

        let id = EdgeId::new(self.next_edge_id);
        self.next_edge_id += 1;
        self.edges.insert(id, Edge::new(id, from, to, payload));
        Ok(id)
    }

    /// Removes an edge by identifier.
    pub fn remove_edge(&mut self, id: EdgeId) -> Result<Edge<E>, GraphError> {
        self.edges.remove(&id).ok_or(GraphError::EdgeNotFound(id))
    }

    /// Returns an immutable node reference.
    pub fn node(&self, id: NodeId) -> Option<&Node<N>> {
        self.nodes.get(&id)
    }

    /// Returns a mutable node reference.
    pub fn node_mut(&mut self, id: NodeId) -> Option<&mut Node<N>> {
        self.nodes.get_mut(&id)
    }

    /// Returns an immutable edge reference.
    pub fn edge(&self, id: EdgeId) -> Option<&Edge<E>> {
        self.edges.get(&id)
    }

    /// Iterates over nodes in identifier order.
    pub fn nodes(&self) -> impl Iterator<Item = (&NodeId, &Node<N>)> {
        self.nodes.iter()
    }

    /// Iterates over edges in identifier order.
    pub fn edges(&self) -> impl Iterator<Item = (&EdgeId, &Edge<E>)> {
        self.edges.iter()
    }

    /// Returns all edges connected to the given port.
    pub fn connected(&self, port: (NodeId, PortId)) -> Vec<EdgeId> {
        self.edges
            .iter()
            .filter_map(|(edge_id, edge)| {
                ((edge.from() == &port) || (edge.to() == &port)).then_some(*edge_id)
            })
            .collect()
    }

    /// Returns all incoming edge identifiers for a node.
    pub fn incoming(&self, node: NodeId) -> Vec<EdgeId> {
        self.edges
            .iter()
            .filter_map(|(edge_id, edge)| (edge.to().0 == node).then_some(*edge_id))
            .collect()
    }

    /// Returns all outgoing edge identifiers for a node.
    pub fn outgoing(&self, node: NodeId) -> Vec<EdgeId> {
        self.edges
            .iter()
            .filter_map(|(edge_id, edge)| (edge.from().0 == node).then_some(*edge_id))
            .collect()
    }

    fn resolve_port(&self, endpoint: &(NodeId, PortId)) -> Result<&Port, GraphError> {
        let node = self
            .nodes
            .get(&endpoint.0)
            .ok_or(GraphError::NodeNotFound(endpoint.0))?;
        node.port(&endpoint.1)
            .ok_or_else(|| GraphError::PortNotFound {
                node: endpoint.0,
                port: endpoint.1.clone(),
            })
    }

    fn connected_edge_ids_for_node(&self, node: NodeId) -> Result<Vec<EdgeId>, GraphError> {
        if !self.nodes.contains_key(&node) {
            return Err(GraphError::NodeNotFound(node));
        }

        Ok(self
            .edges
            .iter()
            .filter_map(|(edge_id, edge)| {
                ((edge.from().0 == node) || (edge.to().0 == node)).then_some(*edge_id)
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use super::NodeGraph;
    use crate::error::GraphError;
    use crate::id::PortId;
    use crate::port::{Port, PortDirection};

    fn port(id: &str, direction: PortDirection, type_tag: &str) -> Port {
        Port::new(PortId::new(id).expect("port id"), direction, type_tag).expect("port")
    }

    #[test]
    fn node_ids_increment_from_zero() {
        let mut graph = NodeGraph::<(), ()>::new();
        let first = graph.add_node(());
        let second = graph.add_node(());

        assert_eq!(first.as_u64(), 0);
        assert_eq!(second.as_u64(), 1);
    }

    #[test]
    fn remove_missing_node_returns_error() {
        let mut graph = NodeGraph::<(), ()>::new();

        let error = graph
            .remove_node(crate::NodeId::new(9))
            .expect_err("missing node");

        assert_eq!(error, GraphError::NodeNotFound(crate::NodeId::new(9)));
    }

    #[test]
    fn remove_missing_edge_returns_error() {
        let mut graph = NodeGraph::<(), ()>::new();

        let error = graph
            .remove_edge(crate::EdgeId::new(4))
            .expect_err("missing edge");

        assert_eq!(error, GraphError::EdgeNotFound(crate::EdgeId::new(4)));
    }

    #[test]
    fn add_edge_requires_output_to_input() {
        let mut graph = NodeGraph::<(), ()>::new();
        let source = graph.add_node_with_ports((), vec![port("in", PortDirection::Input, "Fact")]);
        let target =
            graph.add_node_with_ports((), vec![port("out", PortDirection::Output, "Fact")]);

        let error = graph
            .add_edge(
                (source, PortId::new("in").unwrap()),
                (target, PortId::new("out").unwrap()),
                (),
            )
            .expect_err("direction mismatch");

        assert_eq!(error, GraphError::PortDirectionMismatch);
    }
}
