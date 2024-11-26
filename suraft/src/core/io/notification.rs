use std::fmt;

use futures::io;

use crate::base::display_ext::DisplayInstantExt;
use crate::storage::log::log_id::LogId;
use crate::storage::membership::NodeId;
use crate::storage::vote::Vote;
use crate::suraft::VoteReply;
use crate::type_config::alias::InstantOf;
use crate::TypeConfig;

/// A message coming from the internal components.
pub(crate) enum Notification<C>
where C: TypeConfig
{
    RequestVoteReply {
        target: NodeId,
        reply: VoteReply,

        /// The candidate that sent the vote request.
        ///
        /// A vote identifies a unique server state.
        sender_vote: Vote,
    },

    /// io::Error has taken place locally(not on remote node),
    /// and [`Core`](`crate::core::Core`) needs to shut down.
    StorageError { error: io::Error },

    /// Completion of an IO operation to local store.
    IOCompleted {
        log_id: LogId,
        responders: Vec<Option<C::Responder>>,
        done: bool,
    },

    HeartbeatReply {
        leader_vote: Vote,
        sending_time: InstantOf<C>,
        target: NodeId,
    },

    /// A tick event to wake up Core to check timeout etc.
    Tick {
        /// ith tick
        i: u64,
    },
}

impl<C> Notification<C>
where C: TypeConfig
{
    pub(crate) fn set_io_result(&mut self, done: bool) {
        match self {
            Self::IOCompleted {
                done: ref mut d, ..
            } => {
                *d = done;
            }
            _ => {
                unreachable!("Unexpected notification: {}", self);
            }
        }
    }
}

impl<C> fmt::Display for Notification<C>
where C: TypeConfig
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestVoteReply {
                target,
                reply: resp,
                sender_vote: candidate_vote,
            } => {
                write!(
                    f,
                    "VoteResponse: from target={}, to candidate_vote: {}, {}",
                    target, candidate_vote, resp
                )
            }
            Self::StorageError { error } => {
                write!(f, "StorageError: {}", error)
            }
            Self::IOCompleted {
                log_id: io_id,
                done,
                ..
            } => {
                write!(f, "IOFlushed: {}; done: {}", io_id, done)
            }
            Self::HeartbeatReply {
                leader_vote,
                sending_time,
                target,
            } => {
                write!(
                    f,
                    "HeartbeatProgress: target={}, leader_vote: {}, sending_time: {}",
                    target,
                    leader_vote,
                    sending_time.display(),
                )
            }
            Self::Tick { i } => {
                write!(f, "Tick {}", i)
            }
        }
    }
}
