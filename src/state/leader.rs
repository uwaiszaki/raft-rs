use std::collections::HashMap;
use crate::raft_node::{NodeId};

pub struct Leader {
    pub next_index: HashMap<NodeId, u64>,
    pub match_index: HashMap<NodeId, u64>,
}

impl Leader {
    pub fn new(peers: Vec<NodeId>) -> Self {
        Self {
            next_index: peers.iter().copied().map(|peer| (peer, 1)).collect(),
            match_index: peers.iter().copied().map(|peer| (peer, 0)).collect(),
        }
    }

    // pub async fn run(&mut self, node: &mut Arc<RwLock<RaftNode>>, rpc_rx: Receiver<RpcMessage>) {
    //     loop {
    //         select! {
    //             _ = tokio::time::sleep(node.config.heartbeat_interval) => {
    //                 self.send_heartbeats(node).await;
    //             }
    //         }
    //     }
    // }

    // pub async fn handle_rpc() -> Result<(), Error> {
    //     let mut node = node.write().await;
    //     let mut leader = node.state.as_mut_leader().unwrap();
    //     let mut follower = node.state.as_mut_follower().unwrap();
    //     let mut candidate = node.state.as_mut_candidate().unwrap();

    //     match node.state {
    //         NodeState::Leader(_) => {
    //             leader.handle_rpc(node).await;
    //         }
    //     }
    //     Ok(())
    // }
}