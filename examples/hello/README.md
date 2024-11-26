# Tutorial: Building a Simple 3-Node Cluster with suRaft

This tutorial will guide you step by step through implementing a simple 3-node cluster using **suRaft**. In this cluster, we'll handle write requests using shared storage as the backend and provide a state machine implementation on node 1 to apply committed log entries.

We'll use sample code to explain each part that needs to be implemented when using suRaft, helping you better understand how to use suRaft effectively.

## Overview of the Sample Code

The sample code demonstrates how to:

- Define the application's data structure `Cmd`.
- Declare suRaft application types.
- Implement an independent state machine `StateMachine`.
- Initialize the suRaft cluster.
- Perform write operations and handle leader forwarding logic.

Let's break down the code section by section.

---

## Defining the Application Command Data Structure `Cmd`

First, we define a simple command structure `Cmd`, which contains a key-value pair. The `key` is a string, and the `value` is a byte array. This `Cmd` will be stored as part of the suRaft log in shared storage and eventually applied to the state machine.

```rust
pub struct Cmd {
    pub key: String,
    pub value: Vec<u8>,
}
```

Then, we use the `declare_suraft_types!` macro to add `Cmd` to the suRaft type definition `Types`. suRaft uses `Types` to determine the specific data types it works with, such as `SuRaft<Types>`.

```rust
declare_suraft_types!(pub Types: AppData = Cmd);
```

---

## Implementing a Simple `StateMachine`

In suRaft, the state machine isn't included directly; it can be entirely independent of the suRaft service implementation. The state machine subscribes to suRaft's metrics to detect which logs have been committed to shared storage. It then retrieves those logs from shared storage and applies them internally.

Here's the definition of our simple `StateMachine`:

```rust
struct StateMachine {
    applied: Option<LogId>,
    data: BTreeMap<String, Vec<u8>>,
}
```

- `applied`: Records the ID of the last applied log entry.
- `data`: Stores the applied key-value data as a map.

---

## Implementing the State Machine's Apply Logic and Run Method

The implementation of the state machine is straightforward:

- The `apply` method inserts a `Cmd`'s key-value pair into its internal `data` map.
- The `run` method monitors changes in suRaft's committed logs. When it detects new committed logs, it reads them from the log store, applies each command, and updates the `applied` log ID.

```rust
impl StateMachine {
    fn apply(&mut self, cmd: Cmd) {
        self.data.insert(cmd.key.clone(), cmd.value.clone());
    }

    async fn run(
        mut self,
        su: SuRaft<Types>,
        mut log_store: MemLogStore,
    ) -> Result<(), io::Error> {
        let mut watcher = su.metrics();

        loop {
            let committed = watcher.borrow().committed;

            if committed.is_none() {
                // No committed logs yet, continue monitoring.
                continue;
            }

            while self.applied < committed {
                // Process all new committed logs.
                let next = self.applied.next_index();
                let entry: Entry<Types> = log_store.read_log_entry(next).await?.unwrap();

                for cmd in entry.payload {
                    self.apply(cmd);
                }
                self.applied = Some(entry.log_id);
            }

            // Wait for metrics to change before checking again.
            if watcher.changed().await.is_err() {
                break;
            }
        }
        Ok(())
    }
}
```

---

## The Main Function

In the `main` function, we:

- Declare a default `Config`.
- Create a memory-based `MemLogStore`, simulating S3 shared storage.
- Set up a simulated network `DirectNetwork`.

The `MemLogStore` provides three simple interfaces: `read`, `write`, and `list`. It's the only persistent part of the suRaft system, storing log entries and membership configurations. Election information like `term` is not stored here.

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(suRaft::Config::default());
    let network = DirectNetwork::<Types>::default();
    let mut log = MemLogStore::default();
```

---

## Initializing the Log Store

We need to perform a one-time initialization of the cluster to specify the nodes in the cluster. This writes the membership configuration to the shared storage. With suRaft, since there's only one copy of the log in shared storage, we don't have split-brain issues or need two-phase membership changes.

```rust
    let nodes = btreemap! {
        nid(1) => Node::new(""),
        nid(2) => Node::new(""),
        nid(3) => Node::new("")
    };

    SuRaft::<Types>::initialize(&mut log, nodes).await?;
