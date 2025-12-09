// Mock Storage implementation for testing
use std::sync::{Arc, Mutex};
use crate::storage::{Storage, VotingState};
use crate::raft_node::LogEntry;

#[derive(Clone)]
pub struct MockStorage {
    voting_state: Arc<Mutex<VotingState>>,
    logs: Arc<Mutex<Vec<LogEntry>>>,
    commit_idx: Arc<Mutex<u64>>,
    last_applied_idx: Arc<Mutex<u64>>,
}

impl MockStorage {
    pub fn new() -> Self {
        Self {
            voting_state: Arc::new(Mutex::new(VotingState {
                current_term: 0,
                voted_for: None,
            })),
            logs: Arc::new(Mutex::new(Vec::new())),
            commit_idx: Arc::new(Mutex::new(0)),
            last_applied_idx: Arc::new(Mutex::new(0)),
        }
    }

    pub fn get_voting_state(&self) -> VotingState {
        *self.voting_state.lock().unwrap()
    }

    pub fn get_logs(&self) -> Vec<LogEntry> {
        self.logs.lock().unwrap().clone()
    }

    pub fn get_commit_idx(&self) -> u64 {
        *self.commit_idx.lock().unwrap()
    }
}

impl Storage for MockStorage {
    fn persist_voting_state(&mut self, state: &VotingState) {
        *self.voting_state.lock().unwrap() = *state;
    }

    fn load_voting_state(&self) -> VotingState {
        *self.voting_state.lock().unwrap()
    }

    fn persist_commit_idx(&mut self, commit_idx: u64) {
        *self.commit_idx.lock().unwrap() = commit_idx;
    }

    fn persist_last_applied_idx(&mut self, last_applied_idx: u64) {
        *self.last_applied_idx.lock().unwrap() = last_applied_idx;
    }

    fn append_log_entries(&mut self, logs: Vec<LogEntry>) {
        let mut stored_logs = self.logs.lock().unwrap();
        stored_logs.clear();
        stored_logs.extend(logs);
    }

    fn load_log_entries(&self) -> Vec<LogEntry> {
        self.logs.lock().unwrap().clone()
    }

    fn truncate_from(&mut self, idx: u64) {
        let mut logs = self.logs.lock().unwrap();
        logs.retain(|entry| entry.index < idx);
    }

    fn snapshot_state(&self) {}

    fn recover_state(&mut self) {}
}

