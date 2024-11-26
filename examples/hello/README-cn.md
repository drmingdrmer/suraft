# 使用 suRaft 构建简单的 3 节点集群

本教程将引导你一步步使用 **suRaft** 来实现一个简单的 3 节点集群。
这个集群允许写入用 Cmd （定义的）用户数据，将其持久化到共享存储中，
并为 node-1 节点绑定一个状态机来应用这些 committed 的日志。

我们通过示例代码，详细讲解在使用 suRaft 时需要实现的各个部分，帮助你更好地了解 suRaft 的用法。

## 示例代码概览

示例代码展示了如何：

- 定义应用程序的数据结构 `Cmd`。
- 声明 suRaft 的应用类型。
- 实现一个独立于 suRaft 的简单状态机 `StateMachine`。
- 初始化 suRaft 集群。
- 进行写操作，并处理领导者转发的逻辑。

接下来，我们将逐段解析代码。

## 定义应用程序命令数据结构 `Cmd`

首先，我们定义了一个简单的命令结构 `Cmd`，它包含一个键值对，`key` 是字符串，`value` 是字节数组。这个 `Cmd` 会作为 suRaft 日志的一部分存储到共享存储中，最终应用到状态机里。

```rust
pub struct Cmd {
    pub key: String,
    pub value: Vec<u8>,
}
```

然后，使用 `declare_suraft_types!` 宏将 `Cmd` 添加到 suRaft 的类型定义 `Types` 中。suRaft 会从 `Types` 中获取每种数据对应的类型，例如 `SuRaft<Types>`。

```rust
declare_suraft_types!(pub Types: AppData = Cmd);
```

## 实现简单的状态机 `StateMachine`

在 suRaft 中，状态机并不是直接包含在内的，它可以完全独立于 suRaft 的服务实现。状态机通过订阅 suRaft 的 metrics 来获知哪些日志已经提交到共享存储，然后从共享存储中获取日志，应用到状态机内部。

```rust
struct StateMachine {
    applied: Option<LogId>,
    data: BTreeMap<String, Vec<u8>>,
}
```

- `applied` 记录了最后一次应用的日志 ID。
- `data` 存储了已应用的键值数据。

## 实现状态机的应用逻辑和运行方法

状态机的实现非常直接。`apply` 方法将一个 `Cmd` 的键值对记录到自己的 `data` 中。`run` 函数负责监视 suRaft 的已提交日志的变化，如果发现新的已提交日志，就读取日志并应用，同时记录最后应用的日志 ID。

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
            let committed = watcher.borrow().committed.clone();

            if committed.is_none() {
                continue;
            }

            while self.applied < committed {
                let next = self.applied.next_index();
                let entry: Entry<Types> =
                    log_store.read_log_entry(next).await?.unwrap();

                for cmd in entry.payload {
                    self.apply(cmd);
                }
                self.applied = Some(entry.log_id);
            }

            let Ok(_x) = watcher.changed().await else {
                break;
            };
        }
        Ok(())
    }
}
```

## 主函数

在主函数中，我们首先声明了一个默认的 `Config`，一个纯内存实现的 `MemLogStore`（用于模拟 S3），以及一个模拟的网络 `DirectNetwork`。`MemLogStore` 提供 3 个简单的接口：`read`、`write` 和 `list`。这是 suRaft 系统中唯一持久化的部分，它存储日志条目和成员配置，但选举信息如 `term` 等并不存储在这里。

```rust
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(suRaft::Config::default());
    let network = DirectNetwork::<Types>::default();
    let mut log = MemLogStore::default();
```

## 初始化日志存储

suRaft 组成的集群需要做一次性初始化，用于指定集群中的节点。这一步实际上是向共享存储写入一条成员配置。由于 suRaft 中只有一个日志副本在共享存储中，所以不会出现脑裂问题，也不需要两阶段的成员变更。

```rust
    let nodes = btreemap! {
        nid(1) => Node::new(""),
        nid(2) => Node::new(""),
        nid(3) => Node::new("")
    };

    SuRaft::<Types>::initialize(&mut log, nodes).await?;
