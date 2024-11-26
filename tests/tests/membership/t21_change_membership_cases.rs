use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use maplit::btreeset;
use suraft::ChangeMembers;
use suraft::Config;
use suraft::ServerState;
use suraft_memstore::MemNodeId;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;

// --- change ---

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m0_change_m12() -> anyhow::Result<()> {
    change_from_to(btreeset! {s(0)}, btreeset! {s(1),s(2)}).await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m0_change_m123() -> anyhow::Result<()> {
    change_from_to(btreeset! {s(0)}, btreeset! {s(1), s(2), s(3)}).await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m01_change_m12() -> anyhow::Result<()> {
    change_from_to(btreeset! {s(0), s(1)}, btreeset! {s(1),s(2)}).await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m01_change_m1() -> anyhow::Result<()> {
    change_from_to(btreeset! {s(0), s(1)}, btreeset! {s(1)}).await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m01_change_m2() -> anyhow::Result<()> {
    change_from_to(btreeset! {s(0), s(1)}, btreeset! {s(2)}).await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m01_change_m3() -> anyhow::Result<()> {
    change_from_to(btreeset! {s(0), s(1)}, btreeset! {s(3)}).await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m012_change_m4() -> anyhow::Result<()> {
    change_from_to(btreeset! {s(0), s(1), s(2)}, btreeset! {s(4)}).await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m012_change_m456() -> anyhow::Result<()> {
    change_from_to(btreeset! {s(0), s(1), s(2)}, btreeset! {s(4), s(5), s(6)})
        .await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m01234_change_m0123() -> anyhow::Result<()> {
    change_from_to(
        btreeset! {s(0), s(1), s(2), s(3), s(4)},
        btreeset! {s(0), s(1), s(2), s(3)},
    )
    .await
}

// --- add ---

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m0_add_m01() -> anyhow::Result<()> {
    change_by_add(btreeset! {s(0)}, &[s(0), s(1)]).await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m0_add_m12() -> anyhow::Result<()> {
    change_by_add(btreeset! {s(0)}, &[s(1), s(2)]).await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m01_add_m() -> anyhow::Result<()> {
    change_by_add(btreeset! {s(0),s(1)}, &[]).await
}

// --- remove ---

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m012_remove_m01() -> anyhow::Result<()> {
    change_by_remove(btreeset! {s(0),s(1),s(2)}, &[s(0), s(1)]).await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m012_remove_m3() -> anyhow::Result<()> {
    change_by_remove(btreeset! {s(0),s(1),s(2)}, &[s(3)]).await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m012_remove_m() -> anyhow::Result<()> {
    change_by_remove(btreeset! {s(0),s(1),s(2)}, &[]).await
}

#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn m012_remove_m13() -> anyhow::Result<()> {
    change_by_remove(btreeset! {s(0),s(1),s(2)}, &[s(1), s(3)]).await
}

#[tracing::instrument(level = "debug")]
async fn change_from_to(
    old: BTreeSet<MemNodeId>,
    change_members: BTreeSet<MemNodeId>,
) -> anyhow::Result<()> {
    let new = change_members;

    let mes = format!("from {:?} to {:?}", old, new);

    let only_in_old = old.difference(&new);
    let only_in_new = new.difference(&old);

    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            enable_elect: false,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());

    let mut log_index = router.new_cluster(old.clone(), btreeset! {}).await?;

    tracing::info!(log_index, "--- write 10 logs");
    {
        router.client_request_many(s(0), "client", 10).await?;
        log_index += 10;

        router
            .wait_for_log(
                &old,
                Some(log_index),
                timeout(),
                &format!("write 10 logs, {}", mes),
            )
            .await?;
    }

    let orig_leader =
        router.leader().expect("expected the cluster to have a leader");

    tracing::info!(log_index, "--- change to {:?}", new);
    {
        for id in only_in_new {
            router.new_raft_node(id.clone()).await;
            router.add_learner(s(0), id.clone()).await?;
            log_index += 1;
            router
                .wait_for_log(
                    &old,
                    Some(log_index),
                    timeout(),
                    &format!("add learner, {}", mes),
                )
                .await?;
        }

        let node = router.get_raft_handle(&s(0))?;
        node.change_membership(new.clone(), false).await?;
        log_index += 1;
        if new != old {
            // Two member-change logs.
            log_index += 1;
        }

        tracing::info!(log_index, "--- let a node in the new cluster elect");
        {
            let n = router.get_raft_handle(new.iter().next().unwrap())?;
            n.runtime_config().elect(true);
        }

        tracing::info!(log_index, "--- wait for old leader or new leader");
        {
            for id in new.iter() {
                router
                    .wait(id, Some(Duration::from_millis(5_000)))
                    .metrics(
                        |x| x.current_leader.is_some() && new.contains(&x.current_leader.clone().unwrap()),
                        format!("node {} in new cluster has leader in new cluster, {}", id, mes),
                    )
                    .await?;
            }
        }

        let new_leader =
            router.leader().expect("expected the cluster to have a leader");
        for id in new.iter() {
            // new leader may already elected and committed a blank log.
            router
                .wait(id, timeout())
                .applied_index_at_least(
                    Some(log_index),
                    format!("new cluster, {}", mes),
                )
                .await?;

            if new_leader != orig_leader {
                router
                    .wait(id, timeout())
                    .metrics(
                        |x| x.current_term >= 2,
                        "new cluster has term >= 2 because of new election",
                    )
                    .await?;
            }
        }
    }

    tracing::info!(log_index, "--- removed nodes are left in non-leader state");
    {
        for id in only_in_old.clone() {
            router
                .wait(id, timeout())
                .metrics(
                    |x| {
                        x.server_state == ServerState::Follower
                            || x.server_state == ServerState::Learner
                            || x.server_state == ServerState::Candidate
                    },
                    format!("node {} only in old, {}", id, mes),
                )
                .await?;
        }
    }

    tracing::info!(log_index, "--- write another 10 logs");
    {
        // get new leader

        // TODO(xp): leader may not be stable, other node may take leadership by
        // a higher vote.           Then client write may receive a
        // ForwardToLeader Error with empty leader.           Need to
        // wait for the leader become stable.
        let m = router
            .wait(new.iter().next().unwrap(), timeout())
            .metrics(
                |x| x.current_leader.is_some(),
                format!("wait for new leader, {}", mes),
            )
            .await?;

        let leader = m.current_leader.unwrap();

        router.client_request_many(leader, "client", 10).await?;
        log_index += 10;
    }

    for id in new.iter() {
        router
            .wait(id, timeout())
            // new leader may commit a blank log
            .applied_index_at_least(
                Some(log_index),
                format!("new cluster recv logs 10~20, {}", mes),
            )
            .await?;
    }

    tracing::info!(log_index, "--- log will not be sync to removed node");
    {
        for id in only_in_old {
            let res = router
                .wait(id, timeout())
                .applied_index(
                    Some(log_index),
                    format!(
                        "node {} in old cluster wont recv new logs, {}",
                        id, mes
                    ),
                )
                .await;
            assert!(res.is_err());
        }
    }

    Ok(())
}

/// Test change-membership by adding voters.
#[tracing::instrument(level = "debug")]
async fn change_by_add(
    old: BTreeSet<MemNodeId>,
    add: &[MemNodeId],
) -> anyhow::Result<()> {
    let change = ChangeMembers::AddVoterIds(add.iter().cloned().collect());

    let mes = format!("from {:?} {:?}", old, change);

    let new = old
        .clone()
        .union(&add.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    let only_in_new = new.difference(&old);

    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());

    let mut log_index = router.new_cluster(old.clone(), btreeset! {}).await?;

    tracing::info!(log_index, "--- write 10 logs");
    {
        log_index += router.client_request_many(s(0), "client", 10).await?;
        for id in old.iter() {
            router
                .wait(id, timeout())
                .applied_index(
                    Some(log_index),
                    format!("write 10 logs, {}", mes),
                )
                .await?;
        }
    }

    let leader_id =
        router.leader().expect("expected the cluster to have a leader");

    tracing::info!(log_index, "--- add learner before change-membership");
    {
        for id in only_in_new {
            router.new_raft_node(id.clone()).await;
            router.add_learner(s(0), id.clone()).await?;
            log_index += 1;
            router
                .wait(id, timeout())
                .applied_index(Some(log_index), format!("add learner, {}", mes))
                .await?;
        }
    }

    tracing::info!(log_index, "--- change: {:?}", change);
    {
        let node = router.get_raft_handle(&s(0))?;
        node.change_membership(change, false).await?;
        log_index += 1;
        if new != old {
            log_index += 1; // two member-change logs
        }

        for id in new.iter() {
            router
                .wait(id, timeout())
                .applied_index_at_least(
                    Some(log_index),
                    format!("new cluster, {}", mes),
                )
                .await?;
        }
    }

    tracing::info!(log_index, "--- write another 10 logs");
    {
        log_index +=
            router.client_request_many(leader_id, "client", 10).await?;

        let mes = format!("new cluster recv logs 10~20, {}", mes);

        for id in new.iter() {
            router
                .wait(id, timeout())
                .applied_index_at_least(Some(log_index), &mes)
                .await?;
        }
    }

    Ok(())
}

#[tracing::instrument(level = "debug")]
async fn change_by_remove(
    old: BTreeSet<MemNodeId>,
    remove: &[MemNodeId],
) -> anyhow::Result<()> {
    let change = ChangeMembers::RemoveVoters(remove.iter().cloned().collect());

    let mes = format!("from {:?} {:?}", old, change);

    let new = old
        .clone()
        .difference(&remove.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    let only_in_old = old.difference(&new);

    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            enable_elect: false,
            ..Default::default()
        }
        .validate()?,
    );
    let mut router = ChannelNetwork::new(config.clone());

    let mut log_index = router.new_cluster(old.clone(), btreeset! {}).await?;

    tracing::info!(log_index, "--- write 10 logs");
    {
        log_index += router.client_request_many(s(0), "client", 10).await?;
        for id in old.iter() {
            router
                .wait(id, timeout())
                .applied_index(
                    Some(log_index),
                    format!("write 10 logs, {}", mes),
                )
                .await?;
        }
    }

    let orig_leader =
        router.leader().expect("expected the cluster to have a leader");

    tracing::info!(log_index, "--- change {:?}", &change);
    {
        let node = router.get_raft_handle(&s(0))?;
        node.change_membership(change.clone(), false).await?;
        log_index += 1;
        if new != old {
            // Two member-change logs
            log_index += 1;
        }

        tracing::info!(log_index, "--- let a node in the new cluster elect");
        {
            let n = router.get_raft_handle(new.iter().next().unwrap())?;
            n.runtime_config().elect(true);
        }

        tracing::info!(log_index, "--- wait for old leader or new leader");
        {
            for id in new.iter() {
                router
                    .wait(id, Some(Duration::from_millis(5_000)))
                    .metrics(
                        |x| x.current_leader.is_some() && new.contains(x.current_leader.as_ref().unwrap()),
                        format!("node {} in new cluster has leader in new cluster, {}", id, mes),
                    )
                    .await?;
            }
        }

        let new_leader =
            router.leader().expect("expected the cluster to have a leader");
        for id in new.iter() {
            // new leader may already elected and committed a blank log.
            router
                .wait(id, timeout())
                .applied_index_at_least(
                    Some(log_index),
                    format!("new cluster, {}", mes),
                )
                .await?;

            if new_leader != orig_leader {
                router
                    .wait(id, timeout())
                    .metrics(
                        |x| x.current_term >= 2,
                        "new cluster has term >= 2 because of new election",
                    )
                    .await?;
            }
        }
    }

    tracing::info!(log_index, "--- removed nodes are left in follower state");
    {
        for id in only_in_old.clone() {
            // The removed node will be left in follower state, it can never
            // elect itself successfully.
            router
                .wait(id, timeout())
                .metrics(
                    |x| x.server_state != ServerState::Leader,
                    format!("node {} only in old, {}", id, mes),
                )
                .await?;
        }
    }

    tracing::info!(log_index, "--- write another 10 logs");
    {
        // TODO(xp): leader may not be stable, other node may take leadership by
        // a higher vote.           Then client write may receive a
        // ForwardToLeader Error with empty leader.           Need to
        // wait for the leader become stable.
        let m = router
            .wait(new.iter().next().unwrap(), timeout())
            .metrics(
                |x| x.current_leader.is_some(),
                format!("wait for new leader, {}", mes),
            )
            .await?;

        let leader_id = m.current_leader.unwrap();

        log_index +=
            router.client_request_many(leader_id, "client", 10).await?;
    }

    for id in new.iter() {
        router
            .wait(id, timeout())
            // new leader may commit a blank log
            .applied_index_at_least(
                Some(log_index),
                format!("new cluster recv logs 10~20, {}", mes),
            )
            .await?;
    }

    tracing::info!(log_index, "--- log will not be sync to removed node");
    {
        for id in only_in_old {
            let res = router
                .wait(id, timeout())
                .applied_index(
                    Some(log_index),
                    format!(
                        "node {} in old cluster wont recv new logs, {}",
                        id, mes
                    ),
                )
                .await;
            assert!(res.is_err());
        }
    }

    Ok(())
}

fn timeout() -> Option<Duration> {
    Some(Duration::from_millis(1000))
}
