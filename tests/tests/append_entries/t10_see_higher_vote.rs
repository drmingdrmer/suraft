use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use maplit::btreeset;
use suraft::network::v2::Connection;
use suraft::network::Network;
use suraft::suraft::RequestVote;
use suraft::Config;
use suraft::LogId;
use suraft::ServerState;
use suraft::Vote;
use suraft_memstore::ClientRequest;
use tokio::time::sleep;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

/// A leader reverts to follower if a higher vote is seen when append-entries.
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn append_sees_higher_vote() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            enable_elect: false,
            election_timeout_min: 500,
            election_timeout_max: 501,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    let log_index =
        router.new_cluster(btreeset! {s(0),s(1)}, btreeset! {}).await?;

    tracing::info!(log_index, "--- upgrade vote on node-1");
    {
        // Let leader lease expire
        sleep(Duration::from_millis(800)).await;

        let resp = router
            .new_connection(s(1), &suraft::emp())
            .await
            .request_vote(
                RequestVote {
                    vote: Vote::new(10, s(1)),
                    last_log_id: Some(LogId::new(10, 5)),
                },
                option,
            )
            .await?;

        assert!(resp.is_granted_to(&Vote::new(10, s(1))));
    }

    // Current state:
    // n0: vote=(1,0)
    // n1: vote=(10,1)
    tracing::info!("--- a write operation will see a higher vote, then the leader revert to follower");
    {
        router
            .wait(&s(0), timeout())
            .state(ServerState::Leader, "node-0 is leader")
            .await?;

        let n0 = router.get_raft_handle(&s(0))?;
        tokio::spawn(async move {
            let res = n0
                .client_write(ClientRequest {
                    key: "0".to_string(),
                    value: "2".to_string(),
                })
                .await;

            tracing::debug!("--- client_write res: {:?}", res);
        });

        tokio::time::sleep(Duration::from_millis(500)).await;

        router
            .wait(&s(0), timeout())
            .state(
                ServerState::Follower,
                "node-0 becomes follower due to a higher vote",
            )
            .await?;

        router.external_request(s(0), |st| {
            assert_eq!(
                &Vote::new(10, s(1)),
                st.vote_ref(),
                "higher vote is stored"
            );
        });
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1_000))
}
