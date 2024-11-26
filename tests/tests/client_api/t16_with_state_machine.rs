use std::sync::Arc;

use anyhow::Result;
use maplit::btreeset;
use suraft::errors::Fatal;
use suraft::storage::RaftStateMachine;
use suraft::storage::Snapshot;
use suraft::storage::SnapshotMeta;
use suraft::testing::log_id;
use suraft::Config;
use suraft::Entry;
use suraft::LogId;
use suraft::OptionalSend;
use suraft::RaftSnapshotBuilder;
use suraft::StorageError;
use suraft::StoredMembership;
use suraft::TypeConfig;
use suraft_memstore::ClientResponse;
use suraft_memstore::TypeConfig;

use crate::fixtures::s;
use crate::fixtures::ut_harness;
use crate::fixtures::ChannelNetwork;
use crate::fixtures::MemStateMachine;

/// Access [`RaftStateMachine`] via
/// [`SuRaft::with_state_machine()`](suraft::SuRaft::with_state_machine)
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn with_state_machine() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    tracing::info!("--- initializing cluster");
    let log_index =
        router.new_cluster(btreeset! {s(0),s(1),s(2)}, btreeset! {}).await?;

    let n0 = router.get_raft_handle(&s(0))?;

    tracing::info!("--- get last applied from SM");
    {
        let applied = n0
            .with_state_machine(|sm: &mut MemStateMachine| {
                Box::pin(async move {
                    let d = sm.get_state_machine().await;
                    d.last_applied_log
                })
            })
            .await?
            .unwrap();
        assert_eq!(applied, Some(log_id(1, log_index)));
    }

    tracing::info!("--- shutting down node 0");
    n0.shutdown().await?;

    let res = n0
        .with_state_machine(|_sm: &mut MemStateMachine| Box::pin(async move {}))
        .await;
    assert_eq!(Err(Fatal::Stopped), res);

    Ok(())
}

/// Call [`SuRaft::with_state_machine()`](suraft::SuRaft::with_state_machine)
/// with wrong type [`RaftStateMachine`]
#[tracing::instrument]
#[test_harness::test(harness = ut_harness)]
async fn with_state_machine_wrong_sm_type() -> Result<()> {
    let config = Arc::new(
        Config {
            enable_heartbeat: false,
            ..Default::default()
        }
        .validate()?,
    );

    let mut router = ChannelNetwork::new(config.clone());

    tracing::info!("--- initializing cluster");
    router.new_cluster(btreeset! {s(0)}, btreeset! {}).await?;

    let n0 = router.get_raft_handle(&s(0))?;

    tracing::info!("--- use wrong type SM");
    {
        type TC = TypeConfig;
        type Err = std::io::Error;
        struct FooSM;
        impl RaftSnapshotBuilder<TC> for FooSM {
            async fn build_snapshot(&mut self) -> Result<Snapshot<TC>, Err> {
                todo!()
            }
        }
        impl RaftStateMachine<TC> for FooSM {
            type SnapshotBuilder = Self;

            async fn applied_state(
                &mut self,
            ) -> Result<(Option<LogId>, StoredMembership), Err> {
                todo!()
            }

            async fn apply<I>(
                &mut self,
                _entries: I,
            ) -> Result<Vec<ClientResponse>, Err>
            where
                I: IntoIterator<Item = Entry<TC>> + OptionalSend,
                I::IntoIter: OptionalSend,
            {
                todo!()
            }

            async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
                todo!()
            }

            async fn begin_receiving_snapshot(
                &mut self,
            ) -> Result<Box<<TC as TypeConfig>::SnapshotData>, Err>
            {
                todo!()
            }

            async fn install_snapshot(
                &mut self,
                _meta: &SnapshotMeta,
                _snapshot: Box<<TC as TypeConfig>::SnapshotData>,
            ) -> Result<(), Err> {
                todo!()
            }

            async fn get_current_snapshot(
                &mut self,
            ) -> Result<Option<Snapshot<TC>>, Err> {
                todo!()
            }
        }

        let applied = n0
            .with_state_machine::<_, FooSM, _>(|_sm: &mut FooSM| {
                Box::pin(async move {})
            })
            .await?;
        assert!(applied.is_err());
    }

    Ok(())
}
