use crate::id::{EdgeId, NodeId, PortId};

/// A directed connection between two node ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge<E> {
    id: EdgeId,
    from: (NodeId, PortId),
    to: (NodeId, PortId),
    payload: E,
}

impl<E> Edge<E> {
    /// Creates an edge from two endpoint ports and a payload.
    pub fn new(id: EdgeId, from: (NodeId, PortId), to: (NodeId, PortId), payload: E) -> Self {
        Self {
            id,
            from,
            to,
            payload,
        }
    }

    /// Returns the edge identifier.
    pub const fn id(&self) -> EdgeId {
        self.id
    }

    /// Returns the source endpoint.
    pub fn from(&self) -> &(NodeId, PortId) {
        &self.from
    }

    /// Returns the destination endpoint.
    pub fn to(&self) -> &(NodeId, PortId) {
        &self.to
    }

    /// Returns the stored payload.
    pub fn payload(&self) -> &E {
        &self.payload
    }

    /// Returns mutable access to the stored payload.
    pub fn payload_mut(&mut self) -> &mut E {
        &mut self.payload
    }
}
