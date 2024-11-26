use std::collections::BTreeMap;
use std::fmt::Display;
use std::fmt::Formatter;

/// `NodeId` uniquely identifies a node within the suraft cluster.
///
/// Using a distinct type for `NodeId` enhances type safety and clarity.
/// It can be extended in the future to support more complex identifiers.
pub type NodeId = String;

/// `Node` represents a single participant in the suraft cluster.
///
/// Each node is uniquely identified by [`NodeId`] and contains necessary
/// information for cluster coordination and communication.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Node {
    /// The network address (e.g., IP and port) of the node for communication.
    pub address: String,

    /// Optional metadata or configuration data associated with the node.
    ///
    /// This can include information such as roles, capacities, or other custom
    /// attributes.
    pub metadata: BTreeMap<String, String>,
}

impl Node {
    pub fn new(address: impl ToString) -> Self {
        Node {
            address: address.to_string(),
            metadata: Default::default(),
        }
    }

    pub fn new_with_meta(
        address: impl ToString,
        metadata: BTreeMap<String, String>,
    ) -> Self {
        Node {
            address: address.to_string(),
            metadata,
        }
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}; ", self.address)?;
        for (i, (k, v)) in self.metadata.iter().enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{}:{}", k, v)?;
        }
        Ok(())
    }
}

impl Default for Node {
    // TODO: remove this.
    fn default() -> Self {
        Node {
            address: String::new(),
            metadata: BTreeMap::new(),
        }
    }
}
