#![no_std]

//! necosystems series node graph data model with port-typed nodes and edges.
//!
//! `NodeGraph<N, E>` provides a pure graph model for node editors. Edges are
//! validated against port existence, direction, and `type_tag`, and the
//! optional `json` feature enables `neco-json` based round-trip codec support.

extern crate alloc;

pub mod edge;
pub mod error;
pub mod graph;
pub mod id;
#[cfg(feature = "json")]
mod json;
pub mod node;
pub mod port;

pub use edge::Edge;
pub use error::GraphError;
pub use graph::NodeGraph;
pub use id::{EdgeId, NodeId, PortId};
pub use node::Node;
pub use port::{Port, PortDirection};
