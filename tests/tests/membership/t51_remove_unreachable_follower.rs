use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;
use suraft::ServerState;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Replication should stop after a **unreachable** follower is removed from
/// membership.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn stop_replication_to_removed_unreachable_follower_network_failure(
) -> Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());
    router.new_raft_node(s(0)).await;

    let mut log_index = router
        .new_cluster(btreeset! {s(0), s(1), s(2), s(3), s(4)}, btreeset! {})
        .await?;

    tracing::info!(log_index, "--- isolate node 4");
    {
        router.set_network_error(s(4), true);
    }

    // logs on node 4 will stop here:
    let node4_log_index = log_index;

    tracing::info!(log_index, "--- changing config to 0,1,2");
    {
        let node = router.get_raft_handle(&s(0))?;
        node.change_membership([s(0), s(1), s(2)], false).await?;
        log_index += 2;

        for i in &[s(0), s(1), s(2)] {
            router
                .wait(i, timeout())
                .metrics(
                    |x| x.committed >= Some(log_index),
                    "0,1,2 recv 2 change-membership logs",
                )
                .await?;
        }

        router
            .wait(&s(3), timeout())
            .metrics(
                |x| x.committed >= Some(log_index - 1),
                "node-3 recv at least 1 change-membership log",
            )
            .await?;
    }

    tracing::info!(log_index, "--- replication to node 4 will be removed");
    {
        router
            .wait(&s(0), timeout())
            .metrics(
                |x| {
                    x.replication.as_ref().map(|y| y.contains_key(&s(4)))
                        == Some(false)
                },
                "stopped replication to node 4",
            )
            .await?;
    }

    tracing::info!(
        log_index,
        "--- restore network isolation, node 4 won't catch up log and will enter candidate state"
    );
    {
        router.set_network_error(s(4), false);

        router
            .wait(&s(4), timeout())
            .metrics(
                |x| {
                    x.committed == Some(node4_log_index)
                        && (x.server_state == ServerState::Candidate
                            || x.server_state == ServerState::Follower)
                },
                "node 4 stopped recv log and start to elect",
            )
            .await?;
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1000))
}
