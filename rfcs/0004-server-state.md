# Server State

In **suRaft**, a **Server** is a process that participates in the Raft consensus protocol. All of the server's state is **volatile** and held in memory; none of it needs to be persisted to local disk or shared storage. This stateless design simplifies node management, recovery, and scaling, as servers rely on shared storage for persistent data.

The `Server` struct encapsulates the necessary state for participating in elections, handling client requests, and maintaining leadership status:

```rust
struct Server {
    id: NodeId,
    vote: Option<Vote>,
	// When the leader expire, and a new round of election should be started.
    election_deadline: Instant,
    candidate_state: Option<CandidateState>,
    leader_state: Option<LeaderState>,
}
```

- **`id`**: The unique identifier of the server, represented by a `NodeId`.

- **`vote`**: A `Vote` struct representing the current term and the candidate the server has voted for:

  ```rust
  struct Vote {
      term: u64,
      committed: bool,
      voted_for: NodeId,
  }
  ```

- **`candidate_state`**: An `Option<CandidateState>` representing the server's state when it is a **Candidate** during an election.

- **`leader_state`**: An `Option<LeaderState>` representing the server's state when it is acting as the **Leader** of the cluster. If the server is not currently the leader, this is `None`.


## The `Vote` Structure

The `vote` field in the `Server` struct contains essential information about a server's current election term and voting state:

```rust
struct Vote {
    term: u64,
    committed: bool,
    voted_for: NodeId,
}
```

- **`term`**: A monotonically increasing `u64` value representing the current election term.

- **`committed`**: A boolean indicating whether the candidate specified in `voted_for` has been granted leadership by receiving votes from a quorum of the cluster.

- **`voted_for`**: The `NodeId` of the candidate the server has voted for in the current term.

### The Partial Ordering of `Vote`

The `Vote` struct forms a **partial order**. The comparison between two votes, `a` and `b`, follows these rules:

1. **Term Comparison**:
   - If `a.term` > `b.term`, then `a` is considered **greater** than `b`.
   - If `a.term` == `b.term`, proceed to the next step.

2. **Committed Flag Comparison**:
   - If `a.committed` is `true` and `b.committed` is `false`, then `a` is **greater** than `b`.
   - If both have the same `committed` value, proceed to the next step.

3. **Voted For Comparison**:
   - If `a.voted_for` == `b.voted_for`, then `a` and `b` are considered **equal**.
   - If `a.voted_for` != `b.voted_for`, then `a` and `b` are **incomparable**.

### Significance of the Partial Order

A server updates its `vote` only to **greater** values according to the partial order. This simple rule governs all voting state changes in **suRaft**:

- Higher terms are recognized and adopted
- Committed votes take precedence over uncommitted ones in the same term
- When votes are incomparable, a new election is needed


## Candidate State

When a server becomes a **Candidate** during an election, it maintains the following state:

```rust
struct CandidateState {
    vote: Vote,
    granted_by: BTreeSet<NodeId>,
}
```

- **`vote`**: The `Vote` struct representing the candidate's current election term and the candidate it is voting for (usually itself).

- **`granted_by`**: A set of `NodeId`s of servers that have granted their vote to this candidate.

**Candidate Duties**:

- **Initiate Election**: Increment its `term`, vote for itself, and send vote requests to other servers.
- **Collect Votes**: Track votes received in `granted_by`. If it gains a majority, transition to the leader state.
- **Handle Responses**: Update its state based on responses, acknowledging higher terms or committed leaders.
- **Manage Election Timeout**: If the election times out without success, start a new election with an incremented term.

## Leader State

When a server transitions to the **Leader** state, it maintains the following state:

```rust
struct LeaderState {
    next_log_index: LogIndex,
    followers: BTreeMap<NodeId, NodeInfo>,
}
```

- **`next_log_index`**: The index of the next log entry to be written to shared storage.

- **`followers`**: A mapping of `NodeId` to `NodeInfo`, containing information about each follower in the cluster.

**Leader Duties**:

- **Handle Client Requests**: Receive and process client commands, writing new log entries to shared storage.
- **Send Heartbeats**: Periodically send heartbeats to prevent followers from starting new elections.
