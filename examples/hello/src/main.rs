mod logging;

use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use maplit::btreemap;
use memstore::MemLogStore;
use pseudonet::DirectNetwork;
use suraft::declare_suraft_types;
use suraft::errors::ForwardToLeader;
use suraft::storage::log::entry::Entry;
use suraft::storage::log::log_id::LogId;
use suraft::storage::log::log_id::LogIdOptionExt;
use suraft::storage::LogStorageExt;
use suraft::type_config::TypeConfigExt;
use suraft::Node;
use suraft::SuRaft;

use crate::logging::init_logging;

#[derive(Debug)]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Cmd {
    pub key: String,
    pub value: Vec<u8>,
}

impl Cmd {
    pub fn new(key: impl ToString, value: impl ToString) -> Self {
        Self {
            key: key.to_string(),
            value: value.to_string().as_bytes().to_vec(),
        }
    }
}

declare_suraft_types!(pub Types: AppData = Cmd);

fn nid(id: impl ToString) -> suraft::NodeId {
    id.to_string()
}

#[derive(Default)]
struct StateMachine {
    applied: Option<LogId>,
    data: BTreeMap<String, Vec<u8>>,
}

impl StateMachine {
    fn apply(&mut self, cmd: Cmd) {
        self.data.insert(cmd.key.clone(), cmd.value.clone());
        println!(
            "StateMachine: apply: {:?}; data after applying: {:?}",
            cmd, self.data
        );
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
                println!(
                    "StateMachine: found new committed: {:?}, sm.last_log_id: {:?}",
                    committed, self.applied
                );

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let g = init_logging("su", "_log", "DEBUG");
    Box::leak(Box::new(g));

    let config = Arc::new(suraft::Config::default());
    let network = DirectNetwork::<Types>::default();
    let mut log = MemLogStore::default();

    let nodes = btreemap! {nid(1) =>Node::new(""),nid(2) =>Node::new(""),nid(3) =>Node::new("") };

    SuRaft::<Types>::initialize(&mut log, nodes).await?;

    let su1 = SuRaft::new(nid(1), config.clone(), network.clone(), log.clone());
    let su2 = SuRaft::new(nid(2), config.clone(), network.clone(), log.clone());
    let su3 = SuRaft::new(nid(3), config.clone(), network.clone(), log.clone());

    // Add routes to the network
    network.add_peer(nid(1), su1.clone());
    network.add_peer(nid(2), su2.clone());
    network.add_peer(nid(3), su3.clone());

    // Attach a state machines to node-1.
    // No state machines are attached to node 2 or 3.
    let sm1 = StateMachine::default();
    Types::spawn(sm1.run(su1.clone(), log.clone()));

    let write_res = su1.write(Cmd::new("x", 1)).await?;
    println!(
        "write to arbitrary node, there may not be a leader: {:?}",
        write_res
    );

    Types::sleep(Duration::from_millis(1_000)).await;
    println!("sleeping for 1_000ms to wait for leader election");

    let write_res = su1.write(Cmd::new("x", 1)).await?;
    println!(
        "write to arbitrary node, if it's not leader, \
        it should inform to forward to a leader: {:?}",
        write_res
    );

    // Find the leader
    let leader = if let Err(ForwardToLeader {
        leader_id: Some(leader_id),
    }) = write_res
    {
        network.get_peer(&leader_id).unwrap()
    } else {
        su1.clone()
    };

    let write_res = leader.write(Cmd::new("x", "1")).await?;
    println!("write to leader: {:?}", write_res);

    let write_res = leader.write(Cmd::new("y", "2")).await?;
    println!("write to leader: {:?}", write_res);

    Types::sleep(Duration::from_millis(500)).await;

    Ok(())
}
