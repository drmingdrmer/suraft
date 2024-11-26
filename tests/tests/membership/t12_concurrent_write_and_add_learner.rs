use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;
use suraft::LogIdOptionExt;
use suraft::NodeId;
use suraft::ServerState;
use suraft::Vote;
use tracing::Instrument;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Cluster concurrent_write_and_add_learner test.
///
/// Internally when replication goes to LaggingState(a non-leader lacks a lot
/// logs), the ReplicationCore purges `outbound_buffer` and `replication_buffer`
/// and then sends all **committed** logs found in storage.
///
/// Thus if there are uncommitted logs in `replication_buffer`, these log will
/// never have chance to be replicated, even when replication goes back to
/// LineRateState. Since LineRateState only replicates logs from
/// `ReplicationCore.outbound_buffer` and `ReplicationCore.replication_buffer`.
///
/// This test ensures that when replication goes to LineRateState, it tries to
/// re-send all logs found in storage(including those that are removed from the
/// two buffers.
///
/// NOTE: this test needs a multi nodes cluster because a single-node cluster
/// will commit a log at once.
///
///
/// What does this test do?
///
/// - brings a 3 candidates cluster online.
/// - add another learner and at the same time write a log.
/// - asserts that all of the leader, followers and the learner receives all
///   logs.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn concurrent_write_and_add_learner() -> Result<()> {
    let candidates = btreeset! {s(0), s(1), s(2)};

    // Setup test dependencies.
    let config = Arc::new(
        Config {
            enable_tick: false,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());

    router.new_raft_node(s(0)).await;

    let mut log_index;

    tracing::info!("--- initializing cluster of 1 node");
    {
        router.initialize(s(0)).await?;
        log_index = 1;

        wait_log(&router, &btreeset! {s(0)}, log_index).await?;
    }

    tracing::info!(log_index, "--- adding two candidate nodes");
    {
        // Sync some new nodes.
        router.new_raft_node(s(1)).await;
        router.new_raft_node(s(2)).await;
        router.add_learner(s(0), s(1)).await?;
        router.add_learner(s(0), s(2)).await?;
        log_index += 2; // two add_learner logs

        tracing::info!(log_index, "--- changing cluster config");

        let node = router.get_raft_handle(&s(0))?;
        node.change_membership(candidates.clone(), false).await?;
        log_index += 2; // Tow member change logs

        wait_log(&router, &candidates, log_index).await?;
        for id in [s(0), s(1), s(2)] {
            router
                .wait(&id, timeout())
                .vote(Vote::new_committed(1, s(0)), "after changing membership")
                .await?;
        }
    }

    let leader = router.leader().unwrap();

    tracing::info!(log_index, "--- write one log");
    {
        router.client_request_many(leader.clone(), "client", 1).await?;
        log_index += 1;

        wait_log(&router, &candidates, log_index).await?;
    }

    // Concurrently add Learner and write another log.
    tracing::info!(
        log_index,
        "--- concurrently add learner and write another log"
    );
    {
        router.new_raft_node(s(3)).await;
        let r = router.clone();

        let handle = {
            let leader = leader.clone();
            tokio::spawn(
                async move {
                    r.add_learner(leader.clone(), s(3)).await.unwrap();
                    Ok::<(), anyhow::Error>(())
                }
                .instrument(tracing::debug_span!("spawn-add-learner")),
            )
        };
        log_index += 1; // one add_learner log
        router.client_request_many(leader, "client", 1).await?;
        log_index += 1;

        let _ = handle.await?;
    };

    wait_log(&router, &candidates, log_index).await?;
    router
        .wait_for_metrics(
            &s(3),
            |x| x.server_state == ServerState::Learner,
            timeout(),
            &format!("n{}.state -> {:?}", 3, ServerState::Learner),
        )
        .await?;

    // THe learner should receive the last written log
    router
        .wait_for_metrics(
            &s(3),
            |x| x.committed == Some(log_index),
            timeout(),
            &format!("n{}.last_log_index -> {}", 3, log_index),
        )
        .await?;

    Ok(())
}

async fn wait_log(
    router: &ChannelNetwork,
    node_ids: &BTreeSet<NodeId>,
    want_log: u64,
) -> anyhow::Result<()> {
    for i in node_ids.iter() {
        router
            .wait_for_metrics(
                i,
                |x| x.committed == Some(want_log),
                timeout(),
                &format!("n{}.last_log_index -> {}", i, want_log),
            )
            .await?;
        router
            .wait_for_metrics(
                i,
                |x| x.last_applied.index() == Some(want_log),
                timeout(),
                &format!("n{}.last_applied -> {}", i, want_log),
            )
            .await?;
    }
    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(500))
}
