use core::fmt;
use std::collections::BTreeMap;

use crate::errors::ChangeMembershipError;
use crate::errors::EmptyMembership;
use crate::quorum::QuorumSet;
use crate::storage::membership::IntoNodes;
use crate::storage::membership::NodeId;
use crate::Node;

/// The membership configuration of the cluster.
///
/// It could be a joint of one, two or more configs, i.e., a quorum is a node
/// set that is superset of a majority of every config.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Membership {
    /// Additional info of all nodes, e.g., the connecting host and port.
    ///
    /// A node-id key that is in `nodes` but is not in `configs` is a
    /// **learner**.
    nodes: BTreeMap<NodeId, Node>,
}

impl From<BTreeMap<NodeId, Node>> for Membership {
    fn from(b: BTreeMap<NodeId, Node>) -> Self {
        Membership::new(b)
    }
}

impl fmt::Display for Membership {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{",)?;

        for (i, (node_id, node)) in self.nodes.iter().enumerate() {
            if i > 0 {
                write!(f, ",",)?;
            }
            write!(f, "{node_id}:{node:?}")?;
        }

        write!(f, "}}")?;
        Ok(())
    }
}

// Public APIs
impl Membership {
    /// Create a new Membership from a joint config of voter-ids and a
    /// collection of all `Node`(voter nodes and learner nodes).
    ///
    /// A node id that is in `nodes` but is not in `config` is a **learner**.
    ///
    /// A node presents in `config` but not in `nodes` is filled with default
    /// value.
    ///
    /// The `nodes` can be:
    /// - a simple `()`, if there are no learner nodes,
    /// - `BTreeMap<NodeId, Node>` provides nodes for every node id. Node ids
    ///   that are not in `configs` are learners.
    pub fn new<T>(nodes: T) -> Self
    where T: IntoNodes {
        let nodes = nodes.into_nodes();

        Membership { nodes }
    }

    /// Returns an Iterator of all nodes(voters and learners).
    pub fn nodes(&self) -> impl Iterator<Item = (&NodeId, &Node)> {
        self.nodes.iter()
    }

    /// Get a node(either voter or learner) by node id.
    pub fn get_node(&self, node_id: &NodeId) -> Option<&Node> {
        self.nodes.get(node_id)
    }

    /// Returns an Iterator of all voter node ids. Learners are not included.
    pub fn node_ids(&self) -> impl Iterator<Item = NodeId> {
        self.nodes.clone().into_keys()
    }
}

impl Membership {
    /// Check if the given `NodeId` exists and is a voter.
    #[allow(dead_code)]
    pub(crate) fn contains(&self, node_id: &NodeId) -> bool {
        self.nodes.contains_key(node_id)
    }

    /// Ensure the membership config is valid:
    /// - No empty sub-config in it.
    /// - Every voter has a corresponding Node.
    #[allow(dead_code)]
    pub(crate) fn ensure_valid(&self) -> Result<(), ChangeMembershipError> {
        self.ensure_non_empty_config()?;
        Ok(())
    }

    /// Ensures that none of the sub config in this joint config are empty.
    pub(crate) fn ensure_non_empty_config(
        &self,
    ) -> Result<(), EmptyMembership> {
        if self.nodes.is_empty() {
            return Err(EmptyMembership {});
        }

        Ok(())
    }
}

impl QuorumSet<NodeId> for Membership {
    type Iter = std::collections::btree_map::IntoKeys<NodeId, Node>;

    fn is_quorum<'a, I: Iterator<Item = &'a NodeId> + Clone>(
        &self,
        ids: I,
    ) -> bool {
        self.nodes.is_quorum(ids)
    }

    fn ids(&self) -> Self::Iter {
        self.nodes.ids()
    }
}
