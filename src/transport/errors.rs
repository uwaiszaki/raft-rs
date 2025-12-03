use thiserror::Error;
use tokio::sync::mpsc;

use crate::message::RpcMessage;

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("Failed to send message")]
    SendError(#[from] mpsc::error::SendError<RpcMessage>),
    #[error("Failed to receive message")]
    ReceiveError,

    #[error("Node {0} not found")]
    NodeNotFound(u64),
}