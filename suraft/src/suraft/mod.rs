//! Public interface and data types.
//!
//! [`SuRaft`] serves as the primary interface to a SuRaft node,
//! facilitating all interactions with the underlying Core.
//!
//! While `Core` operates as a singleton within an application, [`SuRaft`]
//! instances are designed to be cheaply cloneable.
//! This allows multiple components within the application that require
//! interaction with `Core` to efficiently share access.

mod change_membership;
mod inner;
mod request_vote;
pub mod responder;
mod runtime_config_handle;
mod write_result;

use std::collections::BTreeMap;
use std::error::Error;
use std::future::Future;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use tracing::info;
use tracing::trace_span;
use tracing::Instrument;
use tracing::Level;

pub use self::request_vote::RequestVote;
pub use self::request_vote::VoteReply;
pub use self::write_result::WriteResult;
use crate::async_runtime::watch::WatchReceiver;
use crate::base::leased::Leased;
use crate::config::Config;
use crate::config::RuntimeConfig;
use crate::core::core::Core;
use crate::core::core_state::CoreState;
use crate::core::io::api_message::APIMessage;
use crate::core::Tick;
use crate::errors::Fatal;
use crate::errors::ForwardToLeader;
use crate::metrics::Metrics;
use crate::metrics::Wait;
use crate::storage::log::log_id::LogId;
use crate::storage::membership::Membership;
use crate::storage::membership::NodeId;
use crate::storage::LogStorage;
use crate::storage::LogStorageExt;
use crate::suraft::inner::SuRaftInner;
use crate::suraft::responder::Responder;
pub use crate::suraft::runtime_config_handle::RuntimeConfigHandle;
use crate::type_config::alias::AsyncRuntimeOf;
use crate::type_config::alias::ResponderOf;
use crate::type_config::alias::ResponderReceiverOf;
use crate::type_config::alias::WatchReceiverOf;
use crate::type_config::TypeConfigExt;
use crate::Network;
use crate::Node;
use crate::OptionalSend;
pub use crate::TypeConfig;

/// The SuRaft API.
///
/// ### Clone
///
/// This type implements `Clone`, and cloning itself is very cheap and helps to
/// facilitate use with async workflows.
///
/// ### Shutting down
///
/// If any of the interfaces returns a [`Fatal`], this indicates that
/// the SuRaft node is shutting down. If the parent application needs to
/// shutdown the SuRaft node for any reason, calling `shutdown` will do the
/// trick.
#[derive(Clone)]
pub struct SuRaft<C>
where C: TypeConfig
{
    inner: Arc<SuRaftInner<C>>,
}

