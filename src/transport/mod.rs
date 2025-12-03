pub mod errors;
pub mod local_transport;

use tokio::sync::mpsc::{Receiver, Sender};

use errors::TransportError;
use crate::{message::RpcMessage, node::NodeId};

type TransportResult = Result<RpcMessage, TransportError>;

pub trait Transport {
    fn setup_node(&mut self, node_id: NodeId) -> (Sender<RpcMessage>, Receiver<RpcMessage>);
    async fn send(&self, node_id: NodeId, message: RpcMessage) -> Result<(), errors::TransportError>;
    async fn receive(&self, rpc_rx: &mut Receiver<RpcMessage>) -> TransportResult;
}
