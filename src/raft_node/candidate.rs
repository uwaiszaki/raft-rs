use tokio::time::{sleep, Instant};
use std::collections::HashSet;

use super::RaftNode;
use crate::{
    message::{MessageEnvelope, RpcMessage, RequestVoteRequest},
    state::NodeState,
    storage::Storage,
    state_machine::StateMachine,
    transport::Transport,
    raft_node::NodeId,
};


impl<S, SM, T> RaftNode<S, SM, T>
where
    S: Storage,
    SM: StateMachine,
    T: Transport
{
    pub async fn run_candidate(&mut self) {
        // Increment term and vote for self
        self.current_term += 1;
        
        // Persist voting state
        let voting_state = crate::storage::VotingState {
            current_term: self.current_term,
            voted_for: Some(self.id.0 as u64),
        };
        self.storage.persist_voting_state(&voting_state);

        // Get last log info
        let (last_log_index, last_log_term) = self.get_last_log_info();

        // Send RequestVote to all peers
        let mut votes_received: HashSet<NodeId> = HashSet::new();
        votes_received.insert(self.id); // Vote for self

        for peer in &self.peers {
            let vote_request = RequestVoteRequest {
                term: self.current_term,
                candidate_id: self.id,
                last_log_index,
                last_log_term,
            };

            let envelope = MessageEnvelope {
                to: *peer,
                from: self.id,
                message: RpcMessage::RequestVote(vote_request),
            };

            // Send asynchronously
            let _ = self.transport.send(envelope).await;
        }

        // Calculate majority
        let total_nodes = self.peers.len() + 1; // Including self
        let majority = total_nodes / 2 + 1;

        // Set election timeout (randomized to avoid split votes)
        let election_timeout = self.generate_election_timeout();

        loop {
            tokio::select! {
                _ = sleep(election_timeout) => {
                    // Election timeout - become follower or start new election
                    // For simplicity, become follower (in production, might retry)
                    self.state_data = NodeState::Follower(crate::state::Follower {
                        voted_for: None,
                    });
                    return;
                }
                rpc_resp = self.receive_rpc() => {
                    match rpc_resp {
                        Ok(rpc_message) => {
                            let (new_leader, new_follower) = self.handle_candidate_message(
                                rpc_message,
                                &mut votes_received,
                                majority
                            ).await;
                            
                            if new_leader {
                                // Won election - become leader
                                self.state_data = NodeState::Leader(crate::state::Leader::new(self.peers.clone()));
                                return;
                            }
                            
                            if new_follower {
                                // Stepped down - become follower
                                return;
                            }
                        }
                        Err(_) => {
                            // Transport error - continue
                        }
                    }
                }
            }
        }
    }

    pub async fn handle_candidate_message(
        &mut self,
        rpc_req: MessageEnvelope,
        votes_received: &mut HashSet<NodeId>,
        majority: usize,
    ) -> (bool, bool) {
        match &rpc_req.message {
            RpcMessage::RequestVoteResponse(resp) => {
                if resp.term > self.current_term {
                    // Higher term - step down
                    self.current_term = resp.term;
                    self.state_data = NodeState::Follower(crate::state::Follower {
                        voted_for: None,
                    });
                    return (false, true); // Become follower
                }

                if resp.term < self.current_term {
                    // Stale response - ignore
                    return (false, false);
                }

                if resp.vote_granted {
                    votes_received.insert(rpc_req.from);

                    // Check if we have majority
                    if votes_received.len() >= majority {
                        return (true, false); // Won election
                    }
                }

                (false, false)
            }
            RpcMessage::AppendEntries(req) => {
                if req.term >= self.current_term {
                    // Leader exists - step down
                    self.current_term = req.term;
                    self.state_data = NodeState::Follower(crate::state::Follower {
                        voted_for: None,
                    });
                    return (false, true); // Become follower
                }
                (false, false)
            }
            RpcMessage::RequestVote(vote_req) => {
                if vote_req.term > self.current_term {
                    // Another candidate with higher term - step down
                    self.current_term = vote_req.term;
                    self.state_data = NodeState::Follower(crate::state::Follower {
                        voted_for: None,
                    });
                    return (false, true); // Become follower
                }
                (false, false)
            }
            _ => (false, false)
        }
    }

}