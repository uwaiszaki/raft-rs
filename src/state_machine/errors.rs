use thiserror::Error as ThisError;


#[derive(ThisError, Debug)]
pub enum Error {
    #[error("Error connecting the state machine")]
    ConnectionError,

    #[error("Query failed with {0}")]
    QueryError(#[from] rocksdb::Error)
}