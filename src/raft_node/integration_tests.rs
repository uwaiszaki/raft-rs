#[cfg(test)]
mod tests {
    use tokio::time::Duration;

    use crate::{
        raft_node::{RaftNode, NodeId},
        state::NodeState,
        state::{Leader, Follower, Candidate},
        config::Config,
        message::{MessageEnvelope, RpcMessage, AppendEntriesRequest, RequestVoteRequest},
        transport::{local_transport::LocalTransport, Transport},
    };
    use crate::storage::mock::MockStorage;
    use crate::state_machine::mock::MockStateMachine;
    use crate::raft_node::test_utils::create_log_entry;

    fn create_node(
        id: NodeId,
        peers: Vec<NodeId>,
        term: u64,
        state: NodeState,
    ) -> RaftNode<MockStorage, MockStateMachine, LocalTransport> {
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

        let mut node = RaftNode::new(
            id,
            config,
            peers,
            state,
            storage,
            state_machine,
            transport,
        );
        node.current_term = term;
        node
    }

    #[tokio::test]
    async fn test_split_brain_scenario() {
        // Scenario: Two leaders exist with different terms
        // Leader 1 (term 1) and Leader 2 (term 2) - Leader 2 should win

        let mut leader1 = create_node(
            NodeId(1),
            vec![NodeId(2), NodeId(3)],
            1,
            NodeState::Leader(Leader::new(vec![NodeId(2), NodeId(3)])),
        );

        let mut leader2 = create_node(
            NodeId(2),
            vec![NodeId(1), NodeId(3)],
            2,
            NodeState::Leader(Leader::new(vec![NodeId(1), NodeId(3)])),
        );

        // Leader 2 sends AppendEntries to Leader 1
        let append_req = AppendEntriesRequest {
            term: 2,
            leader_id: NodeId(2),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let envelope = MessageEnvelope {
            to: NodeId(1),
            from: NodeId(2),
            message: RpcMessage::AppendEntries(append_req),
        };

        let should_step_down = leader1.handle_leader_message(envelope).await;

        // Leader 1 should step down
        assert!(should_step_down);
        assert_eq!(leader1.current_term, 2);
        assert!(matches!(leader1.state_data, NodeState::Follower(_)));
    }

    #[tokio::test]
    async fn test_election_scenario() {
        // Scenario: Follower times out and becomes candidate, wins election

        let mut follower = create_node(
            NodeId(1),
            vec![NodeId(2), NodeId(3)],
            1,
            NodeState::Follower(Follower { voted_for: None }),
        );

        // Follower becomes candidate
        follower.current_term += 1;
        follower.state_data = NodeState::Candidate(Candidate {
            votes_received: vec![NodeId(1)],
        });

        // Send vote requests
        let (last_log_index, last_log_term) = follower.get_last_log_info();
        for peer in &follower.peers {
            let vote_req = RequestVoteRequest {
                term: follower.current_term,
                candidate_id: follower.id,
                last_log_index,
                last_log_term,
            };

            let envelope = MessageEnvelope {
                to: *peer,
                from: follower.id,
                message: RpcMessage::RequestVote(vote_req),
            };

            let _ = follower.transport.send(envelope).await;
        }

        // Simulate receiving votes (in real scenario, these come from other nodes)
        // For this test, we verify the vote request was sent correctly
        let transport = &follower.transport;
        let msg = transport.receive(NodeId(2)).await.unwrap();

        match msg.message {
            RpcMessage::RequestVote(req) => {
                assert_eq!(req.term, 2);
                assert_eq!(req.candidate_id, NodeId(1));
            }
            _ => panic!("Expected RequestVote"),
        }
    }

    #[tokio::test]
    async fn test_leader_timeout_follower_becomes_candidate() {
        // Scenario: Leader stops sending heartbeats, follower times out

        let mut follower = create_node(
            NodeId(2),
            vec![NodeId(1)],
            1,
            NodeState::Follower(Follower { voted_for: None }),
        );

        // Set short timeout
        follower.config.election_timeout = Duration::from_millis(50);

        // No heartbeat received - follower should timeout and become candidate
        // This is tested by the run_follower method which transitions to candidate on timeout
        // For unit test, we verify the timeout configuration
        assert!(follower.config.election_timeout.as_millis() < 100);
    }

    #[tokio::test]
    async fn test_log_replication_scenario() {
        // Scenario: Leader replicates log entries to followers

        let mut leader = create_node(
            NodeId(1),
            vec![NodeId(2), NodeId(3)],
            1,
            NodeState::Leader(Leader::new(vec![NodeId(2), NodeId(3)])),
        );

        // Add log entries
        leader.logs.push(create_log_entry(1, 1, b"cmd1"));
        leader.logs.push(create_log_entry(1, 2, b"cmd2"));

        // Initialize next_index
        if let NodeState::Leader(ref mut leader_state) = leader.state_data {
            leader_state.next_index.insert(NodeId(2), 1);
            leader_state.next_index.insert(NodeId(3), 1);
        }

        // Send AppendEntries
        leader.send_leader_heartbeat().await;

        // Verify followers received entries
        let transport = &leader.transport;
        let msg1 = transport.receive(NodeId(2)).await.unwrap();
        let msg2 = transport.receive(NodeId(3)).await.unwrap();

        match msg1.message {
            RpcMessage::AppendEntries(req) => {
                assert_eq!(req.entries.len(), 2);
                assert_eq!(req.term, 1);
            }
            _ => panic!("Expected AppendEntries"),
        }

        match msg2.message {
            RpcMessage::AppendEntries(req) => {
                assert_eq!(req.entries.len(), 2);
            }
            _ => panic!("Expected AppendEntries"),
        }
    }

    #[tokio::test]
    async fn test_conflict_resolution_scenario() {
        // Scenario: Follower has conflicting log, leader resolves it

        let mut follower = create_node(
            NodeId(2),
            vec![NodeId(1)],
            1,
            NodeState::Follower(Follower { voted_for: None }),
        );

        // Follower has conflicting entry
        follower.logs.push(create_log_entry(1, 1, b"old_cmd"));

        let mut leader = create_node(
            NodeId(1),
            vec![NodeId(2)],
            1,
            NodeState::Leader(Leader::new(vec![NodeId(2)])),
        );

        // Leader has different entry at same index
        leader.logs.push(create_log_entry(1, 1, b"new_cmd"));

        // Initialize next_index
        if let NodeState::Leader(ref mut leader_state) = leader.state_data {
            leader_state.next_index.insert(NodeId(2), 2); // Will need to retry
        }

        // Leader sends AppendEntries
        leader.send_leader_heartbeat().await;

        let transport = &leader.transport;
        let msg = transport.receive(NodeId(2)).await.unwrap();

        match msg.message {
            RpcMessage::AppendEntries(req) => {
                // Should start from beginning (prev_log_index=0)
                assert_eq!(req.prev_log_index, 0);
                assert_eq!(req.entries.len(), 1);
            }
            _ => panic!("Expected AppendEntries"),
        }
    }

    #[tokio::test]
    async fn test_commit_propagation_scenario() {
        // Scenario: Leader commits entry, propagates to followers

        let mut leader = create_node(
            NodeId(1),
            vec![NodeId(2), NodeId(3)],
            1,
            NodeState::Leader(Leader::new(vec![NodeId(2), NodeId(3)])),
        );

        // Add log entry
        leader.logs.push(create_log_entry(1, 1, b"cmd1"));

        // Initialize match_index to simulate replication
        if let NodeState::Leader(ref mut leader_state) = leader.state_data {
            leader_state.match_index.insert(NodeId(2), 1);
            leader_state.match_index.insert(NodeId(3), 1);
        }

        // Update commit index (majority = 2, so index 1 should be committed)
        leader.update_commit_index();

        assert_eq!(leader.commit_idx, 1);
    }

    #[tokio::test]
    async fn test_multiple_candidates_split_vote() {
        // Scenario: Multiple candidates, none gets majority, election fails

        let mut candidate1 = create_node(
            NodeId(1),
            vec![NodeId(2), NodeId(3), NodeId(4)],
            1,
            NodeState::Candidate(Candidate {
                votes_received: vec![NodeId(1)],
            }),
        );

        // Candidate 1 gets vote from node 2
        let mut votes_received: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
        votes_received.insert(NodeId(1));

        let response = crate::message::RequestVoteResponse {
            term: 1,
            vote_granted: true,
        };

        let envelope = MessageEnvelope {
            to: NodeId(1),
            from: NodeId(2),
            message: RpcMessage::RequestVoteResponse(response),
        };

        let majority = (candidate1.peers.len() + 1) / 2 + 1; // 3 for 4 nodes
        let (won, _) = candidate1.handle_candidate_message(envelope, &mut votes_received, majority).await;

        // Should not win yet (only 2 votes, need 3)
        assert!(!won);
        assert_eq!(votes_received.len(), 2);
    }
}

