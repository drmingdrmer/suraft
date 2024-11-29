use std::sync::Arc;
use std::time::Duration;

use maplit::btreeset;
use suraft::Config;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// A follower that restarted should not interrupt a stable cluster by a too
/// quick election.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn follower_restart_does_not_interrupt() -> anyhow::Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            election_timeout_min: 3_000,
            election_timeout_max: 4_000,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    tracing::info!("--- bring up cluster of 3 nodes");
    let log_index =
        router.new_cluster(btreeset! {s(0),s(1),s(2)}, btreeset! {}).await?;
    let _ = log_index;

    tracing::info!(log_index, "--- stop and restart follower nodes 1,2");
    {
        // Stop followers first or the follower may start re-electing.

        let m = router.get_metrics(&s(0))?;
        let term = m.current_term;

        let (n2, sto2, sm2) = router.remove_node(s(2)).unwrap();
        n2.shutdown().await?;

        let (n1, sto1, sm1) = router.remove_node(s(1)).unwrap();
        n1.shutdown().await?;

        let (n0, _sto0, _sm0) = router.remove_node(s(0)).unwrap();
        n0.shutdown().await?;

        tracing::info!(log_index, "--- restart node 1,2");

        router.new_raft_node_with_sto(s(1), sto1, sm1).await;
        router.new_raft_node_with_sto(s(2), sto2, sm2).await;
        let res = router
            .wait(&s(1), Some(Duration::from_millis(1_000)))
            .metrics(
                |x| x.current_term > term,
                "node increase term to start election",
            )
            .await;

        assert!(res.is_err(), "term should not increase");

        router
            .wait(&s(1), Some(Duration::from_millis(9_000)))
            .metrics(
                |x| x.current_term > term,
                concat!(
                    "node increase term to start election after a election timeout.",
                    "When it starts up, it set the last-update time of the vote to `now`, ",
                    "which make it as if a just renewed vote(has a full leader lease)."
                ),
            )
            .await?;
    }

    Ok(())
}
