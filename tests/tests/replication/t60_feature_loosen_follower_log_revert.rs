use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;

use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// With "--features loosen-follower-log-revert", the leader allows follower to
/// revert its log to an earlier state.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn feature_loosen_follower_log_revert() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_tick: false,
            enable_heartbeat: false,
            // Make sure the replication is done in more than one steps
            max_payload_entries: 1,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    tracing::info!("--- initializing cluster");
    let mut log_index = router
        .new_cluster(btreeset! {s(0),s(1),s(2)}, btreeset! {s(3)})
        .await?;

    tracing::info!(log_index, "--- write 10 logs");
    {
        log_index += router.client_request_many(s(0), "0", 10).await?;
        for i in [0, 1, 2, 3] {
            router
                .wait(&i, timeout())
                .applied_index(Some(log_index), format!("{} writes", 10))
                .await?;
        }
    }

    tracing::info!(log_index, "--- erase node 3 and restart");
    {
        let (_raft, _ls, _sm) = router.remove_node(s(3)).unwrap();
        let (log, sm) = suraft_memstore::new_mem_store();

        router.new_raft_node_with_sto(s(3), log, sm).await;
        router.add_learner(s(0), s(3)).await?;
        log_index += 1; // add learner
    }

    tracing::info!(
        log_index,
        "--- write another 10 logs, leader should not panic"
    );
    {
        log_index += router.client_request_many(s(0), "0", 10).await?;
        for i in [0, 1, 2, 3] {
            router
                .wait(&i, timeout())
                .applied_index(Some(log_index), format!("{} writes", 10))
                .await?;
        }
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1_000))
}
