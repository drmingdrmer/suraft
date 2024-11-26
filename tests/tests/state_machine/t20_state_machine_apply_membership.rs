use std::sync::Arc;

use anyhow::Result;
use maplit::btreeset;
use suraft::storage::RaftStateMachine;
use suraft::Config;
use suraft::LogId;
use suraft::LogIdOptionExt;
use suraft::Membership;
use suraft::StoredMembership;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// All log should be applied to state machine.
///
/// What does this test do?
///
/// - bring a cluster with 3 voter and 2 learner.
/// - check last_membership in state machine.

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn state_machine_apply_membership() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    tracing::info!("--- initializing cluster");
    let mut log_index =
        router.new_cluster(btreeset! {s(0)}, btreeset! {}).await?;

    for i in 0..=0 {
        let (_sto, mut sm) = router.get_storage_handle(&s(i))?;
        assert_eq!(
            StoredMembership::new(
                Some(LogId::new(0, 0)),
                Membership::new(vec![btreeset! {s(0)}], None)
            ),
            sm.applied_state().await?.1
        );
    }

    // Sync some new nodes.
    router.new_raft_node(s(1)).await;
    router.new_raft_node(s(2)).await;
    router.new_raft_node(s(3)).await;
    router.new_raft_node(s(4)).await;

    tracing::info!(log_index, "--- adding new nodes to cluster");
    {
        router.add_learner(s(0), s(1)).await?;
        router.add_learner(s(0), s(2)).await?;
        router.add_learner(s(0), s(3)).await?;
        router.add_learner(s(0), s(4)).await?;
    }
    log_index += 4;
    router
        .wait_for_log(&btreeset! {s(0)}, Some(log_index), None, "add learner")
        .await?;

    tracing::info!(log_index, "--- changing cluster config");
    let node = router.get_raft_handle(&s(0))?;
    node.change_membership([s(0), s(1), s(2)], false).await?;

    log_index += 2;

    tracing::info!(log_index, "--- every node receives joint log");
    for i in 0..5 {
        router
            .wait(&s(i), None)
            .metrics(
                |x| x.last_applied.index() >= Some(log_index - 1),
                "joint log applied",
            )
            .await?;
    }

    tracing::info!(log_index, "--- only 3 node applied membership config");
    for i in 0..3 {
        router
            .wait(&s(i), None)
            .metrics(
                |x| x.last_applied.index() == Some(log_index),
                "uniform log applied",
            )
            .await?;

        let (_sto, mut sm) = router.get_storage_handle(&s(i))?;
        let (_, last_membership) = sm.applied_state().await?;
        assert_eq!(
            StoredMembership::new(
                Some(LogId::new(1, log_index)),
                Membership::new(
                    vec![btreeset! {s(0), s(1), s(2)}],
                    Some(btreeset! {s(3),s(4)})
                )
            ),
            last_membership
        );
    }

    Ok(())
}
