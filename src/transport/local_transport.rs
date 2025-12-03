use dashmap::DashMap;
use tokio::sync::{
    Mutex,
    mpsc::{self, Receiver, Sender}
};
use std::sync::Arc;

use crate::message::MessageEnvelope;
use crate::node::NodeId;
use crate::transport::errors::TransportError;
use crate::transport::Transport;

pub struct NodeInfo {
    pub rpc_tx: Sender<MessageEnvelope>,
    pub rpc_rx: Arc<Mutex<Receiver<MessageEnvelope>>>
}

pub struct LocalTransport {
    pub nodes: DashMap<NodeId, NodeInfo>
}

impl LocalTransport {
    pub fn new() -> Self {
        Self {
            nodes: DashMap::new()
        }
    }
}

impl Transport for LocalTransport {
    fn setup_node(&mut self, node_id: NodeId) {
        let (rpc_tx, rpc_rx) = mpsc::channel::<MessageEnvelope>(100);
        let arc_rx = Arc::new(Mutex::new(rpc_rx));
        self.nodes.insert(node_id, NodeInfo { rpc_tx: rpc_tx.clone(), rpc_rx: arc_rx });
    }

    async fn send(&self, message: MessageEnvelope) -> Result<(), TransportError> {
        let Some(target_node) = self.nodes.get(&message.to) else {
            return Err(TransportError::NodeNotFound(message.to))
        };
        let target_node_tx = target_node.rpc_tx.clone();
        target_node_tx.send(message).await?;

        Ok(())
    }

    async fn receive(&self, node_id: NodeId) -> Result<MessageEnvelope, TransportError> {
        let Some(node_info) = self.nodes.get(&node_id) else {
            println!("Node {} not found", node_id);
            return Err(TransportError::ReceiveError);
        };
        let mut rx_guard = node_info.rpc_rx.lock().await;
        rx_guard.recv().await.ok_or_else(|| TransportError::ReceiveError)
    }
}