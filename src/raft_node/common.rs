// Common methods available on all types of nodes
use super::RaftNode;
use crate::{
    message::{MessageEnvelope, RpcMessage}, raft_node::NodeId, state_machine::StateMachine, storage::Storage,
    transport::{Transport, errors::TransportError}
};


impl<S, SM, T> RaftNode<S, SM, T>
where
    S: Storage,
    SM: StateMachine,
    T: Transport
{
    pub async fn receive_rpc(&mut self) -> Result<MessageEnvelope, TransportError> {
        self.transport.receive(self.id).await
    }

    pub async fn send_rpc(&mut self, to: NodeId, message: RpcMessage) -> Result<(), TransportError> {
        let message = MessageEnvelope {
            to,
            from: self.id,
            message
        };
        self.transport.send(message).await
    }

    pub fn get_last_log_info(&self) -> (u64, u64) {
        if self.logs.is_empty() {
            (0, 0)
        } else {
            let last_entry = &self.logs[self.logs.len() - 1];
            (last_entry.index, last_entry.term)
        }
    }

    pub fn apply_committed_entries(&mut self) {
        // Apply entries where commit_idx > last_applied_idx
        while self.last_applied_idx < self.commit_idx {
            self.last_applied_idx += 1;
            if let Some(entry) = self.logs.get((self.last_applied_idx - 1) as usize) {
                // Apply to state machine
                let _ = self.state_machine.set(b"key", &entry.cmd);
            }
        }
        self.storage.persist_last_applied_idx(self.last_applied_idx);
    }

    pub fn generate_election_timeout(&self) -> tokio::time::Duration {
        // Generate random timeout between election_timeout and 2 * election_timeout
        use std::time::SystemTime;
        let seed = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let random_factor = (seed % 1000) as u64;
        let base_timeout = self.config.election_timeout.as_millis() as u64;
        let timeout_ms = base_timeout + (random_factor * base_timeout / 1000);
        tokio::time::Duration::from_millis(timeout_ms)
    }
}