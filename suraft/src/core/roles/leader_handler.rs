use std::time::Duration;

use tracing::debug;

use crate::async_runtime::MpscUnboundedSender;
use crate::base::display_ext::display_option::DisplayOptionExt;
use crate::base::display_ext::DisplayInstantExt;
use crate::core::io::notification::Notification;
use crate::core::roles::leader::Leader;
use crate::network::connection::Connection;
use crate::quorum::progress::Progress;
use crate::storage::membership::NodeId;
use crate::suraft::RequestVote;
use crate::type_config::alias::InstantOf;
use crate::type_config::alias::MpscUnboundedSenderOf;
use crate::type_config::TypeConfigExt;
use crate::Config;
use crate::Network;
use crate::Node;
use crate::TypeConfig;

/// Handle leader operations.
///
/// - Append new logs;
/// - Change membership;
/// - Heartbeat
pub(crate) struct LeaderHandler<'x, C, Net>
where
    C: TypeConfig,
    Net: Network<C>,
{
    pub(crate) config: &'x Config,
    pub(crate) network: &'x mut Net,
    pub(crate) tx_notification: MpscUnboundedSenderOf<C, Notification<C>>,
    pub(crate) leader: &'x mut Leader<C>,
}

impl<'x, C, Net> LeaderHandler<'x, C, Net>
where
    C: TypeConfig,
    Net: Network<C>,
{
    /// Update progress when replicated data(logs or snapshot) matches on
    /// follower/learner and is accepted.
    #[tracing::instrument(level = "debug", skip_all)]
    pub(crate) fn update_leader_clock(
        &mut self,
        target: NodeId,
        t: InstantOf<C>,
    ) {
        debug!(
            "id={} update leader clock: target={}; time={}",
            self.leader.vote.voted_for,
            target,
            t.display(),
        );

        let granted = *self
            .leader
            .clock_progress
            .increase_to(&target, Some(t))
            .expect("it should always update existing progress");

        debug!(
            granted = display(granted.as_ref().map(|x| x.display()).display()),
            clock_progress = display(&self.leader.clock_progress.display_with(
                |f, id, v| {
                    write!(
                        f,
                        "{}: {}",
                        id,
                        v.as_ref().map(|x| x.display()).display()
                    )
                }
            )),
            "granted leader vote clock after updating"
        );
    }

    pub(crate) async fn broadcast_heartbeat(&mut self) {
        let interval = self.config.heartbeat_interval;

        self.leader.next_heartbeat = C::now() + Duration::from_millis(interval);

        for (target, node) in self.leader.membership.clone().nodes() {
            if target == &self.leader.vote.voted_for {
                let notification = Notification::HeartbeatReply {
                    leader_vote: self.leader.vote.clone(),
                    sending_time: C::now(),
                    target: target.clone(),
                };
                let _ = self.tx_notification.send(notification);
            } else {
                self.send_heartbeat(target.clone(), node).await;
            }
        }
    }

    pub(crate) async fn send_heartbeat(&mut self, target: NodeId, node: &Node) {
        let leader_vote = self.leader.vote.clone();
        let id = leader_vote.voted_for.clone();

        let leader_lease =
            Duration::from_millis(self.config.election_timeout_max);

        let heartbeat = RequestVote::new(
            leader_vote.clone(),
            self.leader.io_state.flushed().copied(),
            leader_lease,
        );

        let notification = Notification::HeartbeatReply {
            leader_vote,
            sending_time: C::now(),
            target: target.clone(),
        };

        let timeout = Duration::from_millis(self.config.heartbeat_interval);

        let mut connection =
            self.network.new_connection(target.clone(), node).await;

        let tx = self.tx_notification.clone();

        let fu = async move {
            let res =
                C::timeout(timeout, connection.request_vote(heartbeat.clone()))
                    .await;

            debug!(
                "id={} sent a heartbeat: {}, result: {:?}",
                id, heartbeat, res
            );

            let Ok(Ok(_)) = res else {
                tracing::warn!("{} failed to send a heartbeat: {:?}", id, res);
                return;
            };
            let _ = tx.send(notification);
        };

        let _ = C::spawn(fu);
    }
}
