use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::network::v2::Connection;
use suraft::network::Network;
use suraft::testing::log_id;
use suraft::Config;
use suraft::LogId;
use suraft::Vote;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// append-entries should update the vote when adding new logs with greater
/// vote.
///
/// - Bring up a learner and send to it append_entries request.
///
/// Check the vote updated.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn append_entries_with_bigger_term() -> Result<()> {
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
    let log_index =
        router.new_cluster(btreeset! {s(0)}, btreeset! {s(1)}).await?;

    // before append entries, check hard state in term 1 and vote for node 0
    router
        .assert_storage_state(
            1,
            log_index,
            Some(s(0)),
            LogId::new(1, log_index),
            None,
        )
        .await?;

    // append entries with term 2 and leader_id, this MUST cause hard state
    // changed in node 0
    let req = AppendEntriesRequest {
        vote: Vote::new_committed(2, s(1)),
        leader_commit: Some(log_id(1, log_index)),
    };

    let option = RPCOption::new(Duration::from_millis(1_000));

    let resp = router
        .new_connection(s(0), &suraft::emp())
        .await
        .append_entries(req, option)
        .await?;
    assert!(resp.is_success());

    // after append entries, check hard state in term 2 and vote for node 1
    let (mut store, mut sm) = router.get_storage_handle(&s(0))?;

    router
        .assert_storage_state_with_sto(
            &mut store,
            &mut sm,
            &s(0),
            2,
            log_index,
            Some(s(1)),
            LogId::new(1, log_index),
            &None,
        )
        .await?;

    Ok(())
}
