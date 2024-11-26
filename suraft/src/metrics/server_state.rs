/// All possible states of a SuRaft node.
#[derive(Debug, Clone, Copy, Default)]
#[derive(PartialEq, Eq)]
#[derive(serde::Deserialize, serde::Serialize)]
pub enum ServerState {
    /// The node is replicating logs from the leader.
    #[default]
    Follower,
    /// The node is campaigning to become the cluster leader.
    Candidate,
    /// The node is the SuRaft cluster leader.
    Leader,
}
