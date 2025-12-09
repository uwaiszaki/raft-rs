use rocksdb::{DB, Options};

use crate::raft_node::NodeId;
use super::{StateMachine, errors::Error};

pub struct RocksDb {
    pub db: DB
}

impl RocksDb {
    fn new(node_id: NodeId) -> Self {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        let path = format!("/var/db/rocksdb/{}", node_id);
        let db = DB::open(&opts, path).unwrap();
        Self {
            db
        }
    }
}

impl StateMachine for RocksDb {
    fn set(&self, key: &[u8], value: &[u8]) -> Result<(), Error> {
        self.db.put(key, value)?;
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let val = self.db.get(key)?;
        Ok(val)
    }
}
