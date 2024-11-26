use crate::base::display_ext::DisplayInstantExt;
use crate::quorum::progress::Progress;
use crate::quorum::progress::VecProgress;
use crate::storage::log::io_state::IOState;
use crate::storage::log::log_id::LogId;
use crate::storage::membership::Membership;
use crate::storage::membership::NodeId;
use crate::storage::vote::Vote;
use crate::type_config::alias::InstantOf;
use crate::type_config::alias::ResponderOf;
use crate::type_config::TypeConfigExt;
use crate::TypeConfig;

/// Leading state data.
///
/// Openraft leading state is the combination of Leader and Candidate in
/// original raft. A node becomes Leading at once when starting election,
/// although at this time, it can not propose any new log, because its `vote`
/// has not yet been granted by a quorum. I.e., A leader without commit vote is
/// a Candidate in original raft.
///
/// When the leader's vote is committed, i.e., granted by a quorum,
/// `Vote.committed` is set to true.
/// Then such a leader is the Leader in original raft.
///
/// By combining candidate and leader into one stage, suraft does not need to
/// lose leadership when a higher `leader_id`(roughly the `term` in original
/// raft) is seen. But instead it will be able to upgrade its `leader_id`
/// without losing leadership.
pub(crate) struct Leader<C>
where C: TypeConfig
{
    /// The vote this leader works in.
    ///
    /// `self.voting` may be in progress requesting vote for a higher vote.
    pub(crate) vote: Vote,

    /// The last log id proposed by the last leader.
    /// This log id may not be flushed to storage.
    pub(crate) last_log_id: Option<LogId>,

    pub(crate) membership: Membership,

    pub(crate) app_cmd_buf: Vec<C::AppData>,
    pub(crate) tx_buf: Vec<Option<ResponderOf<C>>>,
    pub(crate) io_state: IOState,

    /// The time to send next heartbeat.
    pub(crate) next_heartbeat: InstantOf<C>,

    /// Tracks the clock time acknowledged by other nodes.
    ///
    /// See [`docs::leader_lease`] for more details.
    ///
    /// [`docs::leader_lease`]: `crate::docs::protocol::replication::leader_lease`
    pub(crate) clock_progress: VecProgress<
        NodeId,
        Option<InstantOf<C>>,
        Option<InstantOf<C>>,
        Membership,
    >,
}

impl<C> Leader<C>
where C: TypeConfig
{
    /// Create a new Leader.
    ///
    /// `last_leader_log_id` is the first and last log id proposed by the last
    /// leader.
    pub(crate) fn new(
        vote: Vote,
        membership: Membership,
        last_log_id: Option<LogId>,
    ) -> Self {
        let leader = Self {
            vote: vote,
            app_cmd_buf: vec![],
            tx_buf: vec![],
            io_state: Default::default(),
            next_heartbeat: C::now(),
            membership: membership.clone(),
            clock_progress: VecProgress::new(membership, [], || None),
            last_log_id: last_log_id,
        };

        leader
    }

    pub(crate) fn committed_vote_ref(&self) -> &Vote {
        &self.vote
    }

    /// Get the last timestamp acknowledged by a quorum.
    ///
    /// The acknowledgement by remote nodes are updated when AppendEntries reply
    /// is received. But if the time of the leader itself is not updated.
    ///
    /// Therefore, everytime to retrieve the quorum acked timestamp, it should
    /// update with the leader's time first.
    /// It does not matter if the leader is not a voter, the QuorumSet will just
    /// ignore it.
    ///
    /// Note that the leader may not be in the QuorumSet at all.
    /// In such a case, the update operation will be just ignored,
    /// and the quorum-acked-time is totally determined by remove voters.
    pub(crate) fn last_quorum_acked_time(&mut self) -> Option<InstantOf<C>> {
        // For `Leading`, the vote is always the leader's vote.
        // Thus vote.voted_for() is this node.

        // Safe unwrap: voted_for() is always non-None in Openraft
        let node_id = self.vote.clone().voted_for().clone();
        let now = C::now();

        tracing::debug!(
            leader_id = display(&node_id),
            now = display(now.display()),
            "{}: update with leader's local time, before retrieving quorum acked clock",
            func_name!()
        );

        let granted = self.clock_progress.increase_to(&node_id, Some(now));

        match granted {
            Ok(x) => *x,
            // The leader node id may not be in the quorum set.
            Err(x) => *x,
        }
    }
}
