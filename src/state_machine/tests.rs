use rocksdb::{DB, Options};
use super::{StateMachine, rocksdb::RocksDb};
use tempfile::TempDir;

#[test]
fn test_state_machine_set_and_get() {
    let tmp_dir = TempDir::new().unwrap();
    let db_path = tmp_dir.path().join("test_db");
    let mut db_opts = Options::default();
    db_opts.create_if_missing(true);
    let db = DB::open(&db_opts, db_path).unwrap();
    
    let store = RocksDb { db };

    let key = b"abc";
    let val = store.get(key).unwrap();
    assert_eq!(val, None);

    store.set(key, b"Random Value").unwrap();
    let val = store.get(key).unwrap();
    assert_eq!(val, Some(b"Random Value".to_vec()));
}