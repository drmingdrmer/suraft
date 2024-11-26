use std::sync::Arc;
use std::time::Duration;

use maplit::btreeset;
use suraft::Config;
use suraft::LogId;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Manually trigger a snapshot with `SuRaft::trigger_snapshot()` on Leader and
/// Follower.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn trigger_snapshot() -> anyhow::Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    tracing::info!("--- initializing cluster");
    let mut log_index =
        router.new_cluster(btreeset! {s(0),s(1)}, btreeset! {}).await?;

    tracing::info!(log_index, "--- trigger snapshot for node-1");
    {
        let n1 = router.get_raft_handle(&s(1))?;
        n1.trigger().snapshot().await?;

        router
            .wait(&s(1), timeout())
            .snapshot(LogId::new(1, log_index), "node-1 snapshot")
            .await?;
    }

    tracing::info!(log_index, "--- send some logs");
    {
        router.client_request_many(s(0), "0", 10).await?;
        log_index += 10;

        router
            .wait(&s(0), timeout())
            .applied_index(Some(log_index), "node-0 write logs")
            .await?;
        router
            .wait(&s(1), timeout())
            .applied_index(Some(log_index), "node-1 write logs")
            .await?;
    }

    tracing::info!(log_index, "--- trigger snapshot for node-0");
    {
        let n0 = router.get_raft_handle(&s(0))?;
        n0.trigger().snapshot().await?;

        router
            .wait(&s(0), timeout())
            .snapshot(LogId::new(1, log_index), "node-0 snapshot")
            .await?;
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1_000))
}
