use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::storage::LogStorage;
use suraft::testing::log_id;
use suraft::Config;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Before applying log, write `committed` log id to log store.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn write_committed_log_id_to_log_store() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_tick: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    tracing::info!("--- initializing cluster");
    let mut log_index =
        router.new_cluster(btreeset! {s(0),s(1),s(2)}, btreeset! {}).await?;

    log_index += router.client_request_many(s(0), "0", 10).await?;

    for i in [s(0), s(1), s(2)] {
        router
            .wait(&i, timeout())
            .applied_index(Some(log_index), "write logs")
            .await?;
    }

    for id in [s(0), s(1), s(2)] {
        let (_, mut ls, _) = router.remove_node(id.clone()).unwrap();
        let committed = ls.read_committed().await?;
        assert_eq!(
            Some(log_id(1, log_index)),
            committed,
            "node-{} committed",
            id
        );
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1000))
}
