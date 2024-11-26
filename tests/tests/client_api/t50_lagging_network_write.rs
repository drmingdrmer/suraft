use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;
use suraft::ServerState;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Lagging network test.
///
/// What does this test do?
///
/// - Setup a network with <=50 ms random delay of messages.
/// - bring a single-node cluster online.
/// - add two Learner and then try to commit one log.
/// - change config to a 3 members cluster and commit another log.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn lagging_network_write() -> Result<()> {
    let config = Arc::new(
        Config {
            heartbeat_interval: 100,
            election_timeout_min: 300,
            election_timeout_max: 600,
            enable_tick: false,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::builder(config).send_delay(50).build();

    tracing::info!("--- initializing cluster");
    let mut log_index =
        router.new_cluster(btreeset! {s(0)}, btreeset! {s(1),s(2)}).await?;

    router.client_request_many(s(0), "client", 1).await?;
    log_index += 1;
    router
        .wait_for_log(
            &btreeset! {s(0), s(1), s(2)},
            Some(log_index),
            timeout(),
            "write one log",
        )
        .await?;

    let node = router.get_raft_handle(&s(0))?;
    node.change_membership([s(0), s(1), s(2)], false).await?;
    log_index += 2;
    router
        .wait_for_state(&btreeset! {s(0)}, ServerState::Leader, None, "changed")
        .await?;
    router
        .wait_for_state(
            &btreeset! {s(1), s(2)},
            ServerState::Follower,
            None,
            "changed",
        )
        .await?;
    router
        .wait_for_log(
            &btreeset! {s(0), s(1), s(2)},
            Some(log_index),
            timeout(),
            "3 candidates",
        )
        .await?;

    router.client_request_many(s(0), "client", 1).await?;
    log_index += 1;
    router
        .wait_for_log(
            &btreeset! {s(0), s(1), s(2)},
            Some(log_index),
            timeout(),
            "write 2nd log",
        )
        .await?;

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(2000))
}
