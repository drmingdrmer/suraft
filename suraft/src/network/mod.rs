//! The SuRaft network interface.

use openraft_macros::add_async_trait;

use crate::network::connection::Connection;
use crate::storage::membership::NodeId;
use crate::Node;
use crate::OptionalSend;
use crate::OptionalSync;
use crate::TypeConfig;

pub mod connection;

/// A trait defining the interface for a SuRaft network factory to create
/// connections between cluster members.
///
/// See the [network chapter of the
/// guide](crate::docs::getting_started#4-implement-raftnetwork) for details and
/// discussion on this trait and how to implement it.
///
/// Typically, the network implementation as such will be hidden behind a
/// `Box<T>` or `Arc<T>` and this interface implemented on the `Box<T>` or
/// `Arc<T>`.
#[add_async_trait]
pub trait Network<C>: OptionalSend + OptionalSync + 'static
where C: TypeConfig
{
    /// Actual type of the network handling a single connection.
    type Connection: Connection<C>;

    /// Create a new network instance sending RPCs to the target node.
    ///
    /// This function should **not** create a connection but rather a client
    /// that will connect when required. Therefore, there is chance it will
    /// build a client that is unable to send out anything, e.g., in case
    /// the Node network address is configured incorrectly. But this method
    /// does not return an error because suraft can only ignore it.
    ///
    /// The method is intentionally async to give the implementation a chance to
    /// use asynchronous sync primitives to serialize access to the common
    /// internal object, if needed.
    async fn new_connection(
        &mut self,
        target: NodeId,
        node: &Node,
    ) -> Self::Connection;
}
