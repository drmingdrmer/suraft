use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::Config;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Ensures the stale value of ReplicationCore.last_log_id won't affect
/// replication. If `ReplicationCore.last_log_id` is used, the end position of
/// log for loading may underflow the start.
///
/// TODO(xp): `max_applied_log_to_keep` to be 0 makes it very easy to enter
/// snapshot replication and it will keeps           replicating every log by
/// snapshot and get timeout.           Thus it is disabled until we find
/// another way to test it.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
#[ignore]
async fn stale_last_log_id() -> Result<()> {
    // Setup test dependencies.
    let config = Arc::new(
        Config {
            election_timeout_min: 500,
            election_timeout_max: 1000,
            max_payload_entries: 1,
            max_in_snapshot_log_to_keep: 0,
            purge_batch_size: 1,
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());
    router.network_send_delay(5);

    let mut log_index = router
        .new_cluster(btreeset! {s(0),s(1),s(2)}, btreeset! {s(3),s(4)})
        .await?;

    let n_threads = 4;
    let n_ops = 500;
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    for i in 0..n_threads {
        tokio::spawn({
            let router = router.clone();
            let tx = tx.clone();

            async move {
                router
                    .client_request_many(s(0), &format!("{}", i), n_ops)
                    .await
                    .unwrap();
                let _ = tx.send(());
            }
        });
    }

    for _i in 0..n_threads {
        let _ = rx.recv().await;
        log_index += n_ops as u64;
    }

    router
        .wait(&s(1), Some(Duration::from_millis(1000)))
        .applied_index(Some(log_index), "")
        .await?;
    router
        .wait(&s(2), Some(Duration::from_millis(1000)))
        .applied_index(Some(log_index), "")
        .await?;
    router
        .wait(&s(3), Some(Duration::from_millis(1000)))
        .applied_index(Some(log_index), "")
        .await?;
    router
        .wait(&s(4), Some(Duration::from_millis(1000)))
        .applied_index(Some(log_index), "")
        .await?;

    Ok(())
}
