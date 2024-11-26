use openraft_macros::add_async_trait;
use openraft_macros::since;

use crate::errors::NetworkError;
use crate::suraft::RequestVote;
use crate::suraft::VoteReply;
use crate::OptionalSend;
use crate::OptionalSync;
use crate::TypeConfig;

/// A trait defining the interface for a network between cluster members.
///
/// See the [network chapter of the
/// guide](crate::docs::getting_started#4-implement-raftnetwork) for details and
/// discussion on this trait and how to implement it.
///
/// A single network instance is used to connect to a single target node. The
/// network instance is constructed by the
/// [`RaftNetworkFactory`](`crate::network::RaftNetworkFactory`).
#[since(version = "0.10.0")]
#[add_async_trait]
pub trait Connection<C>: OptionalSend + OptionalSync + 'static
where C: TypeConfig
{
    /// Send a RequestVote RPC to the target.
    async fn request_vote(
        &mut self,
        rpc: RequestVote,
    ) -> Result<VoteReply, NetworkError>;
}
