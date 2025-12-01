// The Node definition of raft
use bytes::Bytes;
use crate::state::NodeState;

pub type NodeId = u64;

pub struct NodeInfo {
    pub name: String,
    pub address: String
}

pub struct LogEntry {
    pub term: u64,
    pub index: usize,
    pub cmd: Bytes,
}

pub struct RaftNode {
    pub id: NodeId,
    pub node_info: NodeInfo,

    pub peers: Vec<NodeId>,

    pub logs: Vec<LogEntry>,

    pub commit_idx: usize,
    pub last_applied_idx: usize,
    pub last_snapshot_idx: usize,

    pub current_term: u64,

    // how long after not receiving pings from leader the follower converts to candidate
    // pub state: NodeState -> Either Follower, candidate, Leader with their own specific states
    // Follower -> Option<voted_for>>
    // leader -> next_log_index map for each follower
    // Candidate -> Votes Received
    pub state: NodeState,

    // pub S: Storage,
    // pub rpc: Rpc, 
    // 
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
