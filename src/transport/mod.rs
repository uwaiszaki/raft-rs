pub mod errors;
pub mod local_transport;

#[cfg(test)]
mod tests;

use errors::TransportError;
use crate::{message::MessageEnvelope, node::NodeId};

type TransportResult = Result<MessageEnvelope, TransportError>;

pub trait Transport {
    fn setup_node(&mut self, node_id: NodeId);
    async fn send(&self, message: MessageEnvelope) -> Result<(), errors::TransportError>;
    async fn receive(&self, node_id: NodeId) -> TransportResult;
}
