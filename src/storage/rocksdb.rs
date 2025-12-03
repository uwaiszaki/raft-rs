use rocksdb::{DB, Options, WriteBatch, IteratorMode};
use bincode::{config, encode_to_vec, decode_from_slice};

use crate::storage::{Storage, VotingState};
use crate::node::{NodeId, LogEntry};


pub struct RocksDb {
    pub store: DB
}

impl RocksDb {
    fn new(node_id: NodeId) -> Self {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);

        let db = DB::open_cf(
            &options,
            format!("/var/db/rocksdb/{}", node_id),
            &["state", "logs"]
        ).unwrap();

        Self { store: db }
    }
}

impl Storage for RocksDb {
    fn persist_voting_state(&mut self, state: &VotingState) {
        let state_handle = self.store.cf_handle("state").unwrap();
        let value = encode_to_vec(state, config::standard()).unwrap();
        self.store.put_cf(&state_handle, b"state_voting", &value);
    }

    fn load_voting_state(&self) -> VotingState {
        let state_handle = self.store.cf_handle("state").unwrap();
        let Some(voting_state) = self.store.get_cf(&state_handle, b"state_voting").unwrap() else {
            return VotingState { current_term: 0, voted_for: None };
        };

        let (state, _): (VotingState, usize) = decode_from_slice(&voting_state, config::standard()).unwrap();
        state
    }

    fn persist_commit_idx(&mut self, commit_idx: u64) {
        let state_handle = self.store.cf_handle("state").unwrap();
        self.store.put_cf(&state_handle, b"state_commit_idx", commit_idx.to_be_bytes());
    }
    fn persist_last_applied_idx(&mut self, last_applied_idx: u64) {
        let state_handle = self.store.cf_handle("state").unwrap();
        self.store.put_cf(&state_handle, b"state_last_applied_idx", last_applied_idx.to_be_bytes());
    }

    fn append_log_entries(&mut self, logs: Vec<LogEntry>) {
        let mut batch = WriteBatch::new();
        let logs_handle = self.store.cf_handle("logs").unwrap();
    
        for log in &logs {
            // Used padding to ensure 10_1 comes after 2_1
            let key = format!("{:010}_{}", log.index, log.term);
            let value = log.encode_to_bytes();
            batch.put_cf(logs_handle, key, value);
        }

        self.store.write(batch);
    }

    fn load_log_entries(& self) -> Vec<LogEntry> {
        let logs_handle = self.store.cf_handle("logs").unwrap();
        let mut iterator = self.store.iterator_cf(logs_handle, IteratorMode::Start);
        let mut log_entries = Vec::new();
        while let Some(Ok((x, value))) = iterator.next() {
            let log = LogEntry::decode(&value);
            log_entries.push(log);
        }

        log_entries
    }

    // Truncates log entries from index idx to resolve conflicts
    fn truncate_from(&mut self, idx: u64) {
        let logs_handle = self.store.cf_handle("logs").unwrap();
        let from = format!("{:010}_0", idx);
        let to = String::from("a");
        self.store.delete_range_cf(logs_handle, from, to).unwrap();
    }

    fn snapshot_state(&self) {}
    fn recover_state(&mut self) {}
}

