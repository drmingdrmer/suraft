//! Error types exposed by this crate.

use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use std::io;

use anyerror::AnyError;
use tracing::error;

use crate::storage::log::log_id::LogId;
use crate::storage::membership::Membership;
use crate::storage::membership::NodeId;
use crate::storage::vote::Vote;

pub(crate) fn to_any_error<E: fmt::Display + 'static>(e: E) -> AnyError {
    error!("error: {}", e);
    AnyError::error(e)
}

/// Fatal is unrecoverable and shuts down raft at once.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum Fatal {
    #[error(transparent)]
    StorageError(#[from] AnyError),

    #[error("panicked")]
    Panicked,

    /// SuRaft stopped normally.
    #[error("Stopped normally")]
    Stopped,
}

impl From<io::Error> for Fatal {
    fn from(value: io::Error) -> Self {
        Fatal::StorageError(AnyError::new(&value))
    }
}

/// The set of errors which may take place when requesting to propose a config
/// change.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum ChangeMembershipError {
    #[error(transparent)]
    EmptyMembership(#[from] EmptyMembership),
}

/// The set of errors which may take place when initializing a pristine SuRaft
/// node.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, derive_more::TryInto)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum InitializeError {
    #[error(transparent)]
    NotAllowed(#[from] NotAllowed),

    #[error(transparent)]
    NotInMembers(#[from] NotInMembers),
}

/// Error that indicates a **temporary** network error and when it is returned,
/// Openraft will retry immediately.
///
/// Unlike [`Unreachable`], which indicates an error that should backoff before
/// retrying.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("NetworkError: {source}")]
pub struct NetworkError {
    #[from]
    source: AnyError,
}

impl NetworkError {
    pub fn new<E: Error + 'static>(e: &E) -> Self {
        Self {
            source: AnyError::new(e),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[derive(serde::Deserialize, serde::Serialize)]
#[error("has to forward request to: {leader_id:?}")]
pub struct ForwardToLeader {
    pub leader_id: Option<NodeId>,
}

impl ForwardToLeader {
    pub const fn empty() -> Self {
        Self { leader_id: None }
    }

    pub fn new(leader_id: NodeId) -> Self {
        Self {
            leader_id: Some(leader_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[derive(serde::Deserialize, serde::Serialize)]
#[error("not allowed to initialize due to current state: last_log_id: {last_log_id:?} vote: {vote}")]
pub struct NotAllowed {
    pub last_log_id: Option<LogId>,
    pub vote: Vote,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[derive(serde::Deserialize, serde::Serialize)]
#[error("node {node_id} has to be a member. membership:{membership:?}")]
pub struct NotInMembers {
    pub node_id: NodeId,
    pub membership: Membership,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[derive(serde::Deserialize, serde::Serialize)]
#[error("new membership can not be empty")]
pub struct EmptyMembership {}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("infallible")]
pub enum Infallible {}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum RejectVoteRequest {
    #[error("reject vote request by a greater vote: {0}")]
    ByVote(Vote),

    #[allow(dead_code)]
    #[error("reject vote request by a greater last-log-id: {0:?}")]
    ByLastLogId(Option<LogId>),
}
