use alloc::string::String;
use core::fmt;

use crate::id::{EdgeId, NodeId, PortId};

/// Graph operation failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    EmptyPortId,
    EmptyTypeTag,
    NodeNotFound(NodeId),
    EdgeNotFound(EdgeId),
    PortNotFound {
        node: NodeId,
        port: PortId,
    },
    DuplicatePort(PortId),
    PortDirectionMismatch,
    TypeTagMismatch {
        expected: String,
        actual: String,
    },
    SelfLoop,
    DuplicateEdge,
    #[cfg(feature = "json")]
    Json(neco_json::AccessError),
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPortId => f.write_str("port id must not be empty"),
            Self::EmptyTypeTag => f.write_str("port type_tag must not be empty"),
            Self::NodeNotFound(id) => write!(f, "node {id} not found"),
            Self::EdgeNotFound(id) => write!(f, "edge {id} not found"),
            Self::PortNotFound { node, port } => {
                write!(f, "port {port} not found on node {node}")
            }
            Self::DuplicatePort(port) => write!(f, "duplicate port id {port}"),
            Self::PortDirectionMismatch => {
                f.write_str("edge endpoints must connect output to input")
            }
            Self::TypeTagMismatch { expected, actual } => {
                write!(f, "port type mismatch: expected {expected}, got {actual}")
            }
            Self::SelfLoop => f.write_str("self-loop edges are not allowed"),
            Self::DuplicateEdge => f.write_str("duplicate edge is not allowed"),
            #[cfg(feature = "json")]
            Self::Json(error) => write!(f, "json access error: {error}"),
        }
    }
}

impl core::error::Error for GraphError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            #[cfg(feature = "json")]
            Self::Json(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(feature = "json")]
impl From<neco_json::AccessError> for GraphError {
    fn from(value: neco_json::AccessError) -> Self {
        Self::Json(value)
    }
}
