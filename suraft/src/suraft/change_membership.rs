use crate::core::io::api_message::APIMessage;
use crate::errors::Fatal;
use crate::storage::membership::Membership;
use crate::suraft::responder::OneshotResponder;
use crate::type_config::TypeConfigExt;
use crate::SuRaft;
use crate::TypeConfig;

/// Implement blocking mode write operations those reply on oneshot channel for
/// communication between SuRaft core and client.
impl<C> SuRaft<C>
where C: TypeConfig<Responder = OneshotResponder<C>>
{
    // TODO:
    /// Propose a cluster configuration change.
    ///
    /// A node in the proposed config has to be a learner, otherwise it fails
    /// with LearnerNotFound error.
    ///
    /// Internally:
    /// - It proposes a **joint** config.
    /// - When the **joint** config is committed, it proposes a uniform config.
    ///
    /// If `retain` is `true`, then all the members which not exists in the new
    /// membership, will be turned into learners, otherwise will be removed.
    /// If `retain` is `false`, the removed voter will be removed from the
    /// cluster. Existing learners will not be affected.
    ///
    /// Example of `retain` usage:
    /// If the original membership is `{"voter":{1,2,3}, "nodes":{1,2,3,4,5}}`,
    /// where `nodes` includes node information of both voters and learners.
    /// In this case, `4,5` are learners. Call `change_membership` with
    /// `voters={2,3,4}`, then:
    ///    - If `retain` is `true`, the committed new membership is
    ///     `{"voters":{2,3,4}, "nodes":{1,2,3,4,5}}`, node `1` is turned into a
    /// learner.
    ///    - Otherwise if `retain` is `false`, then the new membership is
    ///      `{"voters":{2,3,4}, "nodes":{2,3,4,5}}`, in which the removed
    ///      voters `1` are removed from the cluster. `5` is not affected.
    ///
    /// If it loses leadership or crashed before committing the second
    /// **uniform** config log, the cluster is left in the **joint** config.
    #[tracing::instrument(level = "info", skip_all)]
    pub async fn change_membership(
        &self,
        membership: Membership,
        retain: bool,
    ) -> Result<(), Fatal> {
        tracing::info!(
            changes = debug(&membership),
            retain = display(retain),
            "change_membership: start to commit joint config"
        );

        let (tx, rx) = C::oneshot();

        // res is error if membership can not be changed.
        // If no error, it will enter a joint state
        let res = self
            .inner
            .call_core(APIMessage::ChangeMembership { membership, tx }, rx)
            .await;

        if let Err(e) = &res {
            tracing::error!("the first step error: {}", e);
        }
        let res = res?;

        Ok(res)
    }
}
