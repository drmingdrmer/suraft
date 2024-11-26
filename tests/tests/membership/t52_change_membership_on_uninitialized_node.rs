use std::sync::Arc;

use anyhow::Result;
use maplit::btreemap;
use suraft::ChangeMembers;
use suraft::Config;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Call `SuRaft::change_membership()` on an uninitialized node should not panic
/// due to empty membership.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn change_membership_on_uninitialized_node() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());
    router.new_raft_node(s(0)).await;

    let n0 = router.get_raft_handle(&s(0))?;
    let res = n0
        .change_membership(
            ChangeMembers::AddVoters(btreemap! {s(0)=>suraft::emp()}),
            false,
        )
        .await;
    tracing::info!("{:?}", res);

    let err = res.unwrap_err();
    tracing::info!("{}", err);

    assert!(err.to_string().contains("forward request to"));

    Ok(())
}
