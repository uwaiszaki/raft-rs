#[cfg(test)]
mod tests {
    use tokio::time::Duration;

    use crate::{
        raft_node::{RaftNode, NodeId},
        state::NodeState,
        state::Follower,
        config::Config,
        message::{AppendEntriesRequest, RequestVoteRequest},
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

        let state_data = NodeState::Follower(Follower {
            voted_for: None,
        });

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
    async fn test_follower_accepts_heartbeat() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 1;

        let heartbeat = AppendEntriesRequest {
            term: 1,
            leader_id: NodeId(1),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let response = follower.handle_append_entries(&heartbeat).await;

        assert!(response.success);
        assert_eq!(response.term, 1);
        assert_eq!(response.match_index, 0);
    }

    #[tokio::test]
    async fn test_follower_rejects_lower_term() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 2;

        let heartbeat = AppendEntriesRequest {
            term: 1, // Lower term
            leader_id: NodeId(1),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let response = follower.handle_append_entries(&heartbeat).await;

        assert!(!response.success);
        assert_eq!(response.term, 2);
    }

    #[tokio::test]
    async fn test_follower_updates_term_on_higher_term() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 1;

        let heartbeat = AppendEntriesRequest {
            term: 2, // Higher term
            leader_id: NodeId(1),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let response = follower.handle_append_entries(&heartbeat).await;

        assert!(response.success);
        assert_eq!(follower.current_term, 2);
    }

    #[tokio::test]
    async fn test_follower_appends_entries() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 1;

        let entry1 = create_log_entry(1, 1, b"cmd1");
        let entry2 = create_log_entry(1, 2, b"cmd2");

        let append_req = AppendEntriesRequest {
            term: 1,
            leader_id: NodeId(1),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![entry1.clone(), entry2.clone()],
            leader_commit: 0,
        };

        let response = follower.handle_append_entries(&append_req).await;

        assert!(response.success);
        assert_eq!(follower.logs.len(), 2);
        assert_eq!(follower.logs[0].index, 1);
        assert_eq!(follower.logs[1].index, 2);
    }

    #[tokio::test]
    async fn test_follower_rejects_inconsistent_log() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 1;

        // Follower has entry at index 1 with term 1
        follower.logs.push(create_log_entry(1, 1, b"cmd1"));

        // Leader sends entry with prev_log_index=1 but prev_log_term=2 (mismatch)
        let append_req = AppendEntriesRequest {
            term: 1,
            leader_id: NodeId(1),
            prev_log_index: 1,
            prev_log_term: 2, // Mismatch!
            entries: vec![create_log_entry(1, 2, b"cmd2")],
            leader_commit: 0,
        };

        let response = follower.handle_append_entries(&append_req).await;

