use std::fmt;

use crate::async_runtime::instant::Instant;
use crate::base::display_ext::DisplayBTreeMapOptValue;
use crate::base::display_ext::DisplayOption;
use crate::base::display_ext::DisplayOptionExt;
use crate::errors::Fatal;
use crate::metrics::HeartbeatMetrics;
use crate::metrics::ServerState;
use crate::storage::log::log_id::LogId;
use crate::storage::membership::NodeId;
use crate::storage::vote::Vote;
use crate::type_config::alias::SerdeInstantOf;
use crate::TypeConfig;

/// A set of metrics describing the current state of a SuRaft node.
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(bound = "")]
pub struct Metrics<C: TypeConfig> {
    pub running_state: Result<(), Fatal>,

    /// The ID of the SuRaft node.
    pub id: NodeId,

    /// The last flushed vote.
    pub vote: Option<Vote>,

    /// The last log index has been appended to this SuRaft node's log.
    pub committed: Option<LogId>,

    /// The state of the SuRaft node.
    pub server_state: ServerState,

    /// The current cluster leader.
    pub current_leader: Option<NodeId>,

    /// For a leader, it is the most recently acknowledged timestamp by a
    /// quorum.
    ///
    /// It is `None` if this node is not leader, or the leader is not yet
    /// acknowledged by a quorum. Being acknowledged means receiving a
    /// reply of `AppendEntries`(`AppendEntriesRequest.vote.committed ==
    /// true`). Receiving a reply of
    /// `RequestVote`(`RequestVote.vote.committed == false`) does not count,
    /// because a node will not maintain a lease for a vote with `committed ==
    /// false`.
    ///
    /// This timestamp can be used by the application to assess the likelihood
    /// that the leader has lost synchronization with the cluster.
    /// An older value may suggest a higher probability of the leader being
    /// partitioned from the cluster.
    pub last_quorum_acked: Option<SerdeInstantOf<C>>,

    /// Heartbeat metrics. It is Some() only when this node is leader.
    ///
    /// This field records a mapping between a node's ID and the time of the
    /// last acknowledged heartbeat or replication to this node.
    ///
    /// This duration since the recorded time can be used by applications to
    /// guess if a follower/learner node is offline, longer duration suggests
    /// higher possibility of that.
    pub heartbeat: Option<HeartbeatMetrics<C>>,
}

impl<C> fmt::Display for Metrics<C>
where C: TypeConfig
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Metrics{{")?;

        write!(
            f,
            "id:{}, {:?}, vote:{}, last_log:{}, leader:{}",
            self.id,
            self.server_state,
            self.vote.display(),
            DisplayOption(&self.committed),
            DisplayOption(&self.current_leader),
        )?;

        if let Some(quorum_acked) = &self.last_quorum_acked {
            write!(
                f,
                "(quorum_acked_time:{}, {:?} ago)",
                quorum_acked,
                quorum_acked.elapsed()
            )?;
        } else {
            write!(f, "(quorum_acked_time:None)")?;
        }

        write!(f, ", ")?;
        write!(
            f,
            "heartbeat:{{{}}}",
            DisplayOption(
                &self.heartbeat.as_ref().map(DisplayBTreeMapOptValue)
            ),
        )?;

        write!(f, "}}")?;
        Ok(())
    }
}

impl<C> Metrics<C>
where C: TypeConfig
{
    pub fn new_initial(id: NodeId) -> Self {
        #[allow(deprecated)]
        Self {
            running_state: Ok(()),
            id,

            vote: None,
            committed: None,

            server_state: ServerState::Follower,
            current_leader: None,
            last_quorum_acked: None,
            heartbeat: None,
        }
    }
}
