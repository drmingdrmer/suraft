use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::errors::AllowNextRevertError;
use suraft::errors::ForwardToLeader;
use suraft::errors::NodeNotFound;
use suraft::errors::Operation;
use suraft::Config;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// With `Trigger::allow_next_revert()`  the leader allows follower to revert
/// its log to an earlier state for one time.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn allow_follower_log_revert() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_tick: false,
            enable_heartbeat: false,
            // Make sure the replication is done in more than one steps
            max_payload_entries: 1,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    tracing::info!("--- initializing cluster");
    let mut log_index =
        router.new_cluster(btreeset! {s(0)}, btreeset! {s(1)}).await?;

    tracing::info!(log_index, "--- write 10 logs");
    {
        log_index += router.client_request_many(s(0), "0", 10).await?;
        for i in [s(0), s(1)] {
            router
                .wait(&i, timeout())
                .applied_index(Some(log_index), format!("{} writes", 10))
                .await?;
        }
    }
    tracing::info!(log_index, "--- allow next detected log revert");
    {
        let n0 = router.get_raft_handle(&s(0))?;
        n0.trigger().allow_next_revert(&s(1), true).await??;
    }

    tracing::info!(log_index, "--- erase Learner-1 and restart");
    {
        let (_raft, _ls, _sm) = router.remove_node(s(1)).unwrap();
        let (log, sm) = suraft_memstore::new_mem_store();

        router.new_raft_node_with_sto(s(1), log, sm).await;
        router.add_learner(s(0), s(1)).await?;
        log_index += 1; // add learner
    }

    tracing::info!(
        log_index,
        "--- write another 10 logs, leader should not panic"
    );
    {
        log_index += router.client_request_many(s(0), "0", 10).await?;
        for i in [s(0), s(1)] {
            router
                .wait(&i, timeout())
                .applied_index(Some(log_index), format!("{} writes", 10))
                .await?;
        }
    }

    Ok(())
}

/// Test error returned when `Trigger::allow_next_revert()` is called.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn allow_follower_log_revert_errors() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_tick: false,
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    tracing::info!("--- initializing cluster");
    let log_index =
        router.new_cluster(btreeset! {s(0)}, btreeset! {s(1)}).await?;

    tracing::info!(
        log_index,
        "--- allow next detected log revert to unknown node"
    );
    {
        let n0 = router.get_raft_handle(&s(0))?;
        let res = n0.trigger().allow_next_revert(&s(2), true).await?;
        assert_eq!(
            Err(AllowNextRevertError::NodeNotFound(NodeNotFound::new(
                s(2),
                Operation::AllowNextRevert
            ))),
            res
        );
    }

    tracing::info!(
        log_index,
        "--- allow next detected log revert on non-leader node"
    );
    {
        let n1 = router.get_raft_handle(&s(1))?;
        let res = n1.trigger().allow_next_revert(&s(0), true).await?;
        assert_eq!(
            Err(AllowNextRevertError::ForwardToLeader(ForwardToLeader::new(
                s(0),
                suraft::emp()
            ))),
            res
        );
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1_000))
}
