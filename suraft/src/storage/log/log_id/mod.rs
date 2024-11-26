//! This mod defines the identity of a raft log and provides supporting
//! utilities to work with log id related types.

mod log_id_option_ext;
mod log_index_option_ext;

use std::fmt::Display;
use std::fmt::Formatter;

pub use log_id_option_ext::LogIdOptionExt;
pub use log_index_option_ext::LogIndexOptionExt;

/// The identity of a raft log.
///
/// The log id serves as unique identifier for a log entry across the system. It
/// is composed of two parts: a leader id, which refers to the leader that
/// proposed this log, and an integer index.
#[derive(Debug, Default, Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub struct LogId {
    pub term: u64,
    /// The index of a log in the storage.
    ///
    /// Log index is a consecutive integer.
    pub index: u64,
}

impl Display for LogId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.term, self.index)
    }
}

impl LogId {
    /// Creates a log id proposed by a committed leader with `leader_id` at the
    /// given index.
    pub fn new(term: u64, index: u64) -> Self {
        LogId { term, index }
    }

    pub fn term(&self) -> u64 {
        self.term
    }
}
