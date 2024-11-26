use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn t99_issue_584_replication_state_reverted() -> Result<()> {
    // - Add a learner and replicate all logs to it.
    // - Add the learner as a voter. When membership changes, suraft internally
    //   restarts all replication.
    //
    // This case asserts it does not break the internal
    // monotonic-replication-progress guarantee.

    let config = Arc::new(
        Config {
            max_in_snapshot_log_to_keep: 2000, // prevent snapshot
            enable_tick: false,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());

    let mut log_index =
        router.new_cluster(btreeset! {s(0)}, btreeset! {s(1)}).await?;

    let n = 500u64;
    tracing::info!(log_index, "--- write up to {} logs", n);
    {
        router
            .client_request_many(s(0), "foo", (n - log_index) as usize)
            .await?;
        log_index = n;

        router
            .wait(&s(1), timeout())
            .applied_index(Some(log_index), "replicate all logs to learner")
            .await?;
    }

    tracing::info!(
        log_index,
        "--- change-membership: make learner node-1 a voter. This should not panic"
    );
    {
        let leader = router.get_raft_handle(&s(0))?;
        leader.change_membership([s(0), s(1)], false).await?;
        log_index += 2; // 2 change_membership log

        let _ = log_index;
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1_000))
}
