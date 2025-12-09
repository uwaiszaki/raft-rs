use crate::raft_node::{NodeId, LogEntry};

#[derive(Debug)]
pub struct MessageEnvelope {
    pub to: NodeId,
    pub from: NodeId,
    pub message: RpcMessage
}

#[derive(Debug)]
pub enum RpcMessage {
    Ping(String),
    AppendEntries(AppendEntriesRequest),
    RequestVote(RequestVoteRequest),
    RequestVoteResponse(RequestVoteResponse),
    AppendEntriesResponse(AppendEntriesResponse),
}

#[derive(Debug)]
pub struct AppendEntriesRequest {
    pub term: u64,
    pub leader_id: NodeId,
    pub prev_log_index: u64,
    pub prev_log_term: u64,
    pub entries: Vec<LogEntry>,
    pub leader_commit: u64,
}

#[derive(Debug)]
pub struct AppendEntriesResponse {
    pub term: u64,
    pub success: bool,
    pub match_index: u64,
    // TODO: For conflict resolution
    pub conflict_index: Option<u64>,
    pub conflict_term: Option<u64>,
}

#[derive(Debug)]
pub struct RequestVoteRequest {
    pub term: u64,
    pub candidate_id: NodeId,
    pub last_log_index: u64,
    pub last_log_term: u64,
}

#[derive(Debug)]
pub struct RequestVoteResponse {
    pub term: u64,
    pub vote_granted: bool,
}