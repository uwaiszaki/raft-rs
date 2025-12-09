// Test utilities for Raft node tests
use bytes::Bytes;
use crate::raft_node::LogEntry;

// Helper function to create a log entry
pub fn create_log_entry(term: u64, index: u64, cmd: &[u8]) -> LogEntry {
    LogEntry {
        term,
        index,
        cmd: Bytes::copy_from_slice(cmd),
    }
}

