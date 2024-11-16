# Protocol

This section defines server behavior and communication between servers in **suRaft**.

Upon startup, a server is in the `Follower` state with `candidate` and `leader` fields set to `None`. If the server does not receive a message from a leader before the election timeout (`election_deadline`), it transitions to the `Candidate` state.

## General Behavior

- **Receiving a Greater Vote**: If a server receives a `Vote` greater than its own via a `RequestVote` or `Heartbeat` message, it updates its local `vote`, resets `election_deadline`, sets `candidate` and `leader` to `None`, and becomes a follower.

- **Receiving an Equal or Greater Vote in Heartbeat**: If a `Heartbeat` message contains a `Vote` equal to or greater than the server's own, it extends the leader lease by updating `election_deadline`.

- **Receiving a Lower Vote**: If a message contains a lower `Vote`, the server rejects it.

- **Election Timeout**: When `election_deadline` is reached, the server starts a new election.

## Follower

- A `RequestVote` request is rejected if the server's `election_deadline` has not been reached (i.e., it still considers the current leader as active).

## Candidate

When a follower becomes a candidate:

1. **Read Last Log ID**: Retrieve the last `log_id` from shared storage.

2. **Create New Term**: Compute `new_term = last_log_id.term + 1`.

3. **Initialize Candidate State**:

   ```rust
   CandidateState {
       vote: Vote {
           term: new_term,
           committed: false,
           voted_for: self.id
       },
       granted_by: btreeset! { self.id },
   }
   ```

4. **Send `RequestVote` RPCs**: Send `RequestVote` messages to all other servers.

5. **Await Responses**: Wait for `RequestVoteReply` messages and update the `granted_by` set accordingly.

6. **Become Leader**: If a quorum (including itself) grants their votes, transition to the `Leader` state by initializing the `leader` field and clearing the `candidate` state.

Note: Unlike standard Raft, `RequestVote` does not include `last_log_id`, since all servers share the same log in shared storage.

```rust
struct RequestVote {
    vote: Vote,
}

struct RequestVoteReply {
    granted: bool,
    remote_term: u64,
}
```

## Leader

A candidate becomes a leader when it receives a quorum of votes.

Upon becoming a leader:

1. **Initialize Leader State**:
   - **`next_log_index`**: Set by reading the last log from shared storage.
   - **Cluster Configuration**: Load by reading the membership config from shared storage.

2. **Responsibilities**:
   - **Send Heartbeats**: Periodically send `Heartbeat` messages to all servers to extend the leader lease and prevent new elections.
   - **Handle Client Requests**:
     - **Commit Log Entries**: Process write requests by committing log entries.
     - **Update Membership Config**: On membership change requests, update the config in shared storage and refresh the local config.

### Heartbeat Message

A `Heartbeat` message is similar to `RequestVote`, but with `vote.committed` set to `true`.

```rust
struct Heartbeat {
    vote: Vote,
    lease: Duration, // Duration to extend `election_deadline`
}

struct HeartbeatReply {
    granted: bool,
    remote_term: u64,
}
```

### Committing Log Entries

To commit a log entry:

1. **Create Log Entry**: Form a new `LogEntry` with `server.leader.next_log_index`.

2. **Increment Index**: Increase `next_log_index`.

3. **Write to Shared Storage**:
   - **If Successful**: The log entry is committed.
   - **If Failure**: Revert to `Follower` state and initiate a new election.

### Updating Membership Configuration

Perform membership changes by updating the configuration key in shared storage.

## Safety

Since there is only one copy of the log in shared storage, successfully writing to it ensures all future leaders will see the committed entries, even during membership changes. Therefore, `term` and `voted_for` do not need to be persisted.
