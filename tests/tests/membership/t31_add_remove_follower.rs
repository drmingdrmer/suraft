use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;
use suraft::ServerState;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// When a node is removed from cluster, replication to it should be stopped.
///
/// - brings 5 nodes online: one leader and 4 follower.
/// - asserts that the leader was able to successfully commit logs and that the
///   followers has successfully replicated the payload.
/// - remove one follower: node-4
/// - asserts node-4 becomes learner and the leader stops sending logs to it.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn add_remove_voter() -> Result<()> {
    let c01234 = btreeset! {s(0), s(1), s(2), s(3), s(4)};
    let c0123 = btreeset! {s(0), s(1), s(2), s(3)};

    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());

    let mut log_index =
        router.new_cluster(c01234.clone(), btreeset! {}).await?;

    tracing::info!(log_index, "--- write 100 logs");
    {
        router.client_request_many(s(0), "client", 100).await?;
        log_index += 100;

        router
            .wait_for_log(&c01234, Some(log_index), timeout(), "write 100 logs")
            .await?;
    }

    tracing::info!(log_index, "--- remove n{}", 4);
    {
        let node = router.get_raft_handle(&s(0))?;
        node.change_membership(c0123.clone(), false).await?;
        log_index += 2; // two member-change logs

        router
            .wait_for_log(
                &c0123,
                Some(log_index),
                timeout(),
                "removed node-4 from membership",
            )
            .await?;
    }

    tracing::info!(log_index, "--- write another 100 logs");
    {
        router.client_request_many(s(0), "client", 100).await?;
        log_index += 100;
    }

    router
        .wait_for_log(
            &c0123,
            Some(log_index),
            timeout(),
            "4 nodes recv logs 100~200",
        )
        .await?;

    tracing::info!(log_index, "--- log will not be sync to removed node");
    {
        let x = router.latest_metrics();
        assert!(x[4].committed < Some(log_index - 50));
    }

    router
        .wait(&s(4), timeout())
        .metrics(
            |x| x.server_state == ServerState::Learner || x.server_state == ServerState::Candidate,
            "node-4 is left a learner or follower, depending on if it received the uniform config",
        )
        .await?;

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(2000))
}
