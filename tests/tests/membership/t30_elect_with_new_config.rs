use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;
use suraft::LogIdOptionExt;
use tokio::time::sleep;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Dynamic membership test.
///
/// What does this test do?
///
/// - bring a single-node cluster online.
/// - add a few new nodes and assert that they've joined the cluster properly.
/// - propose a new config change where the old master is not present, and
///   assert that it steps down.
/// - temporarily isolate the new master, and assert that a new master takes
///   over.
/// - restore the isolated node and assert that it becomes a follower.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn leader_election_after_changing_0_to_01234() -> Result<()> {
    // Setup test dependencies.
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            enable_elect: false,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());

    tracing::info!("--- initializing cluster");
    let mut log_index = router
        .new_cluster(btreeset! {s(0), s(1), s(2), s(3), s(4)}, btreeset! {})
        .await?;

    // Isolate old leader and assert that a new leader takes over.
    tracing::info!(log_index, "--- isolating leader node 0");
    router.set_network_error(s(0), true);

    // Wait for leader lease to expire
    sleep(Duration::from_millis(700)).await;

    // Let node-1 become leader.
    let node_1 = router.get_raft_handle(&s(1))?;
    node_1.trigger().elect().await?;
    log_index += 1; // leader initial blank log

    router
        .wait_for_metrics(
            &s(1),
            |x| x.current_leader == Some(s(1)),
            timeout(),
            "wait for new leader",
        )
        .await?;

    for node_id in [s(1), s(2), s(3), s(4)] {
        router
            .wait(&node_id, timeout())
            .applied_index(
                Some(log_index),
                "replicate and apply log to every node",
            )
            .await?;
    }

    let leader_id = s(1);

    tracing::info!(log_index, "--- restore node 0, log_index:{}", log_index);
    router.set_network_error(s(0), false);
    router
        .wait(&s(0), timeout())
        .metrics(
            |x| {
                x.current_leader == Some(leader_id.clone())
                    && x.last_applied.index() == Some(log_index)
            },
            "wait for restored node-0 to sync",
        )
        .await?;

    let current_leader =
        router.leader().expect("expected to find current leader");
    assert_eq!(
        leader_id, current_leader,
        "expected cluster leadership to stay the same"
    );

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1_000))
}
