use tokio::{
    select,
    time::interval
};

use super::RaftNode;
use crate::{
    message::{AppendEntriesRequest, AppendEntriesResponse, MessageEnvelope, RequestVoteRequest, RequestVoteResponse, RpcMessage}, raft_node::{LogEntry, NodeId}, state::NodeState, state_machine::StateMachine, storage::{Storage, VotingState}, transport::Transport
};


impl<S, SM, T> RaftNode<S, SM, T>
where
    S: Storage,
    SM: StateMachine,
    T: Transport
{
    pub async fn run_leader(&mut self) {
        // Initialize leader state
        if let NodeState::Leader(ref mut leader_state) = self.state_data {
            // Initialize next_index and match_index for all peers
            for peer in &self.peers {
                if !leader_state.next_index.contains_key(peer) {
                    leader_state.next_index.insert(*peer, self.logs.len() as u64 + 1);
                }
                if !leader_state.match_index.contains_key(peer) {
                    leader_state.match_index.insert(*peer, 0);
                }
            }
        }

        let heartbeat_interval = self.config.heartbeat_interval;
        let mut heartbeat_timer = interval(heartbeat_interval);

        loop {
            select! {
                _ = heartbeat_timer.tick() => {
                    self.send_leader_heartbeat().await;
                    heartbeat_timer.reset();
                },
                rpc_resp = self.receive_rpc() => {
                    match rpc_resp {
                        Ok(rpc_message) => {
                            let step_down = self.handle_leader_message(rpc_message).await;
                            if step_down {
                                return; // Step down and exit leader loop
                            }
                        }
                        Err(_) => {
                            // Transport error - log and continue
                            // In production, use proper logging
                        }
                    }
                }
            }
        }
    }

    pub async fn send_leader_heartbeat(&mut self) {
        let leader_state = match &mut self.state_data {
            NodeState::Leader(state) => state,
            _ => return, // Not a leader anymore
        };

        // Send AppendEntries (heartbeat or log replication) to all peers
        for peer in &self.peers {
            let next_idx = *leader_state.next_index.get(peer).unwrap_or(&(self.logs.len() as u64 + 1));
            
            // Prepare entries to send (entries already have correct index from when they were added)
            let entries: Vec<LogEntry> = if next_idx <= self.logs.len() as u64 {
                self.logs[(next_idx - 1) as usize..].iter().cloned().collect()
            } else {
                vec![] // Empty = heartbeat
            };

            // Calculate prev_log_index and prev_log_term
            let prev_log_index = if next_idx > 1 {
                next_idx - 1
            } else {
                0
            };
            
            let prev_log_term = if prev_log_index > 0 && prev_log_index <= self.logs.len() as u64 {
                self.logs[(prev_log_index - 1) as usize].term
            } else {
                0
            };

            let request = AppendEntriesRequest {
                term: self.current_term,
                leader_id: self.id,
                prev_log_index,
                prev_log_term,
                entries: entries.clone(),
                leader_commit: self.commit_idx,
            };

            let envelope = MessageEnvelope {
                to: *peer,
                from: self.id,
                message: RpcMessage::AppendEntries(request),
            };

            let transport = &self.transport;
            // We can skip waiting for response.
            let _ = transport.send(envelope).await;
        }
    }

    pub async fn handle_leader_message(&mut self, rpc_req: MessageEnvelope) -> bool {
        match &rpc_req.message {
            RpcMessage::AppendEntriesResponse(resp) => {
                self.handle_append_entries_response(rpc_req.from, resp).await
            }
            RpcMessage::RequestVote(vote_req) => {
                self.handle_request_vote_as_leader(rpc_req.from, vote_req).await
            }
            RpcMessage::AppendEntries(req) => {
                // Another leader exists (split-brain scenario)
                if req.term > self.current_term {
                    self.current_term = req.term;
                    // Step down and become follower
                    self.state_data = NodeState::Follower(crate::state::Follower {
                        voted_for: None,
                    });
                    return true; // Step down
                }
                false
            }
            _ => {
                false
            }
        }
    }

    pub async fn handle_append_entries_response(&mut self, follower_id: NodeId, resp: &AppendEntriesResponse) -> bool {
        // Check if follower has higher term
        if resp.term > self.current_term {
            self.current_term = resp.term;
            self.state_data = NodeState::Follower(crate::state::Follower {
                voted_for: None,
            });
            self.storage.persist_voting_state(&VotingState { current_term: self.current_term, voted_for: None });
            return true; // Will step down in next iteration
        }

        // Only process if we're still leader
        let leader_state = match &mut self.state_data {
            NodeState::Leader(state) => state,
            _ => return true,
        };

        if resp.success {
            // Update match_index and next_index
            if let Some(match_idx) = leader_state.match_index.get_mut(&follower_id) {
                *match_idx = resp.match_index;
            } else {
                leader_state.match_index.insert(follower_id, resp.match_index);
            }

            if let Some(next_idx) = leader_state.next_index.get_mut(&follower_id) {
                *next_idx = resp.match_index + 1;
            } else {
                leader_state.next_index.insert(follower_id, resp.match_index + 1);
            }

            // Update commit_index
            self.update_commit_index();
        } else {
            // Follower rejected - update next_index using conflict info
            if let Some(next_idx) = leader_state.next_index.get_mut(&follower_id) {
                if let (Some(conflict_idx), Some(conflict_term)) = (resp.conflict_index, resp.conflict_term) {
                    // Fast path: use conflict info to find correct next_index
                    // Find the last index in leader's log with term < conflict_term
                    let mut new_next_idx = conflict_idx;
                    
                    // Search backwards from conflict_index to find safe point
                    for i in (0..conflict_idx as usize).rev() {
                        if i < self.logs.len() {
                            if self.logs[i].term < conflict_term {
                                new_next_idx = (i + 1) as u64;
                                break;
                            }
                        } else {
                            new_next_idx = (i + 1) as u64;
                            break;
                        }
                    }
                    
                    *next_idx = new_next_idx;
                } else if let Some(conflict_idx) = resp.conflict_index {
                    // Only conflict_index available - jump to it
                    *next_idx = conflict_idx;
                } else {
                    // Slow path: decrement one by one
                    if *next_idx > 1 {
                        *next_idx -= 1;
                    }
                }
            }
        }

        false
    }

    pub async fn handle_request_vote_as_leader(&mut self, candidate_id: NodeId, vote_req: &RequestVoteRequest) -> bool {
        let mut vote_granted = false;

        if vote_req.term > self.current_term {
            // Candidate has higher term - step down
            self.current_term = vote_req.term;
            self.state_data = NodeState::Follower(crate::state::Follower {
                voted_for: Some(candidate_id),
            });

            let voting_state = VotingState {
                current_term: self.current_term,
                voted_for: Some(candidate_id.0 as u64)
            };

            self.storage.persist_voting_state(&voting_state);

            // Step down as leader
            vote_granted = true;
        }

        // If vote_granted = false, reject vote
        let response = RequestVoteResponse {
            term: self.current_term,
            vote_granted,
        };

        let response_msg = MessageEnvelope {
            to: candidate_id,
            from: self.id,
            message: RpcMessage::RequestVoteResponse(response),
        };

        let _ = self.transport.send(response_msg).await;

        vote_granted
    }

    pub fn update_commit_index(&mut self) {
        let leader_state = match &self.state_data {
            NodeState::Leader(state) => state,
            _ => return,
        };

        // Find the highest index replicated on majority of servers
        let mut match_indices: Vec<u64> = leader_state.match_index.values().copied().collect();
        match_indices.push(self.logs.len() as u64); // Include leader itself
        match_indices.sort();
        match_indices.reverse();


        // The reason we take majority is to commit all entries before majority
        let majority = (self.peers.len() + 1) / 2 + 1;
        if match_indices.len() >= majority {
            let new_commit_idx = match_indices[majority - 1];
            if new_commit_idx > self.commit_idx {
                self.commit_idx = new_commit_idx;
                self.storage.persist_commit_idx(self.commit_idx);
                self.apply_committed_entries();
            }
        }
    }

}
