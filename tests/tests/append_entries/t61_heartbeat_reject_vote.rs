use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::suraft::RequestVote;
use suraft::testing::log_id;
use suraft::Config;
use suraft::TokioInstant;
use suraft::Vote;
use tokio::time::sleep;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// If a follower receives heartbeat, it should reject vote request until leader
/// lease expired.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn heartbeat_reject_vote() -> Result<()> {
    let config = Arc::new(
        Config {
            heartbeat_interval: 200,
            election_timeout_min: 1000,
            election_timeout_max: 1001,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());

    let now = TokioInstant::now();
    sleep(Duration::from_millis(1)).await;

    let log_index = router
        .new_cluster(btreeset! {s(0),s(1),s(2)}, btreeset! {s(3)})
        .await?;

    let vote_modified_time = Arc::new(Mutex::new(Some(TokioInstant::now())));
    tracing::info!(log_index, "--- leader lease is set by heartbeat");
    {
        let m = vote_modified_time.clone();

        router.external_request(s(1), move |state| {
            let mut l = m.lock().unwrap();
            *l = state.vote_last_modified();
            assert!(state.vote_last_modified() > Some(now));
        });

        let now = TokioInstant::now();
        sleep(Duration::from_millis(700)).await;

        let m = vote_modified_time.clone();

        router.external_request(s(1), move |state| {
            let l = m.lock().unwrap();
            assert!(state.vote_last_modified() > Some(now));
            assert!(state.vote_last_modified() > *l);
        });
    }

    let node0 = router.get_raft_handle(&s(0))?;
    let node1 = router.get_raft_handle(&s(1))?;

    tracing::info!(log_index, "--- leader lease rejects vote request");
    {
        let res = node1
            .request_vote(RequestVote::new(
                Vote::new(10, s(2)),
                Some(log_id(10, 10)),
            ))
            .await?;
        assert!(!res.is_granted_to(&Vote::new(10, s(2))), "vote is rejected");
    }

    tracing::info!(
        log_index,
        "--- ensures no more blank-log heartbeat is used"
    );
    {
        // TODO: this part can be removed when blank-log heartbeat is removed.
        sleep(Duration::from_millis(1500)).await;
        router
            .wait(&s(1), timeout())
            .applied_index(Some(log_index), "no log is written")
            .await?;
    }

    tracing::info!(
        log_index,
        "--- disable heartbeat, vote request will be granted"
    );
    {
        node0.runtime_config().heartbeat(false);
        sleep(Duration::from_millis(1500)).await;

        router
            .wait(&s(1), timeout())
            .applied_index(Some(log_index), "no log is written")
            .await?;

        let res = node1
            .request_vote(RequestVote::new(
                Vote::new(10, s(2)),
                Some(log_id(10, 10)),
            ))
            .await?;
        assert!(
            res.is_granted_to(&Vote::new(10, s(2))),
            "vote is granted after leader lease expired"
        );
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1_000))
}
