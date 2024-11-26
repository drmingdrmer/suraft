use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreemap;
use maplit::btreeset;
#[allow(unused_imports)]
use pretty_assertions::assert_eq;
#[allow(unused_imports)]
use pretty_assertions::assert_ne;
use suraft::testing::log_id;
use suraft::Config;
use suraft::LogId;
use suraft::ServerState;
use tokio::time::sleep;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Cluster leader_metrics test.
///
/// - brings 5 nodes online: one leader and 4 learner.
/// - add 4 learner as follower.
/// - asserts that the leader was able to successfully commit logs and that the
///   followers has successfully replicated the payload.
/// - remove one follower: node-4
/// - asserts node-4 becomes learner and the leader stops sending logs to it.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn leader_metrics() -> Result<()> {
    let c01234 = btreeset! {s(0), s(1), s(2), s(3), s(4)};
    let c0123 = btreeset![s(0), s(1), s(2), s(3)];

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
    let mut log_index =
        router.new_cluster(btreeset! {s(0)}, btreeset! {}).await?;

    router
        .wait_for_metrics(
            &s(0),
            |x| {
                if let Some(ref q) = x.replication {
                    q == &btreemap! {
                        s(0) => Some(log_id(1,1)),
                    }
                } else {
                    false
                }
            },
            timeout(),
            "no replication with 1 node cluster",
        )
        .await?;

    // Sync some new nodes.
    router.new_raft_node(s(1)).await;
    router.new_raft_node(s(2)).await;
    router.new_raft_node(s(3)).await;
    router.new_raft_node(s(4)).await;

    tracing::info!(log_index, "--- adding 4 new nodes to cluster");
    {
        router.add_learner(s(0), s(1)).await?;
        router.add_learner(s(0), s(2)).await?;
        router.add_learner(s(0), s(3)).await?;
        router.add_learner(s(0), s(4)).await?;
    }
    log_index += 4; // 4 add_learner log
    router
        .wait_for_log(
            &c01234,
            Some(log_index),
            timeout(),
            "add learner 1,2,3,4",
        )
        .await?;

    tracing::info!(log_index, "--- changing cluster config to 01234");

    let node = router.get_raft_handle(&s(0))?;
    node.change_membership(c01234.clone(), false).await?;
    log_index += 2; // 2 member-change logs

    router
        .wait_for_log(
            &c01234,
            Some(log_index),
            timeout(),
            "change members to 0,1,2,3,4",
        )
        .await?;

    let ww = Some(LogId::new(1, log_index));
    let want_repl = btreemap! { s(0)=>ww.clone(), s(1)=>ww.clone(), s(2)=>ww.clone(), s(3)=>ww.clone(), s(4)=>ww.clone(), };
    router
        .wait_for_metrics(
            &s(0),
            |x| {
                if let Some(ref q) = x.replication {
                    q == &want_repl
                } else {
                    false
                }
            },
            timeout(),
            "replication metrics to 4 nodes",
        )
        .await?;

    // Send some requests
    router.client_request_many(s(0), "client", 10).await?;
    log_index += 10;

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
                "other nodes should commit the membership change log",
            )
            .await?;
    }

    tracing::info!(
        log_index,
        "--- replication metrics should reflect the replication state"
    );
    {
        let ww = Some(LogId::new(1, log_index));
        let want_repl = btreemap! { s(0)=>ww.clone(), s(1)=>ww.clone(), s(2)=>ww.clone(), s(3)=>ww.clone()};
        router
            .wait_for_metrics(
                &s(0),
                |x| {
                    if let Some(ref q) = x.replication {
                        q == &want_repl
                    } else {
                        false
                    }
                },
                timeout(),
                "replication metrics to 3 nodes",
            )
            .await?;
    }

    let n0 = router.get_raft_handle(&s(0))?;
    let n1 = router.get_raft_handle(&s(1))?;

    tracing::info!(
        log_index,
        "--- let node-1 to elect to take leadership from node-0"
    );
    {
        // Let the leader lease expire
        sleep(Duration::from_millis(700)).await;

        n1.trigger().elect().await?;
        n1.wait(timeout())
            .state(ServerState::Leader, "node-1 becomes leader")
            .await?;
        n1.wait(timeout())
            .metrics(|x| x.replication.is_some(), "node-1 starts replication")
            .await?;

        n0.wait(timeout())
            .metrics(|x| x.replication.is_none(), "node-0 stopped replication")
            .await?;
        n0.wait(timeout())
            .metrics(
                |x| x.current_leader == Some(s(1)),
                "node-0 receives leader-1 message",
            )
            .await?;
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1000))
}
