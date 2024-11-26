use crate::errors::ForwardToLeader;
use crate::storage::log::log_id::LogId;

/// The result of a write request to SuRaft.
pub type WriteResult = Result<LogId, ForwardToLeader>;
