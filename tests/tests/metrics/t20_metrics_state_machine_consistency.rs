use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;
use suraft::ServerState;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Cluster metrics_state_machine_consistency test.
///
/// What does this test do?
///
/// - brings 2 nodes online: one leader and one learner.
/// - write one log to the leader.
/// - asserts that when metrics.last_applied is upto date, the state machine
///   should be upto date too.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn metrics_state_machine_consistency() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    let mut log_index = 0;

    router.new_raft_node(s(0)).await;
    router.new_raft_node(s(1)).await;

    tracing::info!(log_index, "--- initializing single node cluster");
    {
        let n0 = router.get_raft_handle(&s(0))?;
        n0.initialize(btreeset! {s(0)}).await?;
        log_index += 1;

        router
            .wait(&s(0), timeout())
            .state(ServerState::Leader, "n0 -> leader")
            .await?;
    }

    tracing::info!(log_index, "--- add one learner");
    router.add_learner(s(0), s(1)).await?;
    log_index += 1;

    tracing::info!(log_index, "--- write one log");
    router.client_request(s(0), "foo", 1).await?;

    // Wait for metrics to be up to date.
    // Once last_applied updated, the key should be visible in state machine.
    tracing::info!(log_index, "--- wait for log to sync");
    log_index += 1;
    for node_id in [s(0), s(1)] {
        router
            .wait_for_log(
                &btreeset![node_id.clone()],
                Some(log_index),
                None,
                "write one log",
            )
            .await?;
        let (_sto, sm) = router.get_storage_handle(&node_id)?;
        assert!(sm.get_state_machine().await.client_status.contains_key("foo"));
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1000))
}
