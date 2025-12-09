use tokio::time::{sleep, Duration};
use std::sync::Arc;

use crate::{
    message::{RpcMessage, MessageEnvelope},
    raft_node::NodeId, transport::Transport
};

use super::local_transport::LocalTransport;

#[tokio::test]
async fn test_local_transport_setup() {
    let mut transport = LocalTransport::new();
    transport.setup_node(NodeId(1));
    transport.setup_node(NodeId(2));

    assert_eq!(transport.nodes.len(), 2);
    transport.nodes.get(&NodeId(1)).unwrap();
}

#[tokio::test]
async fn test_local_transport_send_and_receive() {
    let mut transport = LocalTransport::new();
    transport.setup_node(NodeId(1));
    transport.setup_node(NodeId(2));

    let transport = Arc::new(transport);
    let transport_1 = transport.clone();
    let transport_2 = transport.clone();

    let (pong_result, ping_result) = tokio::join!(
        // Node 1: Send PING, receive PONG
        async move {
            let ping = MessageEnvelope {
                to: NodeId(2),
                from: NodeId(1),
                message: RpcMessage::Ping(String::from("PING"))
            };
            transport_1.send(ping).await.unwrap();
            println!("[NODE-1] Sent PING");
            
            let pong = transport_1.receive(NodeId(1)).await.unwrap();
            println!("[NODE-1] Received PONG");
            assert_eq!(pong.from, NodeId(2));
            pong
        },
        // Node 2: Receive PING, send PONG
        async move {
            let ping = transport_2.receive(NodeId(2)).await.unwrap();
            println!("[NODE-2] Received PING");
            assert_eq!(ping.from, NodeId(1));
            
            let pong = MessageEnvelope {
                to: NodeId(1),
                from: NodeId(2),
                message: RpcMessage::Ping(String::from("PONG"))
            };
            transport_2.send(pong).await.unwrap();
            println!("[NODE-2] Sent PONG");
            ping
        }
    );

    assert_eq!(ping_result.from, NodeId(1));
    assert_eq!(pong_result.from, NodeId(2));
}