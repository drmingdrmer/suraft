# suraft: A Raft Consensus Implementation on Shared Storage

## Overview

**suraft** is a distributed storage project that implements the Raft consensus algorithm by leveraging shared storage systems like Amazon S3. Unlike traditional Raft implementations, where each node maintains its own log and state machine, suraft simplifies the architecture by utilizing shared storage. This eliminates the need for log replication and local state machines. This new approach adapts the Raft protocol to cloud environments where shared storage services are readily available and reliable.

## Architectural Changes from Traditional Raft

### Removal of the State Machine

In the original Raft design, each node maintains a replicated state machine, which applies log entries to change its state. However, in suraft, the state machine is removed from the design:

- **Omission of the State Machine**: Since all logs are centrally stored and accessible, the current state can be determined directly from the logs. There's no need for each node to maintain its own state machine.
- **Separation of Business Logic**: The state machine is considered business-specific. By focusing solely on log consistency, suraft ensures that the core consensus mechanism remains general-purpose and streamlined.

### Elimination of Log Replication

Traditional Raft requires replication of logs across all nodes to ensure consistency. With shared storage, suraft makes significant changes:

- **Centralized Log Storage**: All log entries are stored directly in the shared storage (e.g., Amazon S3). This centralization means that all nodes can access the same log entries.
- **No Log Replication**: Since logs are stored in shared storage, nodes do not need to replicate logs to each other, reducing complexity and network overhead.
- **Simplified Log Management**: Operations like log truncation or reversal are unnecessary since logs are maintained in a single, consistent location.

## Data Storage Layout

An important note in suraft is that `term` and `voted_for` do not need to be persisted because logs are written to shared storage using atomic operations. This means term reversion does not lead to state reversion as it might in standard Raft.

### Components Stored in Shared Storage

The data stored in the shared storage system is primarily the log entries:

- **Log Entries**:
  - **Global Log Vector**: A single, globally accessible sequence of log entries stored in the shared storage.
  - **Log Entry Format**: Each log entry includes the term and the operation payload.

### Log Entry Storage Structure

- **Key Format**: Log entries are stored with keys following the pattern `/log/00001`, `/log/00002`, etc.
- **Value Format**: Each key stores a JSON object containing the term and payload:

  ```json
  {
      "term": 1,
      "payload": "..."
  }
  ```

## Raft Protocol Adaptations

### Election Process

In suraft, elections are conducted with modifications suited for the shared storage environment:

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

Nodes in suraft transition through three states: Follower, Candidate, and Leader. Each state has specific responsibilities and behaviors.

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

suraft reimagines the Raft consensus algorithm by integrating shared storage solutions like Amazon S3. By centralizing log storage and eliminating the need for local state machines and log replication, suraft offers a simplified yet robust consensus mechanism suitable for modern cloud environments. This architecture leverages the reliability and scalability of shared storage to maintain consistency across distributed nodes while reducing the complexity inherent in traditional Raft implementations.

---

**Note**: The suraft design assumes that the shared storage system supports atomic write operations that provide mutual exclusion. This is crucial for ensuring that only one leader can commit a log entry at a given index, which is essential for maintaining consistency in the cluster.
