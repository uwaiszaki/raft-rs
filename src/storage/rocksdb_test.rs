use tempfile::TempDir;
use rocksdb::{DB, Options};
use bytes::Bytes;

#[cfg(test)]
mod tests {

    use crate::storage::{Storage, rocksdb::RocksDb, VotingState};
    use crate::raft_node::LogEntry;

    use super::*;

    fn setup_db() -> RocksDb {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_db");
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        let db = DB::open_cf(
            &opts,
            db_path,
            &["logs", "state"]
        ).unwrap();

        RocksDb{ 
            store: db
        }
    }

    #[test]
    fn test_persist_and_load_voting_state() {
        let mut db = setup_db();
        let mut voting_state = db.load_voting_state();

        // Starts with Empty State
        assert_eq!(voting_state.current_term, 0);
        assert_eq!(voting_state.voted_for, None);

        voting_state.current_term += 1;
        voting_state.voted_for = Some(3);

        db.persist_voting_state(&voting_state);
        voting_state = db.load_voting_state();

        // Should have the new state
        assert_eq!(voting_state.current_term, 1);
        assert_eq!(voting_state.voted_for, Some(3));
    }

    #[test]
    fn test_persist_and_load_voting_state_multiple_updates() {
        let mut db = setup_db();
        
        // First update
        let state1 = VotingState {
            current_term: 5,
            voted_for: Some(10),
        };
        db.persist_voting_state(&state1);
        let loaded = db.load_voting_state();
        assert_eq!(loaded.current_term, 5);
        assert_eq!(loaded.voted_for, Some(10));

        // Second update
        let state2 = VotingState {
            current_term: 10,
            voted_for: None,
        };
        db.persist_voting_state(&state2);
        let loaded = db.load_voting_state();
        assert_eq!(loaded.current_term, 10);
        assert_eq!(loaded.voted_for, None);
    }

    #[test]
    fn test_persist_commit_idx() {
        let mut db = setup_db();

        // Persist commit index
        db.persist_commit_idx(42);

        // Read directly from DB to verify
        let state_handle = db.store.cf_handle("state").unwrap();
        let value = db.store.get_cf(&state_handle, b"state_commit_idx").unwrap().unwrap();
        let commit_idx = u64::from_be_bytes(value.try_into().unwrap());
        assert_eq!(commit_idx, 42);

        // Update to different value
        db.persist_commit_idx(100);
        // Get fresh handle after mutable operation
        let state_handle = db.store.cf_handle("state").unwrap();
        let value = db.store.get_cf(&state_handle, b"state_commit_idx").unwrap().unwrap();
        let commit_idx = u64::from_be_bytes(value.try_into().unwrap());
        assert_eq!(commit_idx, 100);
    }

    #[test]
    fn test_persist_last_applied_idx() {
        let mut db = setup_db();

        // Persist last applied index
        db.persist_last_applied_idx(25);

        // Read directly from DB to verify
        let state_handle = db.store.cf_handle("state").unwrap();
        let value = db.store.get_cf(&state_handle, b"state_last_applied_idx").unwrap().unwrap();
        let last_applied_idx = u64::from_be_bytes(value.try_into().unwrap());
        assert_eq!(last_applied_idx, 25);

        // Update to different value
        db.persist_last_applied_idx(50);
        // Get fresh handle after mutable operation
        let state_handle = db.store.cf_handle("state").unwrap();
        let value = db.store.get_cf(&state_handle, b"state_last_applied_idx").unwrap().unwrap();
        let last_applied_idx = u64::from_be_bytes(value.try_into().unwrap());
        assert_eq!(last_applied_idx, 50);
    }

    #[test]
    fn test_append_and_load_log_entries() {
        let mut db = setup_db();

        // Load logs from empty DB
        let loaded = db.load_log_entries();
        assert_eq!(loaded.len(), 0);

        // Create test log entries
        let logs = vec![
            LogEntry {
                index: 1,
                term: 1,
                cmd: Bytes::from("command1"),
            },
            LogEntry {
                index: 2,
                term: 1,
                cmd: Bytes::from("command2"),
            },
            LogEntry {
                index: 3,
                term: 2,
                cmd: Bytes::from("command3"),
            },
        ];

        // Append logs
        db.append_log_entries(logs);

        // Load logs
        let loaded = db.load_log_entries();

        // Verify all logs are loaded
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].index, 1);
        assert_eq!(loaded[0].term, 1);
        assert_eq!(loaded[0].cmd, Bytes::from("command1"));
        assert_eq!(loaded[1].index, 2);
        assert_eq!(loaded[1].term, 1);
        assert_eq!(loaded[1].cmd, Bytes::from("command2"));
        assert_eq!(loaded[2].index, 3);
        assert_eq!(loaded[2].term, 2);
        assert_eq!(loaded[2].cmd, Bytes::from("command3"));
    }

    #[test]
    fn test_append_log_entries_multiple_batches() {
        let mut db = setup_db();

        // First batch
        let logs1 = vec![
            LogEntry {
                index: 1,
                term: 1,
                cmd: Bytes::from("batch1_cmd1"),
            },
            LogEntry {
                index: 2,
                term: 1,
                cmd: Bytes::from("batch1_cmd2"),
            },
        ];
        db.append_log_entries(logs1);

        // Second batch
        let logs2 = vec![
            LogEntry {
                index: 3,
                term: 2,
                cmd: Bytes::from("batch2_cmd1"),
            },
        ];
        db.append_log_entries(logs2);

        // Load all logs
        let loaded = db.load_log_entries();
        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].index, 1);
        assert_eq!(loaded[1].index, 2);
        assert_eq!(loaded[2].index, 3);
        assert_eq!(loaded[2].cmd, Bytes::from("batch2_cmd1"));
    }

    #[test]
    fn test_truncate_from() {
        let mut db = setup_db();

        // Add some log entries
        let logs = vec![
            LogEntry {
                index: 1,
                term: 1,
                cmd: Bytes::from("cmd1"),
            },
            LogEntry {
                index: 2,
                term: 1,
                cmd: Bytes::from("cmd2"),
            },
            LogEntry {
                index: 3,
                term: 2,
                cmd: Bytes::from("cmd3"),
            },
        ];
        db.append_log_entries(logs);

        // Test truncate_from doesn't crash (implementation is empty)
        // This test verifies the interface works
        db.truncate_from(2);
        
        // Verify logs are still there (since truncate_from is not implemented)
        let loaded = db.load_log_entries();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].index, 1);
    }

    #[test]
    fn test_log_entries_preserve_order() {
        let mut db = setup_db();

        // Create logs with specific order
        let logs: Vec<LogEntry> = (1..=10).map(|i| LogEntry {
            index: i,
            term: i / 3 + 1, // Varying terms
            cmd: Bytes::from(format!("cmd{}", i)),
        }).collect();

        db.append_log_entries(logs);
        let loaded = db.load_log_entries();

        // With zero-padded keys, entries should already be in correct order
        // Verify all entries are present and in correct order
        assert_eq!(loaded.len(), 10);
        for (i, log) in loaded.iter().enumerate() {
            assert_eq!(log.index, (i + 1) as u64);
            assert_eq!(log.cmd, Bytes::from(format!("cmd{}", i + 1)));
        }
    }
}