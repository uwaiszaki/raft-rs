use crate::node::NodeId;

pub struct Follower {
    pub voted_for: Option<NodeId>,
}