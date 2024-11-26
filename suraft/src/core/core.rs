use std::mem::take;
use std::ops::Deref;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use anyerror::AnyError;
use futures::FutureExt;
use tokio::io;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;
use tracing::Instrument;
use tracing::Level;
use tracing::Span;

use crate::async_runtime::watch::WatchSender;
use crate::async_runtime::MpscUnboundedSender;
use crate::async_runtime::OneshotSender;
use crate::async_runtime::TryRecvError;
use crate::base::display_ext::DisplayInstantExt;
use crate::base::display_ext::DisplayOptionExt;
use crate::base::leased::Leased;
use crate::config::Config;
use crate::config::RuntimeConfig;
use crate::core::io::api_message::APIMessage;
use crate::core::io::balancer::Balancer;
use crate::core::io::notification::Notification;
use crate::core::roles::candidate::Candidate;
use crate::core::roles::leader::Leader;
use crate::core::roles::leader_handler::LeaderHandler;
use crate::core::roles::vote_handler::VoteHandler;
use crate::core::CandidateState;
use crate::core::LeaderState;
use crate::errors::to_any_error;
use crate::errors::Fatal;
use crate::errors::ForwardToLeader;
use crate::errors::Infallible;
use crate::metrics::Metrics;
use crate::metrics::SerdeInstant;
use crate::metrics::ServerState;
use crate::network::connection::Connection;
use crate::network::Network;
use crate::quorum::progress::Progress;
use crate::storage::log::entry::Entry;
use crate::storage::log::log_id::LogId;
use crate::storage::log::log_id::LogIdOptionExt;
use crate::storage::membership::NodeId;
use crate::storage::vote::vote_ext::VoteExt;
use crate::storage::vote::Vote;
use crate::storage::ClientCallback;
use crate::storage::IOFlushed;
use crate::storage::LogStorage;
use crate::storage::LogStorageExt;
use crate::suraft::responder::Responder;
use crate::suraft::RequestVote;
use crate::suraft::VoteReply;
use crate::type_config::alias::InstantOf;
use crate::type_config::alias::MpscUnboundedReceiverOf;
use crate::type_config::alias::MpscUnboundedSenderOf;
use crate::type_config::alias::ResponderOf;
use crate::type_config::alias::WatchSenderOf;
use crate::type_config::async_runtime::MpscUnboundedReceiver;
use crate::type_config::TypeConfigExt;
use crate::TypeConfig;

/// The core type implementing the SuRaft protocol.
pub struct Core<C, Net, Log>
where
    C: TypeConfig,
    Net: Network<C>,
    Log: LogStorage<C>,
{
    /// This node's ID.
    pub(crate) id: NodeId,

    /// This node's runtime config.
    pub(crate) config: Arc<Config>,

    pub(crate) election_timeout: Duration,

    pub(crate) runtime_config: Arc<RuntimeConfig>,

    /// The [`Network`] implementation.
    pub(crate) network: Net,

    /// The [`LogStorage`] implementation.
    pub(crate) log_store: Log,

    pub(crate) vote: Leased<Option<Vote>, InstantOf<C>>,

    /// The last log id that is on the storage is committed.
    pub(crate) committed: Option<LogId>,

    /// Represents the Leader state.
    pub(crate) leader: LeaderState<C>,

    /// Represents the Candidate state within Openraft.
    ///
    /// A Candidate can coexist with a Leader in the system.
    /// This scenario is typically used to transition the Leader to a higher
    /// term (vote) without losing leadership status.
    pub(crate) candidate: CandidateState<C>,

    pub(crate) rx_api: MpscUnboundedReceiverOf<C, APIMessage<C>>,

    /// A Sender to send callback by other components to [`Core`], when an
    /// action is finished, such as flushing log to disk, or applying log
    /// entries to state machine.
    pub(crate) tx_notification: MpscUnboundedSenderOf<C, Notification<C>>,

    /// A Receiver to receive callback from other components.
    pub(crate) rx_notification: MpscUnboundedReceiverOf<C, Notification<C>>,

    pub(crate) tx_metrics: WatchSenderOf<C, Metrics<C>>,

    pub(crate) span: Span,
}

