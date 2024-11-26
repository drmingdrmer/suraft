use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;
use suraft::LogIdOptionExt;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Replication should stop after a follower is removed from membership.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn stop_replication_to_removed_follower() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());
    router.new_raft_node(s(0)).await;

    let mut log_index =
        router.new_cluster(btreeset! {s(0),s(1),s(2)}, btreeset! {}).await?;

    tracing::info!(log_index, "--- add node 3,4");

    router.new_raft_node(s(3)).await;
    router.new_raft_node(s(4)).await;

    router.add_learner(s(0), s(3)).await?;
    router.add_learner(s(0), s(4)).await?;
    log_index += 2;
    router
        .wait_for_log(
            &btreeset! {s(0), s(1), s(2)},
            Some(log_index),
            None,
            "cluster of 2 learners",
        )
        .await?;

    tracing::info!(log_index, "--- changing config to 0,3,4");
    {
        let node = router.get_raft_handle(&s(0))?;
        node.change_membership([s(0), s(3), s(4)], false).await?;
        log_index += 2;

        for i in [s(0), s(3), s(4)] {
            router
                .wait(&i, timeout())
                .metrics(
                    |x| x.committed >= Some(log_index),
                    "new cluster nodes recv 2 change-membership logs",
                )
                .await?;
        }

        let res1 = router
            .wait(&s(1), timeout())
            .metrics(
                |x| x.committed >= Some(log_index - 1),
                "removed node-1 recv at least 1 change-membership log",
            )
            .await;

        let res2 = router
            .wait(&s(2), timeout())
            .metrics(
                |x| x.committed >= Some(log_index - 1),
                "removed node-2 recv at least 1 change-membership log",
            )
            .await;

        tracing::info!("result waiting for node-1: {:?}", res1);
        tracing::info!("result waiting for node-2: {:?}", res2);

        assert!(
            res1.is_ok() || res2.is_ok(),
            "committing the first membership log only need to replication to one of node-1 and node-2. node-1 res: {:?}; node-2 res: {:?}",res1, res2
        );
    }

    tracing::info!(
        log_index,
        "--- write to new cluster, current log={}",
        log_index
    );
    {
        let n = 10;
        router.client_request_many(s(0), "after_change", n).await?;
        log_index += n as u64;

        for i in &[s(0), s(3), s(4)] {
            router
                .wait(i, timeout())
                .metrics(
                    |x| x.last_applied.index() >= Some(log_index),
                    "new cluster recv new logs",
                )
                .await?;
        }
    }

    for i in &[s(1), s(2)] {
        router
            .wait(i, timeout())
            .metrics(
                |x| x.last_applied.index() < Some(log_index),
                "old cluster does not recv new logs",
            )
            .await?;
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1000))
}
