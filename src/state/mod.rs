mod follower;
mod leader;
mod candidate;

pub use leader::Leader;
pub use follower::Follower;
pub use candidate::Candidate;

pub enum NodeState {
    Leader(Leader),
    Follower(Follower),
    Candidate(Candidate)
}

impl NodeState {
    pub fn is_leader(&self) -> bool {
        matches!(self, NodeState::Leader(_))
    }

    pub fn is_follower(&self) -> bool {
        matches!(self, NodeState::Follower(_))
    }

    pub fn is_candidater(&self) -> bool {
        matches!(self, NodeState::Candidate(_))
    }
}