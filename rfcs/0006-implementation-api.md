# Storage API

**suRaft** requires a storage interface with the following functions:

- **Write**:

  ```rust
  async fn write(path: &str, buf: &[u8], put_if_absent: bool) -> Result<bool, io::Error>
  ```

  Writes data to a specified `path`. If `put_if_absent` is `true`, the write occurs only if the path does not already exist. Returns `Ok(true)` if the write is successful and `put_if_absent` conditions are met.

- **Read**:

  ```rust
  async fn read(path: &str) -> Result<Option<Reader>, io::Error>
  ```

  Reads data from the specified `path`. Returns a `Reader` that provides access to the stored data. Returns `None` if the `path` is absent.

- **List**:

  ```rust
  async fn list(prefix: &str, start_after: &str) -> Result<Vec<String>, io::Error>
  ```

  Lists all paths in storage that start with the given `prefix`,  beginning after `start_after`.

# Network API

For communication between servers, **suRaft** defines the following network functions:

- **Request Vote**:

  ```rust
  async fn request_vote(&self, req: RequestVote) -> Result<RequestVoteReply, NetError>
  ```

  Sends a `RequestVote` RPC to another server and awaits a reply. Used by candidates during elections to request votes from peers.

- **Heartbeat**:

  ```rust
  async fn heartbeat(&self, hb: Heartbeat) -> Result<HeartbeatReply, NetError>
  ```

  Sends a `Heartbeat` message to another server and awaits a reply. Used by the leader to maintain authority and prevent other servers from initiating elections.
