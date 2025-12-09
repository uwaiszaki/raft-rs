
mod types;
mod leader;
mod follower;
mod candidate;
mod common;

#[cfg(test)]
mod test_utils;

#[cfg(test)]
mod leader_tests;

#[cfg(test)]
mod follower_tests;

#[cfg(test)]
mod candidate_tests;

#[cfg(test)]
mod integration_tests;

// Re-export types for external use
pub use types::*;

use crate::{
    storage::Storage,
    state_machine::StateMachine,
    transport::Transport,
    config::Config,
    state::NodeState
};

pub struct RaftNode<S, SM, T>
where
    T: Transport,
    SM: StateMachine,
    S: Storage
{
    pub id: NodeId,
    pub config: Config,
    
    pub peers: Vec<NodeId>,

    pub logs: Vec<LogEntry>,

    pub commit_idx: u64,
    pub last_applied_idx: u64,
    pub last_snapshot_idx: u64,

    pub current_term: u64,

    pub state_data: NodeState,

    pub storage: S,
    pub state_machine: SM,
    pub transport: T,
}

impl<S, SM, T> RaftNode<S, SM, T>
where
    S: Storage,
    SM: StateMachine,
    T: Transport
{
    pub fn new(
        id: NodeId,
        config: Config,
        peers: Vec<NodeId>,
        state_data: NodeState,
        storage: S,
        state_machine: SM,
        transport : T
    ) -> Self {
        Self {
            id,
            config,
            peers,
            logs: vec![],
            commit_idx: 0,
            last_applied_idx: 0,
            last_snapshot_idx: 0,
            current_term: 0,
            state_data,
            storage,
            state_machine,
            transport
        }
    }

    pub async fn start_node(&mut self) {
        loop {
            let state = &self.state_data;
            match state {
                NodeState::Leader(_) => {
                    self.run_leader().await;
                }
                NodeState::Follower(_) => {
                    self.run_follower().await;
                }
                NodeState::Candidate(_) => {
                    self.run_candidate().await;
                }
            }
        }
    }
}

// What does a RaftNode do?
// 1) New function -> Starts the node with follower state

// 2) Randomized Election Timeout -> Converts to candidate
// 3) Election happens leader is elected
//    a) If voting times out, candidate is again converted to follower
//    b) If candidate wins election, it becomes leader
// 4) Start syncing state and sending heartbeat messages to followers
//    a) Leader sends heartbeat messages to followers to maintain leadership and followers reset their election timeout
//    b) Candidate becomes leader if no heartbeat is received and elections happen again
// 5) AppendEntries -> 
//    a) Leader sends AppendEntries RPC to followers to sync state
//    b) Followers reply with success or failure
//    c) Leader updates next_index and match_index for followers
//    d) Leader commits log entries to followers
//    e) Followers commit log entries to their own logs
// 6) CommitIndex -> 
//    a) Leader commits log entries to its own log
//    b) Leader commits log entries to followers
//    c) Followers commit log entries to their own logs
