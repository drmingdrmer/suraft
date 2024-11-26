use std::sync::Arc;
use std::time::Duration;

use maplit::btreeset;
use suraft::testing::log_id;
use suraft::Config;
use suraft::SnapshotPolicy;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Call `SuRaft::trigger_purged()` to purge logs.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn trigger_purge_log() -> anyhow::Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            // Disable building snapshot by policy.
            snapshot_policy: SnapshotPolicy::Never,
            // Disable auto purge by policy.
            max_in_snapshot_log_to_keep: u64::MAX,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    tracing::info!("--- initializing cluster");
    let mut log_index =
        router.new_cluster(btreeset! {s(0),s(1),s(2)}, btreeset! {}).await?;

    tracing::info!(log_index, "--- write some logs");
    {
        log_index += router.client_request_many(s(0), "0", 10).await?;

        for id in [s(0), s(1), s(2)] {
            router
                .wait(&id, timeout())
                .applied_index(
                    Some(log_index),
                    format_args!("node-{} write logs", id),
                )
                .await?;
        }
    }

    tracing::info!(log_index, "--- trigger snapshot for node-0");
    {
        let n0 = router.get_raft_handle(&s(0))?;
        n0.trigger().snapshot().await?;

        router
            .wait(&s(0), timeout())
            .snapshot(log_id(1, log_index), "node-1 snapshot")
            .await?;
    }

    let snapshot_index = log_index;

    tracing::info!(log_index, "--- write another bunch of logs");
    {
        log_index += router.client_request_many(s(0), "0", 10).await?;

        for id in [s(0), s(1), s(2)] {
            router
                .wait(&id, timeout())
                .applied_index(
                    Some(log_index),
                    format_args!("node-{} write logs", id),
                )
                .await?;
        }
    }

    tracing::info!(log_index, "--- purge log for node 0");
    {
        let n0 = router.get_raft_handle(&s(0))?;
        n0.trigger().purge_log(snapshot_index).await?;

        router
            .wait(&s(0), timeout())
            .purged(
                Some(log_id(1, snapshot_index)),
                format_args!("node-0 purged upto {}", snapshot_index),
            )
            .await?;

        n0.trigger().purge_log(log_index).await?;
        let res = router
            .wait(&s(0), timeout())
            .purged(
                Some(log_id(1, log_index)),
                format_args!("node-0 wont purged upto {}", log_index),
            )
            .await;

        assert!(res.is_err(), "can not purge logs not in snapshot");
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1_000))
}
