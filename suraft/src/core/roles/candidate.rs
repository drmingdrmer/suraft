use std::fmt;

use tracing::info;

use crate::base::display_ext::DisplayInstantExt;
use crate::quorum::progress::Progress;
use crate::quorum::progress::VecProgress;
use crate::storage::log::log_id::LogId;
use crate::storage::membership::Membership;
use crate::storage::membership::NodeId;
use crate::storage::vote::Vote;
use crate::type_config::alias::InstantOf;
use crate::TypeConfig;

/// Candidate: voting state.
#[derive(Clone, Debug)]
#[derive(PartialEq, Eq)]
pub(crate) struct Candidate<C>
where C: TypeConfig
{
    /// When the voting is started.
    starting_time: InstantOf<C>,

    /// The vote.
    pub(crate) vote: Vote,

    pub(crate) last_log_id: Option<LogId>,

    /// Which nodes have granted the vote at certain time point.
    progress: VecProgress<NodeId, bool, bool, Membership>,

    pub(crate) membership: Membership,
}

impl<C> fmt::Display for Candidate<C>
where C: TypeConfig
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{{}@{}, progress:{}}}",
            self.vote,
            self.starting_time.display(),
            self.progress
        )
    }
}

impl<C> Candidate<C>
where C: TypeConfig
{
    pub(crate) fn new(
        starting_time: InstantOf<C>,
        vote: Vote,
        last_log_id: Option<LogId>,
        membership: Membership,
    ) -> Self {
        let progress = VecProgress::new(membership.clone(), [], || false);

        let c = Self {
            starting_time,
            vote,
            last_log_id,
            progress,
            membership,
        };
        info!("new candidate: {}", c);
        c
    }

    pub(crate) fn membership(&self) -> &Membership {
        &self.membership
    }

    pub(crate) fn vote_ref(&self) -> &Vote {
        &self.vote
    }

    /// Grant the vote by a node.
    pub(crate) fn grant_by(&mut self, target: &NodeId) -> bool {
        let granted = *self
            .progress
            .update(target, true)
            .expect("target not in quorum set");

        info!(
            "id={} candidate vote {} is granted by {}",
            self.vote.voted_for(),
            self.vote,
            target
        );

        granted
    }
}
