use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;
use suraft::ServerState;
use suraft::SuRaft;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Cluster learner_restart test.
///
/// What does this test do?
///
/// - brings 2 nodes online: one leader and one learner.
/// - write one log to the leader.
/// - asserts that the leader was able to successfully commit its initial
///   payload and that the learner has successfully replicated the payload.
/// - shutdown all and restart the learner node.
/// - asserts the learner stays in non-voter state.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn learner_restart() -> Result<()> {
    // Setup test dependencies.
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    tracing::info!("--- initializing");
    let mut log_index =
        router.new_cluster(btreeset! {s(0)}, btreeset! {s(1)}).await?;

    router.client_request(s(0), "foo", 1).await?;
    log_index += 1;

    router
        .wait_for_log(
            &btreeset! {s(0), s(1)},
            Some(log_index),
            None,
            "write one log",
        )
        .await?;

    let (node0, _sto0, _sm0) = router.remove_node(s(0)).unwrap();
    node0.shutdown().await?;

    let (node1, sto1, sm1) = router.remove_node(s(1)).unwrap();
    node1.shutdown().await?;

    // restart node-1, assert the state as expected.
    let restarted =
        SuRaft::new(s(1), config.clone(), router.clone(), sto1, sm1).await?;
    restarted
        .wait(timeout())
        .applied_index(Some(log_index), "log after restart")
        .await?;
    restarted
        .wait(timeout())
        .state(ServerState::Learner, "server state after restart")
        .await?;

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(2000))
}
