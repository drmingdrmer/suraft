use core::time::Duration;

use futures::FutureExt;

use crate::async_runtime::watch::WatchReceiver;
use crate::metrics::Condition;
use crate::metrics::Metric;
use crate::metrics::Metrics;
use crate::metrics::ServerState;
use crate::storage::membership::NodeId;
use crate::storage::vote::Vote;
use crate::type_config::alias::WatchReceiverOf;
use crate::type_config::TypeConfigExt;
use crate::OptionalSend;
use crate::TypeConfig;

// Error variants related to metrics.
#[derive(Debug, thiserror::Error)]
pub enum WaitError {
    #[error("timeout after {0:?} when {1}")]
    Timeout(Duration, String),

    #[error("suraft is shutting down")]
    ShuttingDown,
}

/// Wait is a wrapper of RaftMetrics channel that impls several utils to wait
/// for metrics to satisfy some condition.
pub struct Wait<C: TypeConfig> {
    pub timeout: Duration,
    pub rx: WatchReceiverOf<C, Metrics<C>>,
}

impl<C> Wait<C>
where C: TypeConfig
{
    /// Wait for metrics to satisfy some condition or timeout.
    #[tracing::instrument(level = "trace", skip(self, func), fields(msg=%msg.to_string()))]
    pub async fn metrics<T>(
        &self,
        func: T,
        msg: impl ToString,
    ) -> Result<Metrics<C>, WaitError>
    where
        T: Fn(&Metrics<C>) -> bool + OptionalSend,
    {
        let timeout_at = C::now() + self.timeout;

        let mut rx = self.rx.clone();
        loop {
            let latest = rx.borrow_watched().clone();

            tracing::debug!(
                "id={} wait {:} latest: {}",
                latest.id,
                msg.to_string(),
                latest
            );

            if func(&latest) {
                tracing::debug!(
                    "id={} done wait {:} latest: {}",
                    latest.id,
                    msg.to_string(),
                    latest
                );
                return Ok(latest);
            }

            let now = C::now();
            if now >= timeout_at {
                return Err(WaitError::Timeout(
                    self.timeout,
                    format!("{} latest: {}", msg.to_string(), latest),
                ));
            }

            let sleep_time = timeout_at - now;
            tracing::debug!(?sleep_time, "wait timeout");
            let delay = C::sleep(sleep_time);

            futures::select_biased! {
                _ = delay.fuse() => {
                    tracing::debug!( "id={} timeout wait {:} latest: {}", latest.id, msg.to_string(), latest );
                    return Err(WaitError::Timeout(self.timeout, format!("{} latest: {}", msg.to_string(), latest)));
                }
                changed = rx.changed().fuse() => {
                    match changed {
                        Ok(_) => {
                            // metrics changed, continue the waiting loop
                        },
                        Err(err) => {
                            tracing::debug!(
                                "id={} error: {:?}; wait {:} latest: {:?}",
                                latest.id,
                                err,
                                msg.to_string(),
                                latest
                            );

                            return Err(WaitError::ShuttingDown);
                        }
                    }
                }
            };
        }
    }

    /// Wait for `vote` to become `want` or timeout.
    #[tracing::instrument(level = "trace", skip(self), fields(msg=msg.to_string().as_str()))]
    pub async fn vote(
        &self,
        want: Vote,
        msg: impl ToString,
    ) -> Result<Metrics<C>, WaitError> {
        self.eq(Metric::Vote(Some(want)), msg).await
    }

    /// Wait for `current_leader` to become `Some(leader_id)` until timeout.
    #[tracing::instrument(level = "trace", skip(self), fields(msg=msg.to_string().as_str()))]
    pub async fn current_leader(
        &self,
        leader_id: NodeId,
        msg: impl ToString,
    ) -> Result<Metrics<C>, WaitError> {
        self.metrics(
            |m| m.current_leader.as_ref() == Some(&leader_id),
            &format!("{} .current_leader == {}", msg.to_string(), leader_id),
        )
        .await
    }

    /// Wait until applied exactly `want_log`(inclusive) logs or timeout.
    #[deprecated(
        since = "0.9.0",
        note = "use `log_index()` and `applied_index()` instead"
    )]
    #[tracing::instrument(level = "trace", skip(self), fields(msg=msg.to_string().as_str()))]
    pub async fn log(
        &self,
        want_log_index: Option<u64>,
        msg: impl ToString,
    ) -> Result<Metrics<C>, WaitError> {
        self.eq(Metric::LastLogIndex(want_log_index), msg.to_string()).await
    }

    /// Wait until applied at least `want_log`(inclusive) logs or timeout.
    #[deprecated(
        since = "0.9.0",
        note = "use `log_index_at_least()` and `applied_index_at_least()` instead"
    )]
    #[tracing::instrument(level = "trace", skip(self), fields(msg=msg.to_string().as_str()))]
    pub async fn log_at_least(
        &self,
        want_log: Option<u64>,
        msg: impl ToString,
    ) -> Result<Metrics<C>, WaitError> {
        self.ge(Metric::LastLogIndex(want_log), msg.to_string()).await
    }

    /// Block until the last log index becomes exactly `index`(inclusive) or
    /// timeout.
    #[tracing::instrument(level = "trace", skip(self), fields(msg=msg.to_string().as_str()))]
    pub async fn log_index(
        &self,
        index: Option<u64>,
        msg: impl ToString,
    ) -> Result<Metrics<C>, WaitError> {
        self.eq(Metric::LastLogIndex(index), msg).await
    }

    /// Block until the last log index becomes at least `index`(inclusive) or
    /// timeout.
    #[tracing::instrument(level = "trace", skip(self), fields(msg=msg.to_string().as_str()))]
    pub async fn log_index_at_least(
        &self,
        index: Option<u64>,
        msg: impl ToString,
    ) -> Result<Metrics<C>, WaitError> {
        self.ge(Metric::LastLogIndex(index), msg).await
    }

    /// Wait for `state` to become `want_state` or timeout.
    #[tracing::instrument(level = "trace", skip(self), fields(msg=msg.to_string().as_str()))]
    pub async fn state(
        &self,
        want_state: ServerState,
        msg: impl ToString,
    ) -> Result<Metrics<C>, WaitError> {
        self.metrics(
            |m| m.server_state == want_state,
            &format!("{} .state == {:?}", msg.to_string(), want_state),
        )
        .await
    }

    /// Block until a metric becomes greater than or equal the specified value
    /// or timeout.
    ///
    /// For example, to await until the term becomes 2 or greater:
    /// ```ignore
    /// my_raft.wait(None).ge(Metric::Term(2), "become term 2").await?;
    /// ```
    pub async fn ge(
        &self,
        metric: Metric,
        msg: impl ToString,
    ) -> Result<Metrics<C>, WaitError> {
        self.until(Condition::ge(metric), msg).await
    }

    /// Block until a metric becomes equal to the specified value or timeout.
    ///
    /// For example, to await until the term becomes exact 2:
    /// ```ignore
    /// my_raft.wait(None).eq(Metric::Term(2), "become term 2").await?;
    /// ```
    pub async fn eq(
        &self,
        metric: Metric,
        msg: impl ToString,
    ) -> Result<Metrics<C>, WaitError> {
        self.until(Condition::eq(metric), msg).await
    }

    /// Block until a metric satisfies the specified condition or timeout.
    #[tracing::instrument(level = "trace", skip_all, fields(cond=cond.to_string(), msg=msg.to_string().as_str()))]
    pub(crate) async fn until(
        &self,
        cond: Condition,
        msg: impl ToString,
    ) -> Result<Metrics<C>, WaitError> {
        self.metrics(
            |metrics| match &cond {
                Condition::GE(expect) => metrics >= expect,
                Condition::EQ(expect) => metrics == expect,
            },
            &format!("{} .{}", msg.to_string(), cond),
        )
        .await
    }
}
