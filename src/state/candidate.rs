use crate::raft_node::NodeId;

pub struct Candidate {
    pub votes_received: Vec<NodeId>,
}