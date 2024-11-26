//! Defines [`LogStorage`] and [`RaftStateMachine`] trait.
//!
//! [`LogStorage`] is responsible for storing logs,
//! and [`RaftStateMachine`] is responsible for storing state machine and
//! snapshot.

use std::io;

use openraft_macros::add_async_trait;

use crate::OptionalSend;
use crate::OptionalSync;
use crate::TypeConfig;

/// API for log store.
///
/// `vote` API are also included because in raft, vote is part to the log:
/// `vote` is about **when**, while `log` is about **what**. A distributed
/// consensus is about **at what a time, happened what an event**.
///
/// ### To ensure correctness:
///
/// - Logs must be consecutive, i.e., there must **NOT** leave a **hole** in
///   logs.
/// - All write-IO must be serialized, i.e., the internal implementation must
///   **NOT** apply a latter write request before a former write request is
///   completed. This rule applies to both `vote` and `log` IO. E.g., Saving a
///   vote and appending a log entry must be serialized too.
#[add_async_trait]
pub trait LogStorage<C>: Clone + OptionalSend + OptionalSync + 'static
where C: TypeConfig
{
    /// Read the log entry at given log index
    async fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, io::Error>;

    async fn write(
        &mut self,
        path: &str,
        buf: &[u8],
        exclusive: bool,
    ) -> Result<bool, io::Error>;

    async fn list(
        &mut self,
        prefix: &str,
        start_after: &str,
    ) -> Result<Vec<String>, io::Error>;
}
