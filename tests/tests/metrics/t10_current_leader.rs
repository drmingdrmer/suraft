use std::sync::Arc;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Current leader tests.
///
/// What does this test do?
///
/// - create a stable 3-node cluster.
/// - call the current_leader interface on the all nodes, and assert success.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn current_leader() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    let _log_index =
        router.new_cluster(btreeset! {s(0),s(1),s(2)}, btreeset! {}).await?;

    // Get the ID of the leader, and assert that current_leader succeeds.
    let leader = router.leader().expect("leader not found");
    assert_eq!(leader, s(0), "expected leader to be node 0, got {}", leader);

    for i in 0..3 {
        let leader = router.current_leader(s(i)).await;
        assert_eq!(
            leader,
            Some(s(0)),
            "expected leader to be node 0, got {:?}",
            leader
        );
    }

    Ok(())
}