impl<C, Net, Log> Core<C, Net, Log>
where
    C: TypeConfig,
    Net: Network<C>,
    Log: LogStorage<C>,
{
    /// The main loop of the SuRaft protocol.
    pub(crate) async fn main(mut self) -> Result<Infallible, Fatal> {
        debug!("SuRaft node started");

        // Initialize metrics
        self.report_metrics();

        let span = tracing::span!(parent: &self.span, Level::DEBUG, "main");
        let res = self.runtime_loop().instrument(span).await;

        // Flush buffered metrics
        self.report_metrics();

        // Safe unwrap: res is Result<Infallible, _>
        let err = res.unwrap_err();
        match err {
            Fatal::Stopped => { /* Normal quit */ }
            _ => {
                error!(error = display(&err), "quit Core::main on error");
            }
        }

        debug!("update the metrics for shutdown");
        {
            let mut curr = self.tx_metrics.borrow_watched().clone();
            curr.running_state = Err(err.clone());

            let _ = self.tx_metrics.send(curr);
        }

        info!("Core shutdown complete");

        Err(err)
    }

    fn write_entry(
        &mut self,
        app_cmd: C::AppData,
        resp_tx: Option<ResponderOf<C>>,
    ) {
        let Some((lh, tx)) = self.get_leader_handler_or_reject(resp_tx) else {
            return;
        };

        lh.leader.app_cmd_buf.push(app_cmd);
        lh.leader.tx_buf.push(tx);

        if !lh.leader.io_state.is_idle() {
            debug!("write_entry: io_state is not idle, postpone");
            return;
        }

        let log_index = lh.leader.last_log_id.next_index();
        let log_id = LogId::new(lh.leader.vote.term(), log_index);

        lh.leader.last_log_id = Some(log_id.clone());

        let entry = Entry {
            log_id: log_id.clone(),
            payload: take(&mut lh.leader.app_cmd_buf),
        };

        lh.leader.io_state.submit(log_id.clone());

        let responders = take(&mut lh.leader.tx_buf);

        debug!("AppendEntry: {}", log_id);

        let notify = Notification::<C>::IOCompleted {
            log_id: log_id.clone(),
            responders,
            done: false,
        };

        let callback = IOFlushed::new(notify, self.tx_notification.downgrade());

        let mut sto = self.log_store.clone();
        let fu = async move {
            let res = sto.write_log_entry(&entry).await;
            callback.io_completed(res);
        };
        let _ = C::spawn(fu);
    }

    /// Report a metrics payload on the current state of the SuRaft node.
    pub(crate) fn report_metrics(&mut self) {
        let heartbeat = if let Some(leader) = self.leader.as_ref() {
            let clock_prog = &leader.clock_progress;
            let heartbeat = Some(
                clock_prog
                    .iter()
                    .map(|(id, opt_t)| {
                        (id.clone(), opt_t.map(SerdeInstant::new))
                    })
                    .collect(),
            );

            heartbeat
        } else {
            None
        };

        let last_quorum_acked = self.last_quorum_acked_time();

        let server_state = if self.leader.is_some() {
            ServerState::Leader
        } else if self.candidate.is_some() {
            ServerState::Candidate
        } else {
            ServerState::Follower
        };

        let current_leader = self.current_leader();

        let committed = if let Some(l) = &self.leader {
            l.io_state.flushed().copied()
        } else {
            self.committed
        };

        let m = Metrics {
            running_state: Ok(()),
            id: self.id.clone(),

            vote: self.vote.deref().clone(),
            committed,

            server_state,
            current_leader: current_leader.clone(),
            last_quorum_acked: last_quorum_acked.map(SerdeInstant::new),
            heartbeat: heartbeat.clone(),
        };

        debug!("id={} report_metrics: {}", self.id, m);
        let res = self.tx_metrics.send(m);

        if let Err(err) = res {
            error!(error=%err, id=display(&self.id), "error reporting metrics");
        }
    }

    #[tracing::instrument(level = "debug", skip(self))]
    pub(crate) fn current_leader(&self) -> Option<NodeId> {
        debug!(
            "id={} get current_leader: local vote:{}",
            self.id,
            self.vote.deref().display()
        );

        let vote = self.vote.deref().as_ref();

        if !vote.map(|x| x.is_committed()).unwrap_or_default() {
            return None;
        }

        vote.map(|x| x.voted_for().clone())
    }

    /// Retrieves the most recent timestamp that is acknowledged by a quorum.
    ///
    /// This function returns the latest known time at which the leader received
    /// acknowledgment from a quorum of followers, indicating its leadership
    /// is current and recognized. If the node is not a leader or no
    /// acknowledgment has been received, `None` is returned.
    fn last_quorum_acked_time(&mut self) -> Option<InstantOf<C>> {
        let leading = self.leader.as_mut();
        leading.and_then(|l| l.last_quorum_acked_time())
    }

    /// Run an event handling loop
    ///
    /// It always returns a [`Fatal`] error upon returning.
    #[tracing::instrument(level = "debug", skip_all, fields(id=display(&self.id)))]
    async fn runtime_loop(&mut self) -> Result<Infallible, Fatal> {
        // Ratio control the ratio of number of APIMessage to process to number
        // of Notification to process.
        let mut balancer = Balancer::new(10_000);

        loop {
            self.report_metrics();

            debug!("id={} runtime_loop: wait for next event", self.id);

            futures::select_biased! {
                notify_res = self.rx_notification.recv().fuse() => {
                    match notify_res {
                        Some(notify) => self.handle_notification(notify).await?,
                        None => {
                            tracing::error!("all rx_notify senders are dropped");
                            return Err(Fatal::Stopped);
                        }
                    };
                }

                msg_res = self.rx_api.recv().fuse() => {
                    match msg_res {
                        Some(msg) => self.handle_api_msg(msg).await?,
                        None => {
                            tracing::info!("all rx_api senders are dropped");
                            return Err(Fatal::Stopped);
                        }
                    };
                }
            }

            // There is a message waking up the loop, process channels one by
            // one.

            let api_messages_processed =
                self.process_raft_msg(balancer.api_message()).await?;
            let notify_processed =
                self.process_notification(balancer.notification()).await?;

            // If one of the channel consumed all its budget, re-balance the
            // budget ratio.

            #[allow(clippy::collapsible_else_if)]
            if notify_processed == balancer.notification() {
                info!("there may be more Notification to process, increase Notification ratio");
                balancer.increase_notification();
            } else {
                if api_messages_processed == balancer.api_message() {
                    info!("there may be more APIMessage to process, increase APIMessage ratio");
                    balancer.increase_api_message();
                }
            }
        }
    }

    /// Process APIMessage as many as possible.
    ///
    /// It returns the number of processed message.
    /// If the input channel is closed, it returns `Fatal::Stopped`.
    async fn process_raft_msg(&mut self, at_most: u64) -> Result<u64, Fatal> {
        for i in 0..at_most {
            let res = self.rx_api.try_recv();
            let msg = match res {
                Ok(msg) => msg,
                Err(e) => match e {
                    TryRecvError::Empty => {
                        return Ok(i + 1);
                    }
                    TryRecvError::Disconnected => {
                        info!("id={} rx_api is disconnected, quit", self.id);
                        return Err(Fatal::Stopped);
                    }
                },
            };

            self.handle_api_msg(msg).await?;
        }

        debug!(
            "id={} at_most({}) reached, there are more queued APIMessage to process",
            self.id,
            at_most
        );

        Ok(at_most)
    }

    /// Process Notification as many as possible.
    ///
    /// It returns the number of processed notifications.
    /// If the input channel is closed, it returns `Fatal::Stopped`.
    async fn process_notification(
        &mut self,
        at_most: u64,
    ) -> Result<u64, Fatal> {
        for i in 0..at_most {
            let res = self.rx_notification.try_recv();
            let notify = match res {
                Ok(msg) => msg,
                Err(e) => match e {
                    TryRecvError::Empty => {
                        return Ok(i + 1);
                    }
                    TryRecvError::Disconnected => {
                        error!("rx_notify is disconnected, quit");
                        return Err(Fatal::Stopped);
                    }
                },
            };

            self.handle_notification(notify).await?;
        }

        debug!(
            "at_most({}) reached, there are more queued Notification to process",
            at_most
        );

        Ok(at_most)
    }

    /// Spawn parallel vote requests to all cluster members.
    async fn broadcast_request_vote(&mut self, req: &RequestVote) {
        let candidate = self.candidate.as_ref().unwrap();
        let membership = candidate.membership();

        let vote = req.vote.clone();

        for (target, target_node) in membership.nodes() {
            if target == &self.id {
                let reply = VoteReply::new(Some(vote.clone()), None, true);
                let notification = Notification::RequestVoteReply {
                    target: self.id.clone(),
                    reply,
                    sender_vote: vote.clone(),
                };
                let _ = self.tx_notification.send(notification);
                continue;
            }

            let mut client =
                self.network.new_connection(target.clone(), target_node).await;

            let req = req.clone();
            let sender_vote = vote.clone();
            let target = target.clone();
            let ttl = Duration::from_millis(self.config.election_timeout_min);
            let tx = self.tx_notification.clone();

            let fu = async move {
                let res = C::timeout(ttl, client.request_vote(req)).await;
                let reply = res.map_err(to_any_error)?.map_err(to_any_error)?;

                let notification = Notification::RequestVoteReply {
                    target,
                    reply,
                    sender_vote,
                };

                let _ = tx.send(notification);
                Ok::<(), AnyError>(())
            };

            // False positive lint warning(`non-binding `let` on a future`): https://github.com/rust-lang/rust-clippy/issues/9932
            #[allow(clippy::let_underscore_future)]
            let _ = C::spawn(fu);
        }
    }

    #[tracing::instrument(level = "debug", skip(self, msg), fields(id=display(&self.id)))]
    pub(crate) async fn handle_api_msg(
        &mut self,
        msg: APIMessage<C>,
    ) -> Result<(), io::Error> {
        debug!("RAFT_event id={:<2}  input: {}", self.id, msg);

        match msg {
            APIMessage::RequestVote { rpc, tx } => {
                let now = C::now();
                info!(
                    now = display(now.display()),
                    vote_request = display(&rpc),
                    "received APIMessage::RequestVote: {}",
                    func_name!()
                );

                let resp = self.handle_request_vote(rpc);
                let _ = tx.send(resp);
            }
            APIMessage::Write { app_data, tx } => {
                self.write_entry(app_data, Some(tx));
            }
            APIMessage::ChangeMembership { membership, tx } => {
                info!(
                    members = debug(&membership),
                    "received APIMessage::ChangeMembership: {}",
                    func_name!()
                );

                if let Ok(lh) = self.leader_handler() {
                    lh.leader.membership = membership.clone();
                }

                let cb = ClientCallback::<C, _>::new(
                    tx,
                    self.tx_notification.downgrade(),
                );

                let mut sto = self.log_store.clone();

                let _ = C::spawn(async move {
                    let res = sto.write_membership(&membership).await;
                    cb.io_completed(res);
                });
            }
            APIMessage::Elect => {
                if self.leader_handler().is_ok() {
                    info!("ExternalCommand: already a Leader");
                } else {
                    self.elect().await?;
                }
            }
            APIMessage::BroadcastHeartbeat => {
                if let Ok(mut lh) = self.leader_handler() {
                    lh.broadcast_heartbeat().await;
                }
            }
        };

        Ok(())
    }

    pub(crate) async fn handle_notification(
        &mut self,
        notify: Notification<C>,
    ) -> Result<(), Fatal> {
        debug!("RAFT_event id={:<2} notify: {}", self.id, notify);

        match notify {
            Notification::RequestVoteReply {
                target,
                reply,
                sender_vote,
            } => {
                self.handle_vote_reply(target, reply, sender_vote).await;
            }

            Notification::Tick { i } => {
                let now = C::now();
                debug!("received tick: {}, now: {}", i, now.display());

                self.tick_election(now).await?;
                self.tick_heartbeat(now).await;
            }

            Notification::StorageError { error } => {
                error!("Core received Notification::StorageError: {}", error);
                return Err(Fatal::from(error));
            }

            Notification::IOCompleted {
                log_id,
                responders,
                done,
            } => {
                if done {
                    self.update_committed(Some(log_id));

                    if let Ok(lh) = self.leader_handler() {
                        debug!("IOCompleted: flush IO: {}", log_id);
                        lh.leader.io_state.flush(log_id);
                    } else {
                        info!(
                            "IOCompleted: but no longer a Leader, ignore: {}",
                            log_id
                        );
                    };

                    for responder in responders {
                        if let Some(r) = responder {
                            let _ = r.send(Ok(log_id));
                        }
                    }
                } else {
                    // TODO:
                    // IO conflict, quit leader
                }
            }

            Notification::HeartbeatReply {
                leader_vote,
                sending_time,
                target,
            } => {
                if let Ok(mut lh) = self.leader_handler() {
                    if leader_vote == lh.leader.vote {
                        lh.update_leader_clock(target, sending_time);
                    }
                }
            }
        };
        Ok(())
    }

    async fn elect(&mut self) -> Result<(), io::Error> {
        let candidate = self.new_candidate().await?;

        let new_vote = candidate.vote_ref().clone();

        info!("{}, new candidate: {}", func_name!(), candidate);

        let last_log_id = self.get_committed();

        let election_timeout = self.election_timeout;

        // Simulate sending RequestVote RPC to local node.
        // Safe unwrap(): it won't reject itself ˙–˙
        self.vote_handler().update_vote(&new_vote, election_timeout).unwrap();

        let request_vote =
            RequestVote::new(new_vote, last_log_id, Duration::default());

        self.broadcast_request_vote(&request_vote).await;

        Ok(())
    }

    pub(crate) async fn new_candidate(
        &mut self,
    ) -> Result<&mut Candidate<C>, io::Error> {
        let now = C::now();

        let last_log_id = self.refresh_committed().await?;

        let new_vote = {
            let log_term_next = last_log_id.next_term();
            let local_term_next = self.vote.next_term();

            let new_term = std::cmp::max(log_term_next, local_term_next);
            let new_vote = Vote::new(new_term, self.id.clone());
            new_vote
        };

        let membership = self.log_store.read_membership().await?;

        let Some(membership) = membership else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Membership is not found",
            ));
        };

        self.candidate =
            Some(Candidate::new(now, new_vote, last_log_id, membership));

        Ok(self.candidate.as_mut().unwrap())
    }

    async fn tick_heartbeat(&mut self, now: InstantOf<C>) {
        if !self.runtime_config.enable_heartbeat.load(Ordering::Relaxed) {
            return;
        }

        let Ok(mut lh) = self.leader_handler() else {
            return;
        };

        if now < lh.leader.next_heartbeat {
            return;
        }

        lh.broadcast_heartbeat().await;
    }

    async fn tick_election(
        &mut self,
        now: InstantOf<C>,
    ) -> Result<(), io::Error> {
        // TODO: leader lease should be extended. Or it has to examine if it is
        // leader       before electing.
        if self.leader.is_some() {
            debug!(
                "id={} already a leader, do not elect again in this tick",
                self.id
            );
            return Ok(());
        }

        if !self.runtime_config.enable_elect.load(Ordering::Relaxed) {
            debug!("id={} election is disabled", self.id);
            return Ok(());
        }

        let local_vote = &self.vote;
        let election_timeout = self.election_timeout;

        debug!(
            "id={} check if to elect: local vote: {}, election_timeout: {:?}",
            self.id,
            local_vote.display(),
            election_timeout,
        );

        if local_vote.is_expired(now, election_timeout) {
            info!("id={} election timeout passed, about to elect", self.id);
        } else {
            debug!("id={} election timeout has not yet passed", self.id);
            return Ok(());
        }

        self.elect().await?;
        Ok(())
    }

    async fn establish(
        &mut self,
        candidate: Candidate<C>,
    ) -> Option<&'_ mut Leader<C>> {
        let vote = candidate.vote_ref().clone();

        debug_assert_eq!(
            vote.voted_for(),
            &self.id,
            "it can only commit its own vote"
        );

        if let Some(l) = self.leader.as_ref() {
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            if !(vote > l.committed_vote_ref().clone()) {
                warn!(
                    "vote is not greater than current existing leader vote. Do not establish new leader and quit"
                );
                return None;
            }
        }

        let vote = {
            let vote = candidate.vote_ref().clone();
            debug_assert!(!vote.is_committed());
            vote.commit()
        };

        let leader = Leader::new(
            vote.clone(),
            candidate.membership,
            candidate.last_log_id,
        );
        self.leader = Some(Box::new(leader));

        self.vote_handler().update_vote(&vote, Duration::default()).unwrap();

        self.leader.as_mut().map(|x| x.as_mut())
    }

    pub(crate) fn vote_handler(&mut self) -> VoteHandler<C> {
        VoteHandler {
            id: self.id.clone(),
            vote: &mut self.vote,
            leader: &mut self.leader,
            candidate: &mut self.candidate,
        }
    }

    /// Candidate vote is granted by a quorum, leader established.
    async fn establish_leader(&mut self) {
        info!("{}", func_name!());

        let candidate = self.candidate.take().unwrap();
        let leader = self.establish(candidate).await;

        // There may already be a Leader with higher vote
        let Some(leader) = leader else { return };

        let vote = leader.committed_vote_ref().clone();

        // Before sending any log, update the vote.
        // This could not fail because `internal_server_state` will be cleared
        // once `state.vote` is changed to a value of other node.
        let _res =
            self.vote_handler().update_vote(&vote.clone(), Duration::default());
        debug_assert!(_res.is_ok(), "commit vote can not fail but: {:?}", _res);
    }

    #[tracing::instrument(level = "debug", skip_all)]
    pub(crate) fn handle_request_vote(
        &mut self,
        req: RequestVote,
    ) -> VoteReply {
        let now = C::now();
        let local_leased_vote = &self.vote;
        let my_vote = &**local_leased_vote;

        let my_last_log_id = self.get_last_log_id();

        info!(req = display(&req), "handle_request_vote");
        info!(
            my_vote = display(my_vote.display()),
            my_last_log_id = display(my_last_log_id.display()),
            lease = display(local_leased_vote.display_lease_info(now)),
            "handle_request_vote"
        );

        if my_vote.as_ref().map(|x| x.is_committed()).unwrap_or_default() {
            // Current leader lease has not yet expired, reject voting request
            if !local_leased_vote.is_expired(now, Duration::from_millis(0)) {
                info!(
                    "reject vote-request: leader lease has not yet expire: {}",
                    local_leased_vote.display_lease_info(now)
                );

                return VoteReply::new(
                    self.vote.deref().clone(),
                    my_last_log_id,
                    false,
                );
            }
        }

        // The first step is to check log. If the candidate has less log,
        // nothing needs to be done.

        if req.last_log_id >= my_last_log_id {
            // Ok
        } else {
            info!(
                "reject vote-request: by last_log_id: !(req.last_log_id({}) >= my_last_log_id({})",
                req.last_log_id.display(),
                my_last_log_id.display(),
            );

            // Return the updated vote, this way the candidate knows which vote
            // is granted, in case the candidate's vote is changed
            // after sending the vote request.
            return VoteReply::new(
                self.vote.deref().clone(),
                my_last_log_id,
                false,
            );
        }

        // Then check vote just as it does for every incoming event.

        let res = self
            .vote_handler()
            .update_vote(&req.vote, Duration::from_millis(req.lease_ms));

        info!(
            req = display(&req),
            result = debug(&res),
            "handle vote request result"
        );

        self.update_committed(req.last_log_id);

        // Return the updated vote, this way the candidate knows which vote is
        // granted, in case the candidate's vote is changed after
        // sending the vote request.
        VoteReply::new(self.vote.deref().clone(), my_last_log_id, res.is_ok())
    }

    async fn refresh_committed(&mut self) -> Result<Option<LogId>, io::Error> {
        let last_log_entry = self.log_store.read_last_log_entry().await?;
        let last_log_id = last_log_entry.map(|x| x.log_id);

        self.update_committed(last_log_id);

        Ok(self.get_committed())
    }

    fn update_committed(&mut self, log_id: Option<LogId>) {
        self.committed = std::cmp::max(self.committed, log_id.clone());
    }

    fn get_committed(&self) -> Option<LogId> {
        self.committed
    }

    fn get_last_log_id(&self) -> Option<LogId> {
        if let Some(l) = self.leader.as_ref() {
            return l.last_log_id;
        }

        self.committed
    }

    pub(crate) async fn handle_vote_reply(
        &mut self,
        target: NodeId,
        reply: VoteReply,
        sender_vote: Vote,
    ) {
        let Some(candidate) = self.candidate_mut() else {
            // If the voting process has finished or canceled,
            // just ignore the delayed vote_resp.
            return;
        };

        if sender_vote != candidate.vote {
            return;
        }

        // If resp.vote is different, it may be a delay response to previous
        // voting.
        if reply.vote_granted
            && reply.vote.as_ref() == Some(candidate.vote_ref())
        {
            let quorum_granted = candidate.grant_by(&target);
            if quorum_granted {
                info!("a quorum granted my vote");
                self.establish_leader().await;
            }
            return;
        }

        // If not equal, vote is rejected:

        // Note that it is still possible seeing a smaller vote:
        // - The target has more logs than this node;
        // - Or leader lease on remote node is not expired;
        // - It is a delayed response of previous voting(resp.vote_granted could
        //   be true)
        // In any case, no need to proceed.

        // Update if resp.vote is greater.
        if let Some(vote) = reply.vote.as_ref() {
            let _ = self.vote_handler().update_vote(vote, Duration::default());
        }
    }

    /// Get a LeaderHandler for handling leader's operation. If it is not a
    /// leader, it sends back a ForwardToLeader error through the tx.
    ///
    /// If tx is None, no response will be sent.
    pub(crate) fn get_leader_handler_or_reject(
        &mut self,
        tx: Option<ResponderOf<C>>,
    ) -> Option<(LeaderHandler<C, Net>, Option<ResponderOf<C>>)> {
        let res = self.leader_handler();
        let forward_err = match res {
            Ok(lh) => {
                debug!("this node is a leader");
                return Some((lh, tx));
            }
            Err(forward_err) => forward_err,
        };

        if let Some(tx) = tx {
            tx.send(Err(forward_err.into()));
        }

        None
    }

    /// Build a ForwardToLeader error that contains the leader id
    pub(crate) fn forward_to_leader(&self) -> ForwardToLeader {
        let vote = self.vote.deref();

        if vote.is_committed() {
            // Safe unwrap(): vote that is committed has to already have voted
            // for some node.
            let id = vote.as_ref().unwrap().voted_for().clone();

            return ForwardToLeader::new(id);
        }

        ForwardToLeader::empty()
    }

    pub(crate) fn leader_handler(
        &mut self,
    ) -> Result<LeaderHandler<C, Net>, ForwardToLeader> {
        if self.leader.is_none() {
            return Err(self.forward_to_leader());
        }

        let leader = self.leader.as_mut().unwrap();

        Ok(LeaderHandler {
            config: self.config.as_ref(),
            network: &mut self.network,
            tx_notification: self.tx_notification.clone(),
            leader,
        })
    }

    pub(crate) fn candidate_mut(&mut self) -> Option<&mut Candidate<C>> {
        self.candidate.as_mut()
    }
}
