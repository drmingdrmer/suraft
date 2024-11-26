use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Large heartbeat should not block replication.
/// I.e., replication should not be driven by heartbeat.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn large_heartbeat() -> Result<()> {
    // Setup test dependencies.
    let config = Arc::new(
        Config {
            heartbeat_interval: 10_000,
            election_timeout_min: 20_000,
            election_timeout_max: 30_000,
            max_payload_entries: 2,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());

    let mut log_index =
        router.new_cluster(btreeset! {s(0)}, btreeset! {s(1)}).await?;

    router.client_request_many(s(0), "foo", 10).await?;
    log_index += 10;

    router
        .wait(&s(1), Some(Duration::from_millis(3_000)))
        .applied_index(Some(log_index), "")
        .await?;

    Ok(())
}
