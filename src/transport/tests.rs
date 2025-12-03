use tokio::time::{sleep, Duration};
use std::sync::Arc;

use crate::{
    message::{RpcMessage, MessageEnvelope},
    node::NodeId, transport::Transport
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

    let handle = tokio::spawn(async move {
        let ping_message = MessageEnvelope {
            to: NodeId(2),
            from: NodeId(1),
            message: RpcMessage::Ping(String::from("PING"))
        };
        transport_1.send(ping_message).await.unwrap();
        println!("[THREAD-1] Sent the ping :: Now waiting for PONG");
        let pong_message = transport_1.receive(NodeId(1)).await.unwrap();
        println!("[THREAD-1] Got {:?}", pong_message);

        assert_eq!(pong_message.from, NodeId(2), "PONG should be from node 2");
        assert_eq!(pong_message.to, NodeId(1), "PONG should be to node 1");
        match pong_message.message {
            RpcMessage::Ping(ref msg) => {
                assert_eq!(msg, "PONG", "Should receive PONG message");
            }
            _ => panic!("Expected Ping message, got something else"),
        }
    });


    let ping_message = transport.receive(NodeId(2)).await.unwrap();
    println!("[MAIN] Got {:?}", ping_message);
    assert_eq!(ping_message.from, NodeId(1), "PING should be from node 1");
    assert_eq!(ping_message.to, NodeId(2), "PING should be to node 2");
    match ping_message.message {
        RpcMessage::Ping(ref msg) => {
            assert_eq!(msg, "PING", "Should receive PING message");
        }
        _ => panic!("Expected Ping message, got something else"),
    }

    let pong_message = MessageEnvelope {
        to: 1.into(),
        from: 2.into(),
        message: RpcMessage::Ping(String::from("PONG"))
    };
    transport.send(pong_message).await.unwrap();
    handle.await.unwrap();
}