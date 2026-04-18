use alloc::string::{String, ToString};
use core::fmt;

use crate::error::GraphError;

/// Opaque identifier for a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(u64);

impl NodeId {
    /// Creates a node identifier from a raw value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw numeric value.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Opaque identifier for an edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeId(u64);

impl EdgeId {
    /// Creates an edge identifier from a raw value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the raw numeric value.
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl fmt::Display for EdgeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Opaque identifier for a node port.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PortId(String);

impl PortId {
    /// Creates a port identifier. Empty strings are rejected.
    pub fn new(value: &str) -> Result<Self, GraphError> {
        if value.is_empty() {
            return Err(GraphError::EmptyPortId);
        }
        Ok(Self(value.to_string()))
    }

    /// Returns the underlying string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for PortId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
