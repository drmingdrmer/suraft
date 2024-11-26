use std::sync::Arc;
use std::time::Duration;

use suraft::storage::LogStorage;
use suraft::Config;
use suraft::ServerState;
use suraft::Vote;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Special case: A leader that is not a member(neither a voter or non-voter)
/// should be started too, as a learner.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn issue_920_non_member_leader_restart() -> anyhow::Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    let (mut log_store, sm) = router.new_store();
    // Set committed vote that believes node 0 is the leader.
    log_store.save_vote(&Vote::new_committed(1, s(0))).await?;
    router.new_raft_node_with_sto(s(0), log_store, sm).await;

    router
        .wait(&s(0), timeout())
        .state(ServerState::Learner, "node 0 becomes learner when startup")
        .await?;

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1000))
}
