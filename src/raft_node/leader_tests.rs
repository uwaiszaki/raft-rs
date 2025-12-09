#[cfg(test)]
mod tests {
    use tokio::time::Duration;

    use crate::{
        raft_node::{RaftNode, NodeId},
        state::NodeState,
        state::Leader,
        config::Config,
        message::{MessageEnvelope, RpcMessage, AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest},
        transport::{local_transport::LocalTransport, Transport},
    };
    use crate::storage::mock::MockStorage;
    use crate::state_machine::mock::MockStateMachine;
    use crate::raft_node::test_utils::create_log_entry;

    fn setup_node(id: NodeId, peers: Vec<NodeId>) -> RaftNode<MockStorage, MockStateMachine, LocalTransport> {
        let config = Config::default()
            .with_heartbeat_interval(Duration::from_millis(100))
            .with_election_timeout(Duration::from_millis(500));
        
        let storage = MockStorage::new();
        let state_machine = MockStateMachine::new();
        let mut transport = LocalTransport::new();
        transport.setup_node(id);
        for peer in &peers {
            transport.setup_node(*peer);
        }

        let leader_state = Leader::new(peers.clone());
        let state_data = NodeState::Leader(leader_state);

        RaftNode::new(
            id,
            config,
            peers,
            state_data,
            storage,
            state_machine,
            transport,
        )
    }

    #[tokio::test]
    async fn test_leader_sends_heartbeat() {
        let mut leader = setup_node(NodeId(1), vec![NodeId(2), NodeId(3)]);
        leader.current_term = 1;

        // Send heartbeat
        leader.send_leader_heartbeat().await;

        // Check that followers received heartbeat
        let transport = &leader.transport;
        let msg1 = transport.receive(NodeId(2)).await.unwrap();
        let msg2 = transport.receive(NodeId(3)).await.unwrap();

        match msg1.message {
            RpcMessage::AppendEntries(req) => {
                assert_eq!(req.term, 1);
                assert_eq!(req.leader_id, NodeId(1));
                assert_eq!(req.entries.len(), 0); // Empty = heartbeat
            }
            _ => panic!("Expected AppendEntries"),
        }

        match msg2.message {
            RpcMessage::AppendEntries(req) => {
                assert_eq!(req.term, 1);
                assert_eq!(req.leader_id, NodeId(1));
            }
            _ => panic!("Expected AppendEntries"),
        }
    }

    #[tokio::test]
    async fn test_leader_sends_log_entries() {
        let mut leader = setup_node(NodeId(1), vec![NodeId(2)]);
        leader.current_term = 1;

        // Add some log entries
        leader.logs.push(create_log_entry(1, 1, b"cmd1"));
        leader.logs.push(create_log_entry(1, 2, b"cmd2"));

        // Initialize next_index
        if let NodeState::Leader(ref mut leader_state) = leader.state_data {
            leader_state.next_index.insert(NodeId(2), 1);
        }

        leader.send_leader_heartbeat().await;

        let transport = &leader.transport;
        let msg = transport.receive(NodeId(2)).await.unwrap();

        match msg.message {
            RpcMessage::AppendEntries(req) => {
                assert_eq!(req.entries.len(), 2);
                assert_eq!(req.prev_log_index, 0);
                assert_eq!(req.prev_log_term, 0);
            }
            _ => panic!("Expected AppendEntries with entries"),
        }
    }

    #[tokio::test]
    async fn test_leader_handles_append_entries_response_success() {
        let mut leader = setup_node(NodeId(1), vec![NodeId(2)]);
        leader.current_term = 1;

        // Initialize leader state
        if let NodeState::Leader(ref mut leader_state) = leader.state_data {
            leader_state.next_index.insert(NodeId(2), 1);
            leader_state.match_index.insert(NodeId(2), 0);
        }

        // Add log entry
        leader.logs.push(create_log_entry(1, 1, b"cmd1"));

        let response = AppendEntriesResponse {
            term: 1,
            success: true,
            match_index: 1,
            conflict_index: None,
            conflict_term: None,
        };

        leader.handle_append_entries_response(NodeId(2), &response).await;

        // Check that match_index and next_index were updated
        if let NodeState::Leader(ref leader_state) = leader.state_data {
            assert_eq!(leader_state.match_index.get(&NodeId(2)), Some(&1));
            assert_eq!(leader_state.next_index.get(&NodeId(2)), Some(&2));
        }
    }

    #[tokio::test]
    async fn test_leader_handles_append_entries_response_failure() {
        let mut leader = setup_node(NodeId(1), vec![NodeId(2)]);
        leader.current_term = 1;

        // Add log entries
        leader.logs.push(create_log_entry(1, 1, b"cmd1"));
        leader.logs.push(create_log_entry(1, 2, b"cmd2"));

        // Initialize next_index to 3 (follower doesn't have these entries)
        if let NodeState::Leader(ref mut leader_state) = leader.state_data {
            leader_state.next_index.insert(NodeId(2), 3);
        }

        let response = AppendEntriesResponse {
            term: 1,
            success: false,
            match_index: 0,
            conflict_index: Some(1),
            conflict_term: Some(0),
        };

        leader.handle_append_entries_response(NodeId(2), &response).await;

        // Check that next_index was decremented
        if let NodeState::Leader(ref leader_state) = leader.state_data {
            let next_idx = leader_state.next_index.get(&NodeId(2)).unwrap();
            assert!(*next_idx < 3); // Should be reduced
        }
    }

