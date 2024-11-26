use std::fmt;

use crate::storage::membership::Membership;
use crate::suraft::RequestVote;
use crate::suraft::VoteReply;
use crate::type_config::alias::OneshotSenderOf;
use crate::type_config::alias::ResponderOf;
use crate::TypeConfig;

/// A message sent by application to the [`Core`].
///
/// [`Core`]: crate::core::Core
pub(crate) enum APIMessage<C>
where C: TypeConfig
{
    RequestVote {
        rpc: RequestVote,
        tx: OneshotSenderOf<C, VoteReply>,
    },

    Write {
        app_data: C::AppData,
        tx: ResponderOf<C>,
    },

    ChangeMembership {
        membership: Membership,
        tx: OneshotSenderOf<C, ()>,
    },

    Elect,

    BroadcastHeartbeat,
}

impl<C> fmt::Display for APIMessage<C>
where C: TypeConfig
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            APIMessage::RequestVote { rpc, .. } => {
                write!(f, "RequestVote: {}", rpc)
            }
            APIMessage::Write { .. } => {
                write!(f, "Write")
            }
            APIMessage::ChangeMembership { membership, .. } => {
                write!(f, "ChangeMembership to: {}", membership)
            }
            APIMessage::Elect => {
                write!(f, "Elect")
            }
            APIMessage::BroadcastHeartbeat => {
                write!(f, "BroadcastHeartbeat")
            }
        }
    }
}
