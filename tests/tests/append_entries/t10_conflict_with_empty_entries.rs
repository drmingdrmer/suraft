use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use suraft::network::v2::Connection;
use suraft::network::Network;
use suraft::Config;
use suraft::LogId;
use suraft::Vote;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// Cluster conflict_with_empty_entries test.
///
/// `append_entries` should get a response with non-none ConflictOpt even if the
/// entries in message is empty.
/// Otherwise if no conflict is found the leader will never be able to sync logs
/// to a new added Learner, until a next log is proposed on leader.
///
/// What does this test do?
///
/// - brings a 1 Learner node online.
///
/// - send `append_logs` message to it with empty `entries` and some non-zero
///   `prev_log_index`.
/// - asserts that a response with ConflictOpt set.
///
/// - feed several logs to it.
///
/// - send `append_logs` message with conflicting prev_log_index and empty
///   `entries`.
/// - asserts that a response with ConflictOpt set.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn conflict_with_empty_entries() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    router.new_raft_node(s(0)).await;

    // Expect conflict even if the message contains no entries.

    let rpc = AppendEntriesRequest {
        vote: Vote::new_committed(1, s(1)),
        leader_commit: Some(LogId::new(1, 5)),
    };

    let option = RPCOption::new(Duration::from_millis(1_000));
    let resp = router
        .new_connection(s(0), &suraft::emp())
        .await
        .append_entries(rpc, option)
        .await?;
    assert!(!resp.is_success());
    assert!(resp.is_conflict());

    // Feed logs

    let rpc = AppendEntriesRequest {
        vote: Vote::new_committed(1, s(1)),
        leader_commit: Some(LogId::new(1, 5)),
    };

    let option = RPCOption::new(Duration::from_millis(1_000));

    let resp = router
        .new_connection(s(0), &suraft::emp())
        .await
        .append_entries(rpc, option)
        .await?;
    assert!(resp.is_success());
    assert!(!resp.is_conflict());

    // Expect a conflict with prev_log_index == 3

    let rpc = AppendEntriesRequest {
        vote: Vote::new_committed(1, s(1)),
        leader_commit: Some(LogId::new(1, 5)),
    };

    let option = RPCOption::new(Duration::from_millis(1_000));

    let resp = router
        .new_connection(s(0), &suraft::emp())
        .await
        .append_entries(rpc, option)
        .await?;
    assert!(!resp.is_success());
    assert!(resp.is_conflict());

    Ok(())
}
