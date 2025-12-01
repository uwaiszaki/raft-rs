use std::collections::HashMap;
use crate::node::NodeId;


pub struct Leader {
    pub next_index: HashMap<NodeId, usize>,
    pub match_index: HashMap<NodeId, usize>,
}