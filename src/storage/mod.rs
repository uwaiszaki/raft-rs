pub mod rocksdb;

#[cfg(test)]
mod rocksdb_test;

use bincode::{Encode, Decode};
use crate::node::LogEntry;

#[derive(Encode, Decode)]
pub struct VotingState {
    pub current_term: u64,
    pub voted_for: Option<u64>
}

struct CounterState {
    commit_idx: u64,
    last_applied_idx: u64
}

trait Storage {
    fn persist_voting_state(&mut self, state: &VotingState);
    fn load_voting_state(&self) -> VotingState;

    fn persist_commit_idx(&mut self, commit_idx: u64);
    fn persist_last_applied_idx(&mut self, last_applied_idx: u64);

    fn append_log_entries(&mut self, logs: Vec<LogEntry>);
    fn load_log_entries(& self) -> Vec<LogEntry>;

    // Truncates log entries from index idx to resolve conflicts
    fn truncate_from(&mut self, idx: u64);

    fn snapshot_state(&self);
    fn recover_state(&mut self);
}