use std::borrow::Borrow;
use std::fmt;
use std::time::Duration;

use crate::base::display_ext::DisplayOptionExt;
use crate::storage::log::log_id::LogId;
use crate::storage::vote::Vote;

/// An RPC sent by candidates to gather votes (§5.2).
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct RequestVote {
    pub vote: Vote,
    pub last_log_id: Option<LogId>,
    pub lease_ms: u64,
}

impl fmt::Display for RequestVote {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{vote:{}, last_log:{}, lease:{:?}}}",
            self.vote,
            self.last_log_id.display(),
            Duration::from_millis(self.lease_ms)
        )
    }
}

impl RequestVote {
    pub fn new(
        vote: Vote,
        last_log_id: Option<LogId>,
        lease: Duration,
    ) -> Self {
        Self {
            vote,
            last_log_id,
            lease_ms: lease.as_millis() as u64,
        }
    }
}

/// The response to a [`RequestVote`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct VoteReply {
    /// vote after a node handling vote-request.
    /// Thus, `resp.vote >= req.vote` always holds.
    ///
    /// `vote` that equals the `candidate.vote` does not mean the vote is
    /// granted. The `vote` may be updated when a previous Leader sees a
    /// higher vote.
    pub vote: Option<Vote>,

    /// It is true if a node accepted and saved the VoteRequest.
    pub vote_granted: bool,

    /// The last log id stored on the remote voter.
    pub last_log_id: Option<LogId>,
}

impl VoteReply {
    pub fn new(
        vote: Option<Vote>,
        last_log_id: Option<LogId>,
        granted: bool,
    ) -> Self {
        Self {
            vote,
            vote_granted: granted,
            last_log_id: last_log_id.map(|x| x.borrow().clone()),
        }
    }

    /// Returns `true` if the response indicates that the target node has
    /// granted a vote to the candidate.
    pub fn is_granted_to(&self, candidate_vote: &Vote) -> bool {
        self.vote.as_ref() == Some(candidate_vote)
    }
}

impl fmt::Display for VoteReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{{}, last_log:{:?}}}",
            self.vote.display(),
            self.last_log_id.as_ref().map(|x| x.to_string())
        )
    }
}
