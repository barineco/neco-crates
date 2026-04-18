use alloc::string::{String, ToString};

use crate::error::GraphError;
use crate::id::PortId;

/// Port direction used for edge validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    Input,
    Output,
}

/// A typed port belonging to a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Port {
    id: PortId,
    direction: PortDirection,
    type_tag: String,
}

impl Port {
    /// Creates a port. `type_tag` must not be empty.
    pub fn new(id: PortId, direction: PortDirection, type_tag: &str) -> Result<Self, GraphError> {
        if type_tag.is_empty() {
            return Err(GraphError::EmptyTypeTag);
        }

        Ok(Self {
            id,
            direction,
            type_tag: type_tag.to_string(),
        })
    }

    /// Returns the port identifier.
    pub fn id(&self) -> &PortId {
        &self.id
    }

    /// Returns the direction used for connection validation.
    pub const fn direction(&self) -> PortDirection {
        self.direction
    }

    /// Returns the application-defined type tag.
    pub fn type_tag(&self) -> &str {
        self.type_tag.as_str()
    }
}
