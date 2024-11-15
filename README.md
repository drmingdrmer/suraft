# suRaft: A Raft Consensus Implementation on Shared Storage


[![Discord Chat](https://img.shields.io/discord/1306128478331998298?logo=discord)](https://discord.gg/dPDDCyytkN)

**suRaft** is a distributed consensus project that adapts the Raft algorithm to work with shared storage systems like Amazon S3. By leveraging shared storage, `suRaft` simplifies distributed consensus by eliminating log replication, making it ideal for cloud environments with reliable storage services.

- [Goals and Non-Goals](rfcs/0001-goals.md)
- [Architectural Design](rfcs/0002-arch.md)
- [Data Storage](rfcs/0003-data-storage.md)


Optionally Support:

- [Multi-Column Multi-Leader](rfcs/0099-multi-leader.md)


## Roadmap

- **System Design Docs**
    - Core architecture
    - Storage data layout
    - Node state model
    - Protocol design

- **Initial Implementation**
    - Leader election
    - Cluster bootstrap
    - Basic log operations
    - Member management


- **Core Features**
    - State Machine Interface
    - Storage cleanup
    - Read path optimization

- **Production Ready**
    - Metrics and monitoring
    - Performance testing
    - Failure testing
    - Documentation

- **Extensions**
    - Storage providers
    - Client libraries
    - Config management


## Raft Protocol Adaptations

### Election Process

In suRaft, elections are conducted with modifications suited for the shared storage environment:

- **Simplified Comparison**: Nodes compare only their `term` and `voted_for` values, omitting `last_log_id` since logs are centrally stored and accessible to all nodes.
- **Granting Votes**: When a node receives a `RequestVote` request, it grants its vote to a candidate if:
  - The candidate's term is greater than the node's term.
  - The candidate's term is equal to the node's term, and the candidate's `voted_for` matches the node's `voted_for`.

#### RequestVote Structure

```rust
struct RequestVote<T> {
    term: u64,
    voted_for: T::NodeId,
}

struct RequestVoteReply<T> {
    granted: bool,
    remote_term: u64, 
}
```

### Heartbeat Mechanism

Leaders send periodic heartbeats to maintain authority and prevent new elections.

#### Heartbeat Structure

```rust
struct Heartbeat<T> {
    term: u64, 
    leader_id: T::NodeId,
}

struct HeartbeatReply {
    granted: bool,
    remote_term: u64,
}
```

## Committing Log Entries

When a leader receives a client request:

1. **Leader Verification**: The node confirms that it is the current leader.
2. **Log Entry Creation**: It attempts to add a new log entry under the `/log/` directory (e.g., `/log/00003`).
3. **Atomic Write Operation**: The log entry is written in a mutually exclusive manner to ensure that only one leader can write at a time. This atomic write capability is a required feature of the shared storage system.
4. **Commit Confirmation**:
   - **Success**: If the write succeeds, the entry is considered committed, and the leader responds to the client with success.
   - **Failure**: If the write fails (because another leader has written to that log index), the current leader acknowledges the presence of another leader and steps down to become a follower.

## Process Lifecycle and States

Nodes in suRaft transition through three states: Follower, Candidate, and Leader. Each state has specific responsibilities and behaviors.

### 1. Follower

Upon startup, a node begins as a follower.

#### Responsibilities:

- **Heartbeat Handling**:
  - If a leader's heartbeat is received, the node updates its local record of the current leader and resets its leader lease timer.
- **Leader Lease**:
  - The node maintains the current leader's ID and a lease. Before the lease expires, the node will not grant votes to other candidates.
- **RequestVote Handling**:
  - When receiving a `RequestVote` request:
    - **Leader Lease Check**:
      - If the leader lease has not expired, respond with `granted = false`.
    - **Vote Granting Logic**:
      - If the leader lease has expired, apply the following logic (which is the same for all states):

        ```rust
        if req.term > self.term 
           || (req.term == self.term && req.voted_for == self.voted_for) {
            return RequestVoteReply { granted: true, remote_term: self.term };
        } else {
            return RequestVoteReply { granted: false, remote_term: self.term };
        }
        ```

- **Election Timeout**:
  - If no heartbeat is received within a specified timeout period, the node initiates an election and transitions to the candidate state.

### 2. Candidate

A node becomes a candidate when it starts an election.

#### Responsibilities:

- **Requesting Votes**:
  - Upon entering the candidate state, the node sends `RequestVote` messages to all other nodes.
- **Vote Counting**:
  - If the node receives a majority of `granted = true` responses, it becomes the leader.
- **Handling Responses**:
  - **Heartbeat Reception**:
    - If a heartbeat is received from a valid leader, the candidate steps back to follower.
- **RequestVote Handling**:
  - Follows the same logic as in the follower state.

### 3. Leader

Once a candidate becomes a leader, it initializes and then starts serving.

#### Leader Initialization:

- **Log Indexing**:
  - The leader lists the `/log/` directory to find the maximum log index.
  - It sets the next log index to `max_index + 1`.
- **Term Verification**:
  - If an existing log entry has a term greater than the leader's term, the leader steps down to follower.

#### Leader Operation:

- **Heartbeat Broadcasting**:
  - The leader periodically sends heartbeats to other nodes to assert leadership and renew its leader lease.
- **Client Request Handling**:
  - On receiving a write request from a client:
    - The leader attempts to insert a new log entry at the next log index.
    - If successful:
      - It responds to the client with success.
      - Increments the next log index.
    - If the write fails (due to another leader writing to the log):
      - The leader steps down to become a follower.
- **Heartbeat and RequestVote Handling**:
  - **Higher Term Detection**:
    - If a heartbeat or `RequestVote` message with a higher term is received, the leader steps down to follower.
  - **RequestVote Processing**:
    - Processes `RequestVote` messages using the same logic as in the follower state.

## Summary of Node Behavior

### Leader Lease Mechanism

- **Purpose**: Prevents a node from granting votes to new candidates while a known leader is active, ensuring stability in the cluster.
- **Lease Renewal**: The leader lease is renewed upon receiving heartbeats from the current leader.
- **Expiration Handling**: Upon expiration of the leader lease, nodes become open to new election requests.

### Mutual Exclusion in Log Writes

- **Atomicity**: Only one leader can write a new log entry at a specific index due to the atomic write operation guaranteed by the shared storage system.
- **Conflict Detection**: If a write fails because an entry already exists at that log index, it indicates that another leader has written to the log.
- **Leader Demotion**: Upon detecting such a conflict, the current leader steps down to ensure consistency across the cluster.

## Conclusion

suRaft reimagines the Raft consensus algorithm by integrating shared storage solutions like Amazon S3. By centralizing log storage and eliminating the need for local state machines and log replication, suRaft offers a simplified yet robust consensus mechanism suitable for modern cloud environments. This architecture leverages the reliability and scalability of shared storage to maintain consistency across distributed nodes while reducing the complexity inherent in traditional Raft implementations.

---

**Note**: The suRaft design assumes that the shared storage system supports atomic write operations that provide mutual exclusion. This is crucial for ensuring that only one leader can commit a log entry at a given index, which is essential for maintaining consistency in the cluster.