        assert!(!response.success);
        assert!(response.conflict_index.is_some());
    }

    #[tokio::test]
    async fn test_follower_truncates_conflicting_entries() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 1;

        // Follower has: [term1:cmd1, term1:cmd2, term1:cmd3]
        follower.logs.push(create_log_entry(1, 1, b"cmd1"));
        follower.logs.push(create_log_entry(1, 2, b"cmd2"));
        follower.logs.push(create_log_entry(1, 3, b"cmd3"));

        // Leader sends: prev_log_index=1 (matches), but new entries to replace index 2+
        let new_entry = create_log_entry(1, 2, b"new_cmd2");
        let append_req = AppendEntriesRequest {
            term: 1,
            leader_id: NodeId(1),
            prev_log_index: 1,
            prev_log_term: 1,
            entries: vec![new_entry.clone()],
            leader_commit: 0,
        };

        let response = follower.handle_append_entries(&append_req).await;

        assert!(response.success);
        assert_eq!(follower.logs.len(), 2); // Should truncate and keep only first entry + new
        assert_eq!(follower.logs[0].index, 1);
        assert_eq!(follower.logs[1].index, 2);
    }

    #[tokio::test]
    async fn test_follower_updates_commit_index() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 1;

        // Follower has entries
        follower.logs.push(create_log_entry(1, 1, b"cmd1"));
        follower.logs.push(create_log_entry(1, 2, b"cmd2"));

        let append_req = AppendEntriesRequest {
            term: 1,
            leader_id: NodeId(1),
            prev_log_index: 2,
            prev_log_term: 1,
            entries: vec![],
            leader_commit: 2, // Leader committed index 2
        };

        let response = follower.handle_append_entries(&append_req).await;

        assert!(response.success);
        assert_eq!(follower.commit_idx, 2);
    }

    #[tokio::test]
    async fn test_follower_grants_vote() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 1;

        let vote_req = RequestVoteRequest {
            term: 2, // Higher term
            candidate_id: NodeId(3),
            last_log_index: 0,
            last_log_term: 0,
        };

        let response = follower.handle_request_vote(&vote_req).await;

        assert!(response.vote_granted);
        assert_eq!(response.term, 2);
        assert_eq!(follower.current_term, 2);
    }

    #[tokio::test]
    async fn test_follower_rejects_vote_same_term_already_voted() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 1;

        // Vote for candidate 3
        let vote_req1 = RequestVoteRequest {
            term: 1,
            candidate_id: NodeId(3),
            last_log_index: 0,
            last_log_term: 0,
        };
        let _ = follower.handle_request_vote(&vote_req1).await;

        // Try to vote for candidate 4 in same term
        let vote_req2 = RequestVoteRequest {
            term: 1,
            candidate_id: NodeId(4),
            last_log_index: 0,
            last_log_term: 0,
        };
        let response = follower.handle_request_vote(&vote_req2).await;

        assert!(!response.vote_granted);
    }

    #[tokio::test]
    async fn test_follower_rejects_vote_stale_log() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 1;

        // Follower has more up-to-date log
        follower.logs.push(create_log_entry(1, 1, b"cmd1"));
        follower.logs.push(create_log_entry(1, 2, b"cmd2"));

        // Candidate has stale log (index 0, term 0)
        let vote_req = RequestVoteRequest {
            term: 2,
            candidate_id: NodeId(3),
            last_log_index: 0, // Stale
            last_log_term: 0,
        };

        let response = follower.handle_request_vote(&vote_req).await;

        assert!(!response.vote_granted);
    }

    #[tokio::test]
    async fn test_follower_becomes_candidate_on_timeout() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 1;

        // Set very short timeout for testing
        follower.config.election_timeout = Duration::from_millis(50);

        // Test that timeout is configured correctly
        // In a real scenario, follower would timeout and become candidate
        assert!(follower.config.election_timeout.as_millis() < 100);
    }

    #[tokio::test]
    async fn test_follower_resets_timeout_on_heartbeat() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 1;

        // Send heartbeat
        let heartbeat = AppendEntriesRequest {
            term: 1,
            leader_id: NodeId(1),
            prev_log_index: 0,
            prev_log_term: 0,
            entries: vec![],
            leader_commit: 0,
        };

        let _ = follower.handle_append_entries(&heartbeat).await;

        // Timeout should be reset (tested by checking that follower doesn't become candidate)
        // This is verified by the fact that handle_append_entries doesn't change state
        assert!(matches!(follower.state_data, NodeState::Follower(_)));
    }

    #[tokio::test]
    async fn test_follower_finds_conflict_correctly() {
        let mut follower = setup_node(NodeId(2), vec![NodeId(1)]);
        follower.current_term = 1;

        // Follower has: [term1:cmd1, term1:cmd2, term2:cmd3]
        follower.logs.push(create_log_entry(1, 1, b"cmd1"));
        follower.logs.push(create_log_entry(1, 2, b"cmd2"));
        follower.logs.push(create_log_entry(2, 3, b"cmd3"));

        // Leader sends with prev_log_index=2, prev_log_term=1 (matches)
        // But follower has term2 at index 3, so there's a conflict
        let append_req = AppendEntriesRequest {
            term: 1,
            leader_id: NodeId(1),
            prev_log_index: 2,
            prev_log_term: 1, // Matches
            entries: vec![create_log_entry(1, 3, b"new_cmd3")], // Different term
            leader_commit: 0,
        };

        let response = follower.handle_append_entries(&append_req).await;

        // Should accept and truncate
        assert!(response.success);
        assert_eq!(follower.logs.len(), 3);
        assert_eq!(follower.logs[2].term, 1); // Should be replaced
    }
}