impl<C> SuRaft<C>
where C: TypeConfig
{
    pub async fn initialize<Log>(
        log_store: &mut Log,
        nodes: BTreeMap<NodeId, Node>,
    ) -> Result<(), io::Error>
    where
        Log: LogStorage<C>,
    {
        let membership = log_store.read_membership().await?;
        if let Some(m) = membership {
            info!("Already initialized: {}", m);
            return Ok(());
        }

        let m = Membership::new(nodes);
        log_store.write_membership(&m).await?;

        info!("Initialized with: {}", m);

        Ok(())
    }

    /// Create and spawn a new SuRaft task.
    ///
    /// ### `id`
    /// The ID which the spawned SuRaft task will use to identify itself within
    /// the cluster. Applications must guarantee that the ID provided to
    /// this function is stable, and should be persisted in a well known
    /// location, probably alongside the SuRaft log and the application's
    /// state machine. This ensures that restarts of the node will yield the
    /// same ID every time.
    ///
    /// ### `config`
    /// SuRaft's runtime config. See the docs on the `Config` object for more
    /// details.
    ///
    /// ### `network`
    /// An implementation of the [`Network`] trait which will be used
    /// by SuRaft for sending RPCs to peer nodes within the cluster.
    ///
    /// ### `storage`
    /// An implementation of the [`LogStorage`] and [`RaftStateMachine`]
    /// trait which will be used by SuRaft for data storage.
    #[tracing::instrument(level = "debug", skip_all)]
    pub fn new<Log, Net>(
        id: NodeId,
        config: Arc<Config>,
        network: Net,
        log_store: Log,
    ) -> Self
    where
        Net: Network<C>,
        Log: LogStorage<C>,
    {
        let (tx_api, rx_api) = C::mpsc_unbounded();
        let (tx_notify, rx_notify) = C::mpsc_unbounded();
        let (tx_metrics, rx_metrics) =
            C::watch_channel(Metrics::new_initial(id.clone()));

        let tick_handle = Tick::spawn(
            Duration::from_millis(config.heartbeat_interval * 3 / 2),
            tx_notify.clone(),
            config.enable_tick,
        );

        let runtime_config = Arc::new(RuntimeConfig::new(&config));

        let core_span = tracing::span!(
            parent: tracing::Span::current(),
            Level::DEBUG,
            "Core",
            id = display(&id),
        );

        let core: Core<C, Net, Log> = Core {
            id: id.clone(),
            config: config.clone(),
            election_timeout: config
                .new_rand_election_timeout::<AsyncRuntimeOf<C>>(),
            runtime_config: runtime_config.clone(),
            network,
            log_store,

            vote: Leased::new(C::now(), Duration::default(), None),

            committed: None,
            leader: None,
            candidate: None,

            rx_api,

            tx_notification: tx_notify,
            rx_notification: rx_notify,

            tx_metrics,

            span: core_span,
        };

        let core_handle =
            C::spawn(core.main().instrument(trace_span!("spawn").or_current()));

        let inner = SuRaftInner {
            id,
            config,
            runtime_config,
            tick_handle,
            tx_api,
            rx_metrics,
            core_state: std::sync::Mutex::new(CoreState::Running(core_handle)),
        };

        Self {
            inner: Arc::new(inner),
        }
    }

    /// Return a handle to update runtime config.
    ///
    /// Such enabling/disabling heartbeat, election, etc.
    ///
    /// Example:
    /// ```ignore
    /// let raft = SuRaft::new(...).await?;
    /// raft.runtime_config().heartbeat(true);
    /// raft.runtime_config().tick(true);
    /// raft.runtime_config().elect(true);
    /// ```
    pub fn runtime_config(&self) -> RuntimeConfigHandle<C> {
        RuntimeConfigHandle::new(self.inner.as_ref())
    }

    /// Return the config of this SuRaft node.
    pub fn config(&self) -> &Arc<Config> {
        &self.inner.config
    }

    /// Trigger election at once and return at once.
    ///
    /// Returns error when Core has [`Fatal`] error, e.g. shut down or
    /// having storage error. It is not affected by
    /// `SuRaft::enable_elect(false)`.
    pub async fn elect(&self) -> Result<(), Fatal> {
        let msg = APIMessage::Elect;
        self.inner.send_msg(msg).await?;
        Ok(())
    }

    /// Trigger a heartbeat at once and return at once.
    ///
    /// Returns error when Core has [`Fatal`] error, e.g. shut down or
    /// having storage error. It is not affected by
    /// `SuRaft::enable_heartbeat(false)`.
    pub async fn broadcast_heartbeat(&self) -> Result<(), Fatal> {
        let msg = APIMessage::BroadcastHeartbeat;
        self.inner.send_msg(msg).await?;
        Ok(())
    }

    /// Submit a VoteRequest (RequestVote in the spec) RPC to this SuRaft node.
    ///
    /// These RPCs are sent by cluster peers which are in candidate state
    /// attempting to gather votes (§5.2).
    pub async fn handle_request_vote(
        &self,
        rpc: RequestVote,
    ) -> Result<VoteReply, Fatal> {
        tracing::info!(rpc = display(&rpc), "handle_request_vote()");

        let (tx, rx) = C::oneshot();
        self.inner.call_core(APIMessage::RequestVote { rpc, tx }, rx).await
    }

    /// Get the ID of the current leader from this SuRaft node.
    ///
    /// This method is based on the SuRaft metrics system which does a good job
    /// at staying up-to-date; however, the `is_leader` method must still be
    /// used to guard against stale reads. This method is perfect for making
    /// decisions on where to route client requests.
    pub async fn current_leader(&self) -> Option<NodeId> {
        let m = self.metrics();
        let mm = m.borrow_watched();
        let v = mm.vote.as_ref();
        v.map(|x| x.voted_for().clone())
    }

    /// Submit a mutating client request to SuRaft to update the state of the
    /// system (§5.1).
    ///
    /// It will be appended to the log, committed to the cluster, and then
    /// applied to the application state machine. The result of applying the
    /// request to the state machine will be returned as the response from
    /// this method.
    ///
    /// Our goal for SuRaft is to implement linearizable semantics. If the
    /// leader crashes after committing a log entry but before responding to
    /// the client, the client may retry the command with a new leader,
    /// causing it to be executed a second time. As such, clients should
    /// assign unique serial numbers to every command. Then, the state
    /// machine should track the latest serial number processed for each
    /// client, along with the associated response. If it receives a command
    /// whose serial number has already been executed, it responds
    /// immediately without re-executing the request (§8). The
    /// [`RaftStateMachine::apply`] method is the perfect place to implement
    /// this.
    ///
    /// These are application specific requirements, and must be implemented by
    /// the application which is being built on top of SuRaft.
    pub async fn write<E>(
        &self,
        app_data: C::AppData,
    ) -> Result<Result<LogId, ForwardToLeader>, Fatal>
    where
        ResponderReceiverOf<C>: Future<Output = Result<WriteResult, E>>,
        E: Error + OptionalSend,
    {
        let rx = self.write_ff(app_data).await?;

        let res: WriteResult = self.inner.recv_msg(rx).await?;

        Ok(res)
    }

    /// Submit a mutating client request to SuRaft to update the state machine,
    /// returns an application defined response receiver
    /// [`Responder::Receiver`].
    ///
    /// `_ff` means fire and forget.
    ///
    /// It is same as [`SuRaft::client_write`] but does not wait for the
    /// response.
    #[tracing::instrument(level = "debug", skip(self, app_data))]
    pub async fn write_ff(
        &self,
        app_data: C::AppData,
    ) -> Result<ResponderReceiverOf<C>, Fatal> {
        let (app_data, tx, rx) = ResponderOf::<C>::from_app_data(app_data);

        self.inner.send_msg(APIMessage::Write { app_data, tx }).await?;

        Ok(rx)
    }

    /// Get a handle to the metrics channel.
    pub fn metrics(&self) -> WatchReceiverOf<C, Metrics<C>> {
        self.inner.rx_metrics.clone()
    }

    /// Get a handle to wait for the metrics to satisfy some condition.
    ///
    /// If `timeout` is `None`, then it will wait forever(10 years).
    /// If `timeout` is `Some`, then it will wait for the specified duration.
    ///
    /// ```ignore
    /// # use std::time::Duration;
    /// # use suraft::{State, SuRaft};
    ///
    /// let timeout = Duration::from_millis(200);
    ///
    /// // wait for raft log-3 to be received and applied:
    /// r.wait(Some(timeout)).log(Some(3), "log").await?;
    ///
    /// // wait for ever for raft node's current leader to become 3:
    /// r.wait(None).current_leader(2, "wait for leader").await?;
    ///
    /// // wait for raft state to become a follower
    /// r.wait(None).state(State::Follower, "state").await?;
    /// ```
    pub fn wait(&self, timeout: Option<Duration>) -> Wait<C> {
        let timeout = match timeout {
            Some(t) => t,
            None => Duration::from_secs(86400 * 365 * 100),
        };
        Wait {
            timeout,
            rx: self.inner.rx_metrics.clone(),
        }
    }
}