    #[tokio::test]
    async fn test_leader_steps_down_on_higher_term() {
        let mut leader = setup_node(NodeId(1), vec![NodeId(2)]);
        leader.current_term = 1;

        let response = AppendEntriesResponse {
            term: 2, // Higher term
            success: false,
            match_index: 0,
            conflict_index: None,
            conflict_term: None,
        };

        leader.handle_append_entries_response(NodeId(2), &response).await;

        // Should step down
        assert_eq!(leader.current_term, 2);
        assert!(matches!(leader.state_data, NodeState::Follower(_)));
    }

    #[tokio::test]
    async fn test_leader_handles_request_vote_rejects_lower_term() {
        let mut leader = setup_node(NodeId(1), vec![NodeId(2)]);
        leader.current_term = 2;

        let vote_req = RequestVoteRequest {
            term: 1, // Lower term
            candidate_id: NodeId(3),
            last_log_index: 0,
            last_log_term: 0,
        };

        leader.handle_request_vote_as_leader(NodeId(3), &vote_req).await;

        // Should send rejection
        let transport = &leader.transport;
        let response = transport.receive(NodeId(3)).await.unwrap();

        match response.message {
            RpcMessage::RequestVoteResponse(resp) => {
                assert_eq!(resp.term, 2);
                assert_eq!(resp.vote_granted, false);
            }
            _ => panic!("Expected RequestVoteResponse"),
        }
    }

    #[tokio::test]
    async fn test_leader_steps_down_on_higher_term_vote_request() {
        let mut leader = setup_node(NodeId(1), vec![NodeId(2)]);
        leader.current_term = 1;

        let vote_req = RequestVoteRequest {
            term: 2, // Higher term
            candidate_id: NodeId(3),
            last_log_index: 0,
            last_log_term: 0,
        };

        leader.handle_request_vote_as_leader(NodeId(3), &vote_req).await;

        // Should step down
        assert_eq!(leader.current_term, 2);
        assert!(matches!(leader.state_data, NodeState::Follower(_)));
    }

    #[tokio::test]
    async fn test_leader_handles_split_brain() {
        let mut leader = setup_node(NodeId(1), vec![NodeId(2)]);
        leader.current_term = 1;

        // Another leader with higher term sends AppendEntries
        let split_brain_request = AppendEntriesRequest {
            term: 2, // Higher term
            leader_id: NodeId(3),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let envelope = MessageEnvelope {
            to: NodeId(1),
            from: NodeId(3),
            message: RpcMessage::AppendEntries(split_brain_request),
        };

        let should_step_down = leader.handle_leader_message(envelope).await;

        // Should step down
        assert!(should_step_down);
        assert_eq!(leader.current_term, 2);
        assert!(matches!(leader.state_data, NodeState::Follower(_)));
    }

    #[tokio::test]
    async fn test_leader_updates_commit_index() {
        let mut leader = setup_node(NodeId(1), vec![NodeId(2), NodeId(3)]);
        leader.current_term = 1;

        // Add log entries
        leader.logs.push(create_log_entry(1, 1, b"cmd1"));
        leader.logs.push(create_log_entry(1, 2, b"cmd2"));

        // Initialize leader state
        if let NodeState::Leader(ref mut leader_state) = leader.state_data {
            leader_state.match_index.insert(NodeId(2), 2);
            leader_state.match_index.insert(NodeId(3), 2);
        }

        // Update commit index (majority = 2, so index 2 should be committed)
        leader.update_commit_index();

        assert_eq!(leader.commit_idx, 2);
    }

    #[tokio::test]
    async fn test_leader_only_commits_current_term_entries() {
        let mut leader = setup_node(NodeId(1), vec![NodeId(2), NodeId(3)]);
        leader.current_term = 2;

        // Add log entries from different terms
        leader.logs.push(create_log_entry(1, 1, b"cmd1")); // Old term
        leader.logs.push(create_log_entry(2, 2, b"cmd2")); // Current term

        // Initialize leader state - both followers have index 2
        if let NodeState::Leader(ref mut leader_state) = leader.state_data {
            leader_state.match_index.insert(NodeId(2), 2);
            leader_state.match_index.insert(NodeId(3), 2);
        }

        leader.update_commit_index();

        // Should only commit index 2 (current term), not index 1 (old term)
        assert_eq!(leader.commit_idx, 2);
    }

    #[tokio::test]
    async fn test_leader_conflict_resolution_with_conflict_info() {
        let mut leader = setup_node(NodeId(1), vec![NodeId(2)]);
        leader.current_term = 1;

        // Leader has: [term1:cmd1, term1:cmd2, term1:cmd3]
        leader.logs.push(create_log_entry(1, 1, b"cmd1"));
        leader.logs.push(create_log_entry(1, 2, b"cmd2"));
        leader.logs.push(create_log_entry(1, 3, b"cmd3"));

        // Initialize next_index to 4 (follower doesn't have these)
        if let NodeState::Leader(ref mut leader_state) = leader.state_data {
            leader_state.next_index.insert(NodeId(2), 4);
        }

        // Follower rejects with conflict at index 1, term 0 (follower has nothing)
        let response = AppendEntriesResponse {
            term: 1,
            success: false,
            match_index: 0,
            conflict_index: Some(1),
            conflict_term: Some(0),
        };

        leader.handle_append_entries_response(NodeId(2), &response).await;

        // Should update next_index to 1 (start from beginning)
        if let NodeState::Leader(ref leader_state) = leader.state_data {
            let next_idx = leader_state.next_index.get(&NodeId(2)).unwrap();
            assert_eq!(*next_idx, 1);
        }
    }
}

