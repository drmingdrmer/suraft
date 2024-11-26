use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use suraft::errors::NetworkError;
use suraft::network::connection::Connection;
use suraft::suraft::RequestVote;
use suraft::suraft::VoteReply;
use suraft::Network;
use suraft::Node;
use suraft::NodeId;
use suraft::SuRaft;
use suraft::TypeConfig;

#[derive(Default, Clone)]
pub struct DirectNetwork<C>
where C: TypeConfig
{
    peers: Arc<Mutex<BTreeMap<NodeId, SuRaft<C>>>>,
}

impl<C> DirectNetwork<C>
where C: TypeConfig
{
    pub fn add_peer(&self, node_id: NodeId, su_raft: SuRaft<C>) {
        let mut peers = self.peers.lock().unwrap();
        peers.insert(node_id, su_raft);
    }

    pub fn remove_peer(&self, node_id: NodeId) {
        let mut peers = self.peers.lock().unwrap();
        peers.remove(&node_id);
    }

    pub fn get_peer(&self, node_id: &NodeId) -> Option<SuRaft<C>> {
        let peers = self.peers.lock().unwrap();
        peers.get(node_id).cloned()
    }
}

impl<C> Network<C> for DirectNetwork<C>
where C: TypeConfig
{
    type Connection = Conn<C>;

    async fn new_connection(
        &mut self,
        target: NodeId,
        _node: &Node,
    ) -> Self::Connection {
        let peers = self.peers.lock().unwrap();
        let su = peers.get(&target).unwrap().clone();
        Conn {
            _node_id: target,
            su_raft: su,
        }
    }
}

pub struct Conn<C>
where C: TypeConfig
{
    _node_id: NodeId,
    su_raft: SuRaft<C>,
}

impl<C> Connection<C> for Conn<C>
where C: TypeConfig
{
    async fn request_vote(
        &mut self,
        rpc: RequestVote,
    ) -> Result<VoteReply, NetworkError> {
        self.su_raft
            .handle_request_vote(rpc)
            .await
            .map_err(|e| NetworkError::new(&e))
    }
}
