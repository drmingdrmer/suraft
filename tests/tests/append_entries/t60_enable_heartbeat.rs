use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::impls::TokioRuntime;
use suraft::type_config::AsyncRuntime;
use suraft::type_config::TypeConfigExt;
use suraft::Config;
use suraft_memstore::TypeConfig;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Enable heartbeat, heartbeat should be replicated.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn enable_heartbeat() -> Result<()> {
    // Setup test dependencies.
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());

    let log_index = router
        .new_cluster(btreeset! {s(0),s(1),s(2)}, btreeset! {s(3)})
        .await?;
    let _ = log_index;

    let node0 = router.get_raft_handle(&s(0))?;
    node0.runtime_config().heartbeat(true);

    for _i in 0..3 {
        let now = TypeConfig::now();
        TokioRuntime::sleep(Duration::from_millis(500)).await;

        for node_id in [s(1), s(2), s(3)] {
            // no new log will be sent,
            router
                .wait(&node_id, timeout())
                .applied_index_at_least(
                    Some(log_index),
                    format!("node {} emit heartbeat log", node_id),
                )
                .await?;

            // leader lease is extended.
            router.external_request(node_id, move |state| {
                assert!(state.vote_last_modified() > Some(now));
            });
        }
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1_000))
}
