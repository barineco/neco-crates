use alloc::vec::Vec;

use crate::error::GraphError;
use crate::id::{NodeId, PortId};
use crate::port::Port;

/// A graph node with payload and typed ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node<N> {
    id: NodeId,
    payload: N,
    ports: Vec<Port>,
}

impl<N> Node<N> {
    /// Creates a node with the provided identifier, payload, and ports.
    pub fn new(id: NodeId, payload: N, ports: Vec<Port>) -> Self {
        Self { id, payload, ports }
    }

    /// Returns the node identifier.
    pub const fn id(&self) -> NodeId {
        self.id
    }

    /// Returns the stored payload.
    pub fn payload(&self) -> &N {
        &self.payload
    }

    /// Returns mutable access to the stored payload.
    pub fn payload_mut(&mut self) -> &mut N {
        &mut self.payload
    }

    /// Returns the declared ports.
    pub fn ports(&self) -> &[Port] {
        self.ports.as_slice()
    }

    /// Adds a port when its identifier is not already present.
    pub fn add_port(&mut self, port: Port) -> Result<(), GraphError> {
        if self.ports.iter().any(|existing| existing.id() == port.id()) {
            return Err(GraphError::DuplicatePort(port.id().clone()));
        }
        self.ports.push(port);
        Ok(())
    }

    /// Returns the port with the given identifier.
    pub fn port(&self, port_id: &PortId) -> Option<&Port> {
        self.ports.iter().find(|port| port.id() == port_id)
    }
}
