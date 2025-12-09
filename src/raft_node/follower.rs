use tokio::time::{Duration, Instant};
use std::cmp::min;

use super::{RaftNode, LogEntry};
use crate::{
    message::{MessageEnvelope, RpcMessage, AppendEntriesRequest, AppendEntriesResponse, RequestVoteRequest, RequestVoteResponse},
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
    pub async fn run_follower(&mut self) {
        // Generate random election timeout
        let mut election_timeout = self.generate_election_timeout();
        let _last_heartbeat = Instant::now();

        loop {
            tokio::select! {
                _ = tokio::time::sleep(election_timeout) => {
                    // Election timeout expired - become candidate
                    self.current_term += 1;
                    self.state_data = NodeState::Candidate(crate::state::Candidate {
                        votes_received: vec![self.id], // Vote for self
                    });
                    return; // Exit follower loop, will become candidate
                }
                rpc_resp = self.receive_rpc() => {
                    match rpc_resp {
                        Ok(rpc_message) => {
                            let became_leader = self.handle_follower_message(rpc_message).await;
                            if became_leader {
                                return; // Exit follower loop, will become leader
                            }
                            
                            // Reset election timer on any valid message
                            // (Timer is reset by generating new timeout)
                            election_timeout = self.generate_election_timeout();
                        }
                        Err(_) => {
                            // Transport error - continue
                        }
                    }
                }
            }
        }
    }

    async fn handle_follower_message(&mut self, rpc_req: MessageEnvelope) -> bool {
        match &rpc_req.message {
            RpcMessage::AppendEntries(req) => {
                let response = self.handle_append_entries(req).await;
                let response_msg = MessageEnvelope {
                    to: rpc_req.from,
                    from: self.id,
                    message: RpcMessage::AppendEntriesResponse(response),
                };
                let _ = self.transport.send(response_msg).await;
                false
            }
            RpcMessage::RequestVote(vote_req) => {
                let response = self.handle_request_vote(vote_req).await;
                let response_msg = MessageEnvelope {
                    to: rpc_req.from,
                    from: self.id,
                    message: RpcMessage::RequestVoteResponse(response),
                };
                let _ = self.transport.send(response_msg).await;
                false
            }
            _ => false
        }
    }

    pub async fn handle_append_entries(&mut self, req: &AppendEntriesRequest) -> AppendEntriesResponse {
        // 1. Check term - step down if higher
        if req.term > self.current_term {
            self.current_term = req.term;
            // Update voting state
            let voting_state = crate::storage::VotingState {
                current_term: self.current_term,
                voted_for: None,
            };
            self.storage.persist_voting_state(&voting_state);
        }

        // 2. Reject if term is lower
        if req.term < self.current_term {
            return AppendEntriesResponse {
                term: self.current_term,
                success: false,
                match_index: 0,
                conflict_index: Some(req.prev_log_index),
                conflict_term: Some(req.prev_log_term),
            };
        }

        // 3. Check log consistency (prev_log_index/prev_log_term)
        let log_ok = self.check_log_consistency(req.prev_log_index, req.prev_log_term);

        if !log_ok {
            // Find conflict point
            // Find the first index where we have a conflict
            let conflict_index = if req.prev_log_index > 0 && req.prev_log_index <= self.logs.len() as u64 {
                // We have an entry at prev_log_index, but term doesn't match
                // Find the first index of the conflicting term
                let conflicting_term = self.logs[(req.prev_log_index - 1) as usize].term;
                let mut first_conflict_idx = req.prev_log_index;
                
                // Search backwards to find first entry with this term
                for i in (0..req.prev_log_index as usize).rev() {
                    if i < self.logs.len() && self.logs[i].term == conflicting_term {
                        first_conflict_idx = (i + 1) as u64;
                    } else {
                        break;
                    }
                }
                first_conflict_idx
            } else if req.prev_log_index > self.logs.len() as u64 {
                // Log too short - conflict at end of our log
                self.logs.len() as u64 + 1
            } else {
                // prev_log_index is 0, but we have entries - conflict at index 1
                1
            };
            
            let conflict_term = if conflict_index > 0 && conflict_index <= self.logs.len() as u64 {
                self.logs[(conflict_index - 1) as usize].term
            } else {
                0
            };

            return AppendEntriesResponse {
                term: self.current_term,
                success: false,
                match_index: 0,
                conflict_index: Some(conflict_index),
                conflict_term: Some(conflict_term),
            };
        }

        // 4. Append entries to log (not state machine yet!)
        if !req.entries.is_empty() {
            // Truncate conflicting entries if any
            let start_idx = req.prev_log_index as usize;
            if start_idx < self.logs.len() {
                self.logs.truncate(start_idx);
                self.storage.truncate_from((start_idx + 1) as u64);
            }

            // Append new entries (entries from leader already have correct index)
            for entry in req.entries.iter() {
                self.logs.push(entry.clone());
            }

            // Persist to storage
            let logs_to_store: Vec<LogEntry> = self.logs.iter().cloned().collect();
            self.storage.append_log_entries(logs_to_store);
        }

        // 5. Update commit_index (but don't apply yet)
        if req.leader_commit > self.commit_idx {
            let last_log_idx = self.logs.len() as u64;
            self.commit_idx = min(req.leader_commit, last_log_idx);
            self.storage.persist_commit_idx(self.commit_idx);
        }

        // 6. Apply committed entries to state machine
        self.apply_committed_entries();

        // 7. Return success
        AppendEntriesResponse {
            term: self.current_term,
            success: true,
            match_index: self.logs.len() as u64,
            conflict_index: None,
            conflict_term: None,
        }
    }

    pub async fn handle_request_vote(&mut self, vote_req: &RequestVoteRequest) -> RequestVoteResponse {
        // Check term
        if vote_req.term > self.current_term {
            self.current_term = vote_req.term;
            // Reset voted_for when term increases
            let voting_state = crate::storage::VotingState {
                current_term: self.current_term,
                voted_for: None,
            };
            self.storage.persist_voting_state(&voting_state);
        }

        // Get current voted_for
        let voting_state = self.storage.load_voting_state();
        let voted_for = voting_state.voted_for.map(|id| NodeId(id as u16));

        // Check if we should grant vote
        let vote_granted = if vote_req.term < self.current_term {
            false // Stale term
        } else if let Some(voted) = voted_for {
            voted == vote_req.candidate_id // Already voted for this candidate
        } else {
            // Check if candidate's log is at least as up-to-date
            let (last_log_index, last_log_term) = self.get_last_log_info();
            let candidate_log_ok = vote_req.last_log_term > last_log_term ||
                (vote_req.last_log_term == last_log_term && vote_req.last_log_index >= last_log_index);

            if candidate_log_ok {
                // Grant vote
                let voting_state = crate::storage::VotingState {
                    current_term: self.current_term,
                    voted_for: Some(vote_req.candidate_id.0 as u64),
                };
                self.storage.persist_voting_state(&voting_state);
                true
            } else {
                false
            }
        };

        RequestVoteResponse {
            term: self.current_term,
            vote_granted,
        }
    }

    fn check_log_consistency(&self, prev_log_index: u64, prev_log_term: u64) -> bool {
        // If prev_log_index is 0, log is consistent (no previous entry)
        if prev_log_index == 0 {
            return true;
        }

        // Check if we have an entry at prev_log_index
        if prev_log_index > self.logs.len() as u64 {
            return false; // Log too short
        }

        // Check if term matches
        let entry = &self.logs[(prev_log_index - 1) as usize];
        entry.term == prev_log_term
    }


}