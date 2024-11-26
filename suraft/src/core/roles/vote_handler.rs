use std::ops::Deref;
use std::time::Duration;

use tracing::debug;

use crate::base::display_ext::display_instant::DisplayInstantExt;
use crate::base::display_ext::DisplayOptionExt;
use crate::base::leased::Leased;
use crate::core::CandidateState;
use crate::core::LeaderState;
use crate::errors::RejectVoteRequest;
use crate::storage::membership::NodeId;
use crate::storage::vote::Vote;
use crate::type_config::alias::InstantOf;
use crate::type_config::TypeConfigExt;
use crate::TypeConfig;

/// Handle raft vote related operations
///
/// A `vote` defines the state of a suraft node.
/// See [`RaftState::calc_server_state`] .
pub(crate) struct VoteHandler<'st, C>
where C: TypeConfig
{
    pub(crate) id: NodeId,
    pub(crate) vote: &'st mut Leased<Option<Vote>, InstantOf<C>>,
    pub(crate) leader: &'st mut LeaderState<C>,
    pub(crate) candidate: &'st mut CandidateState<C>,
}

impl<'st, C> VoteHandler<'st, C>
where C: TypeConfig
{
    /// Check and update the local vote and related state for every message
    /// received.
    ///
    /// This is used by all incoming event, such as the three RPC
    /// append-entries, vote, install-snapshot to check the `vote` field.
    ///
    /// It grants the input vote and persists it if `input_vote >= my_vote`.
    ///
    /// Note: This method does not check last-log-id. handle-vote-request has to
    /// deal with last-log-id itself.
    #[tracing::instrument(level = "debug", skip_all)]
    pub(crate) fn update_vote(
        &mut self,
        vote: &Vote,
        lease: Duration,
    ) -> Result<(), RejectVoteRequest> {
        let my_vote = self.vote.deref().as_ref();
        // Partial ord compare:
        // Vote does not have to be total ord.
        // `!(a >= b)` does not imply `a < b`.
        if Some(vote) >= my_vote {
            // Ok
        } else {
            tracing::info!(
                "vote {} is rejected by local vote: {}",
                vote,
                my_vote.display()
            );
            return Err(RejectVoteRequest::ByVote(
                self.vote_ref().unwrap().clone(),
            ));
        }

        debug!(
            "id={} update local vote from {:?} to {}",
            self.id, self.vote, vote
        );

        // Grant the vote

        if Some(vote) > self.vote_ref() {
            tracing::info!(
                "vote is changing from {} to {}",
                self.vote_ref().display(),
                vote
            );

            self.vote.update(C::now(), lease, Some(vote.clone()));
            self.update_server_state();
        } else {
            let now = C::now();
            debug!(
                "id={} equal vote({}) when updating, update lease: now:{} lease:{:?}",
                self.id,
                self.vote_ref().display(),
                now.display(),
                lease,
            );
            self.vote.touch(now, lease);
        }

        Ok(())
    }

    fn vote_ref(&self) -> Option<&Vote> {
        self.vote.deref().as_ref()
    }

    /// Update to Leader or following state depending on `Engine.state.vote`.
    pub(crate) fn update_server_state(&mut self) {
        let vote = self.vote_ref().unwrap();
        if vote.voted_for() == &self.id {
            //
        } else {
            if self.candidate.is_some() {
                tracing::info!("{} is no longer a candidate", self.id);
            }
            if self.leader.is_some() {
                tracing::info!("{} is no longer a leader", self.id);
            }
            *self.candidate = None;
            *self.leader = None;
        }
    }
}
