use thiserror::Error;
use tokio::sync::mpsc;

use crate::{message::MessageEnvelope, raft_node::NodeId};

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("Failed to send message")]
    SendError(#[from] mpsc::error::SendError<MessageEnvelope>),
    #[error("Failed to receive message")]
    ReceiveError,

    #[error("Node {0} not found")]
    NodeNotFound(NodeId),
}