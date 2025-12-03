use std::io::Cursor;

// The Node definition of raft
use bytes::{Buf, BufMut, Bytes, BytesMut};
// use tokio::io::{AsyncReadExt, AsyncWriteExt};
// use tokio::net::{TcpListener, TcpStream};

use crate::state::NodeState;
use crate::config::Config;
use crate::transport::Transport;

pub type NodeId = u16;

// pub struct RaftNode<S, SM, T>
// where
//     T: Transport
// {
//     pub id: NodeId,
//     pub config: Config,
    
//     pub peers: Vec<NodeId>,

//     pub logs: Vec<LogEntry>,

//     pub commit_idx: u64,
//     pub last_applied_idx: u64,
//     pub last_snapshot_idx: u64,

//     pub current_term: u64,

//     pub state_data: NodeState,

//     pub storage: S,
//     pub state_machine: SM,
//     pub transport: T,
// }

// impl RaftNode<S, SM, T> {
//     pub fn new(
//         id: NodeId,
//         config: Config,
//         peers: Vec<NodeId>,
//         state_data: NodeState,
//         S: Storage,
//     ) -> Self {
//         Self {
//             id,
//             config,
//             peers,
//             logs: vec![],
//             commit_idx: 0,
//             last_applied_idx: 0,
//             last_snapshot_idx: 0,
//             current_term: 0,
//             state_data,
//         }
//     }

//     pub async fn start_node(&mut self) {
//         loop {
//             let state = &self.state_data;
//             match state {
//                 NodeState::Leader(_) => {
//                     self.run_leader().await;
//                 }
//                 NodeState::Follower(_) => {
//                     self.run_follower().await;
//                 }
//                 NodeState::Candidate(_) => {
//                     self.run_candidate().await;
//                 }
//             }
//         }
//     }

//     pub async fn run_leader(&mut self) {
//         // What is required for a leader?
//         // Timers
//         //  Heartbeats -> Send heartbeats to followers and commit data
//         //  Send AppendEntries requests to followers to sync state
//         let election_timeout = self.config.election_timeout.to_std().unwrap();
//         let mut election_timer = tokio::time::interval(election_timeout);
//     }

//     pub async fn run_follower(&mut self) {
//         // Receive AppendEntries requests
//         // Receive Election requests
//     }

//     pub async fn run_candidate(&mut self) {
//         // Receive Election requests
//         // Send Election responses to followers
//         // If election is won, becomes leader and returns from this function to use run_leader
//         // Else, become follower and returns from this function to use run_follower
//     }
// }

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



pub struct LogEntry {
    pub term: u64,
    pub index: u64,
    pub cmd: Bytes,
}

impl LogEntry {
    pub fn encode_to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::new();
        buf.put_u64(self.index);
        buf.put_u64(self.term);
        buf.put_slice(&self.cmd);
        buf.freeze()
    }

    pub fn decode(buf: &[u8]) -> Self {
        let mut cursor = Cursor::new(buf);
        let index = cursor.get_u64();
        let term = cursor.get_u64();
        let cmd = Bytes::copy_from_slice(&buf[cursor.position() as usize..]);

        Self {
            term,
            index,
            cmd
        }
    }
}