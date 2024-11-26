//! RuntimeConfigHandle is an interface to change SuRaft runtime config.

use std::sync::atomic::Ordering;

use crate::suraft::SuRaftInner;
use crate::TypeConfig;

/// RuntimeConfigHandle is an interface to update runtime config.
///
/// These config are mainly designed for testing purpose and special use cases.
/// Usually you don't need to change runtime config.
pub struct RuntimeConfigHandle<'r, C>
where C: TypeConfig
{
    suraft_inner: &'r SuRaftInner<C>,
}

impl<'r, C> RuntimeConfigHandle<'r, C>
where C: TypeConfig
{
    pub(in crate::suraft) fn new(raft_inner: &'r SuRaftInner<C>) -> Self {
        Self {
            suraft_inner: raft_inner,
        }
    }

    /// Enable or disable raft internal ticker.
    ///
    /// Disabling tick will disable election and heartbeat.
    pub fn tick(&self, enabled: bool) {
        self.suraft_inner.tick_handle.enable(enabled);
    }

    /// Enable or disable heartbeat message when a leader has no more log to
    /// replicate.
    ///
    /// Note that the follower's leader-lease will not be renewed if it does
    /// receive message from the leader, and it will start election(if
    /// `Self::elect()` is enabled) when the lease timed out.
    pub fn heartbeat(&self, enabled: bool) {
        self.suraft_inner
            .runtime_config
            .enable_heartbeat
            .store(enabled, Ordering::Relaxed);
    }

    /// Enable or disable election for a follower when its leader lease timed
    /// out.
    pub fn elect(&self, enabled: bool) {
        self.suraft_inner
            .runtime_config
            .enable_elect
            .store(enabled, Ordering::Relaxed);
    }
}
