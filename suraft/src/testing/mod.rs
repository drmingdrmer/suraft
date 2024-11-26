//! Testing utilities for OpenRaft.

pub mod runtime;

use std::fmt::Display;

use crate::storage::log::entry::Entry;
use crate::storage::log::log_id::LogId;
use crate::storage::membership::NodeId;
use crate::TypeConfig;

/// Builds a log id, for testing purposes.
pub fn log_id(term: u64, index: u64) -> LogId {
    LogId { term, index }
}

/// Create a blank log entry for test.
pub fn blank_ent<C: TypeConfig>(term: u64, index: u64) -> Entry<C> {
    Entry::<C>::new_blank(LogId::new(term, index))
}

#[allow(dead_code)]
pub(crate) fn nid(x: impl Display) -> NodeId {
    format!("{}", x)
}
