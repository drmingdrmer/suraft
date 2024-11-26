use std::collections::BTreeMap;
use std::collections::BTreeSet;

use maplit::btreemap;

use crate::storage::membership::NodeId;
use crate::Node;

/// Convert into a map of `Node`.
///
/// This is used as a user input acceptor when building a Membership, to convert
/// various input types into a map of `Node`.
pub trait IntoNodes {
    fn into_nodes(self) -> BTreeMap<NodeId, Node>;
}

impl IntoNodes for () {
    fn into_nodes(self) -> BTreeMap<NodeId, Node> {
        btreemap! {}
    }
}

impl IntoNodes for Option<BTreeSet<NodeId>> {
    fn into_nodes(self) -> BTreeMap<NodeId, Node> {
        match self {
            Some(set) => set.into_nodes(),
            None => btreemap! {},
        }
    }
}
impl IntoNodes for BTreeSet<NodeId> {
    fn into_nodes(self) -> BTreeMap<NodeId, Node> {
        self.into_iter().map(|id| (id, Node::new(""))).collect()
    }
}

impl IntoNodes for BTreeMap<NodeId, Node> {
    fn into_nodes(self) -> BTreeMap<NodeId, Node> {
        self
    }
}
