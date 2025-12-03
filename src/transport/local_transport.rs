use dashmap::DashMap;
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::message::RpcMessage;
use crate::node::NodeId;
use crate::transport::errors::TransportError;
use crate::transport::{Transport, TransportResult};

struct NodeInfo {
    rpc_tx: Sender<RpcMessage>
}

pub struct LocalTransport {
    nodes: DashMap<NodeId, NodeInfo>
}

impl LocalTransport {
    fn new() -> Self {
        Self {
            nodes: DashMap::new()
        }
    }
}

impl Transport for LocalTransport {
    fn setup_node(&mut self, node_id: NodeId) -> (Sender<RpcMessage>, Receiver<RpcMessage>) {
        let (rpc_tx, mut rpc_rx) = mpsc::channel::<RpcMessage>(100);
        self.nodes.insert(node_id, NodeInfo { rpc_tx: rpc_tx.clone() });
        // let x = rpc_rx;
        return (rpc_tx, rpc_rx);
    }

    async fn send(&self, node_id: NodeId, message: RpcMessage) -> Result<(), TransportError> {
        let Some(target_node) = self.nodes.get(&node_id) else {
            return Err(TransportError::NodeNotFound(node_id as u64))
        };
        let target_node_tx = target_node.rpc_tx.clone();

        target_node_tx.send(message).await?;

        Ok(())
    }

    async fn receive(&self, rpc_rx: &mut Receiver<RpcMessage>) -> Result<RpcMessage, TransportError> {
        rpc_rx.recv().await.ok_or(TransportError::ReceiveError)
    }
}