```

---

## Creating suRaft Instances for Each Node

Next, we create a `SuRaft` instance for each node by specifying:

- **Node ID**: Unique identifier for each node.
- **Configuration (`config`)**: Shared configuration settings.
- **Network (`network`)**: Shared network for node communication.
- **Log Store (`log`)**: Shared log storage.

These resources are shared among all nodes.

```rust
    let su1 = SuRaft::new(nid(1), config.clone(), network.clone(), log.clone());
    let su2 = SuRaft::new(nid(2), config.clone(), network.clone(), log.clone());
    let su3 = SuRaft::new(nid(3), config.clone(), network.clone(), log.clone());
```

---

## Attaching the State Machine to Node 1

We demonstrate how to implement a state machine, but it's important to note that suRaft's cluster operation doesn't require a state machine. Applications can implement their own state machines as needed.

In this example, we attach a state machine only to node 1; the other nodes do not have one. Since the state machine operates independently of suRaft's consensus component, we spawn it as a separate task.

```rust
    let sm1 = StateMachine::default();
    Types::spawn(sm1.run(su1.clone(), log.clone()));
```

---

## Performing Write Operations and Handling Leader Forwarding Logic

Now that the suRaft nodes are running and the state machine is monitoring and applying committed logs, we can start writing data.

However, at this point:

- The cluster might not have elected a leader yet.
- Even if there is a leader, it might not be node 1.

As a result, our initial write attempts may fail. We'll show how to handle this situation and use the `ForwardToLeader` error to locate the current leader.

### Attempting to Write Through Node 1

```rust
    let write_res = su1.write(Cmd::new("x", 1)).await?;
```

We attempt to write the key-value pair `("x", 1)` via node 1. Since a leader may not have been elected, this could fail, returning:

```rust
Err(ForwardToLeader { leader_id: None })
```

This indicates there's currently no leader.

### Waiting for Leader Election

We wait for the leader election to complete:

```rust
    Types::sleep(Duration::from_millis(1_000)).await;
    let write_res = su1.write(Cmd::new("x", 1)).await?;
```

### Handling Leader Forwarding

If node 1 is not the leader, it will return:

```rust
Err(ForwardToLeader { leader_id: Some("2") })
```

This tells us to forward the request to the leader (node 2 in this case). If no error is returned, node 1 is the leader.

### Writing to the Leader

We can then get the leader node and attempt the write again:

```rust
    // Find the leader
    let leader = if let Err(ForwardToLeader { leader_id: Some(l), }) = write_res {
        network.get_peer(&l).unwrap()
    } else {
        su1.clone()
    };

    let write_res = leader.write(Cmd::new("x", "1")).await?;
    let write_res = leader.write(Cmd::new("y", "2")).await?;
```

These write operations should succeed, and you'll see output like:

```
write to leader: Ok(LogId { term: 0, index: 0 })
write to leader: Ok(LogId { term: 0, index: 1 })
```

---

## Waiting for Log Application

Since the state machine runs in another task, we wait briefly to allow it to detect the committed changes, apply the logs, and print out the results:

```rust
    Types::sleep(Duration::from_millis(500)).await;
```

You should see output similar to:

```
StateMachine: found new committed: Some(LogId { term: 0, index: 1 }), sm.last_log_id: None
StateMachine: apply: Cmd { key: "x", value: [49] }; data after applying: {"x": [49]}
StateMachine: found new committed: Some(LogId { term: 0, index: 1 }), sm.last_log_id: Some(LogId { term: 0, index: 0 })
StateMachine: apply: Cmd { key: "y", value: [50] }; data after applying: {"x": [49], "y": [50]}
```

---

## Summary

In this tutorial, we've learned how to use suRaft to:

- **Declare the necessary application types**: Defining how suRaft interacts with your application's data.
- **Initialize and run a suRaft cluster with multiple nodes**: Setting up nodes and shared resources.
- **Attach a state machine to a node to apply committed logs**: Implementing application-specific logic.
- **Perform write operations and handle leader forwarding logic**: Interacting with the cluster and managing leadership roles.

This simple example demonstrates the basic usage of suRaft, laying the groundwork for building more complex distributed systems. You can extend this by:

- **Adding more nodes**: Scaling your cluster.
- **Implementing more sophisticated state machine logic**: Enhancing application functionality.

We hope this tutorial has given you a deeper understanding of suRaft and how you can use it to build your own distributed applications.

---

**Happy coding with suRaft!**
