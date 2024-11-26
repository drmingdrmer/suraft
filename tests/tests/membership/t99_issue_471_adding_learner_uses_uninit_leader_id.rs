use std::sync::Arc;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// When adding learner and waiting for the learner to become up to date,
/// it should not try to use `matched.leader_id` which may be uninitialized,
/// i.e., `(0,0)`. https://github.com/suraft/suraft/issues/471
///
/// - Brings up 1 leader.
/// - Add learner at once.
/// - It should not panic.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn adding_learner_do_not_use_matched_leader_id() -> Result<()> {
    let config = Arc::new(
        Config {
            // Replicate log one by one, to trigger a state report with
            // matched=(0,0,0), which is the first log id.
            max_payload_entries: 1,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());

    router.new_cluster(btreeset! {s(0)}, btreeset! {}).await?;

    tracing::info!("--- feed 2 log to make replication busy");
    {
        router.client_request_many(s(0), "foo", 2).await?;
    }

    // Delay replication.
    router.network_send_delay(100);

    tracing::info!("--- add learner: node-1");
    {
        router.new_raft_node(s(1)).await;
        router.add_learner(s(0), s(1)).await?;
    }

    Ok(())
}