```

## 为每个节点创建 suRaft 实例

接下来，为每个节点创建一个 `SuRaft` 实例，指定节点 ID、配置、网络和日志存储。`config`、`network` 和 `log` 都是共享的。

```rust
    let su1 = SuRaft::new(nid(1), config.clone(), network.clone(), log.clone());
    let su2 = SuRaft::new(nid(2), config.clone(), network.clone(), log.clone());
    let su3 = SuRaft::new(nid(3), config.clone(), network.clone(), log.clone());
```

## 将状态机附加到节点 1

这里我们演示如何实现状态机。suRaft 的集群运行并不需要状态机的参与，应用程序可以根据需要以自己的方式实现状态机。在这个例子中，我们只为 suRaft 集群的节点 1 提供了状态机，另外两个节点没有。

由于状态机本身与 suRaft 的共识部分是并行且无依赖的，我们将它 `spawn` 到另一个任务中独立执行。

```rust
    let sm1 = StateMachine::default();
    Types::spawn(sm1.run(su1.clone(), log.clone()));
```

## 进行写操作，并处理领导者转发逻辑

现在，suRaft 的 3 个节点已经在运行，状态机也在单独的任务中监视并及时应用已提交的日志。我们可以开始尝试写入数据。然而，此时可能集群还没选出领导者，而且即使选出了领导者，也可能不是我们选择进行写入的节点 1。所以，最初的几次写入可能会失败。我们展示了这种可能性，并说明如何利用返回的 `ForwardToLeader` 错误来定位当前的领导者。

```rust
    let write_res = su1.write(Cmd::new("x", 1)).await?;
```

尝试通过节点 1 进行写操作，写入键值对 `("x", 1)`。由于此时可能还未选出领导者，因此会失败，返回的错误是 `Err(ForwardToLeader { leader_id: None })`，表示现在没有领导者。

接着，我们等待一段时间，等待领导者选举完成，再次尝试写入：

```rust
    Types::sleep(Duration::from_millis(1_000)).await;
    let write_res = su1.write(Cmd::new("x", 1)).await?;
```

如果节点 1 不是领导者，它会返回 `ForwardToLeader` 错误，指示将请求转发给领导者，例如 `Err(ForwardToLeader { leader_id: Some("2") })`。现在，我们可以找到领导者并进行写操作。如果没有返回错误，那么节点 1 就是领导者。根据返回的领导者 ID，我们获取领导者节点，再次尝试写入，就会成功：

```rust
    // 找到领导者
    let leader = if let Err(ForwardToLeader { leader_id: Some(l), }) = write_res
    {
        network.get_peer(&l).unwrap()
    } else {
        su1.clone()
    };

    let write_res = leader.write(Cmd::new("x", "1")).await?;
    let write_res = leader.write(Cmd::new("y", "2")).await?;
```

最后会打印出写入了两条日志的信息：

```
write to leader: Ok(LogId { term: 0, index: 0 })
write to leader: Ok(LogId { term: 0, index: 1 })
```

## 等待日志应用

由于状态机在另一个任务中运行，我们等待一段时间，让状态机监控到已提交的变化，应用日志并打印出信息：

```rust
    Types::sleep(Duration::from_millis(500)).await;
```

这时会显示：

```
StateMachine: found new committed: Some(LogId { term: 0, index: 1 }), sm.last_log_id: None
StateMachine: apply: Cmd { key: "x", value: [49] }; data after applying: {"x": [49]}
StateMachine: found new committed: Some(LogId { term: 0, index: 1 }), sm.last_log_id: Some(LogId { term: 0, index: 0 })
StateMachine: apply: Cmd { key: "y", value: [50] }; data after applying: {"x": [49], "y": [50]}
```

## 总结

通过本教程，我们学习了如何使用 suRaft：

- 声明 suRaft 所需的类型。
- 创建节点并启动 suRaft 实例。
- 将状态机附加到节点以应用日志。
- 进行写操作，并处理领导者转发的逻辑。

这个简单的示例展示了 suRaft 的基本用法，为构建更复杂的分布式系统奠定了基础。你可以在此基础上扩展，实现更丰富的功能，例如添加更多节点、实现更复杂的状态机逻辑等。

希望通过这个教程，你对 suRaft 有了更深入的了解，能够运用它来构建自己的分布式应用程序。
