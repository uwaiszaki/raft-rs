use tokio::time::Duration;

pub struct Config {
    pub election_timeout: Duration,
    pub heartbeat_interval: Duration,
    pub max_log_entries: usize,

    // Transfer all read requests to the leader
    pub allow_stale_reads: bool,
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn default() -> Self {
        Self {
            election_timeout: Duration::from_secs(1),
            heartbeat_interval: Duration::from_millis(500),
            max_log_entries: 1000,
            allow_stale_reads: false,
        }
    }

    pub fn with_election_timeout(mut self, election_timeout: Duration) -> Self {
        self.election_timeout = election_timeout;
        self
    }

    pub fn with_heartbeat_interval(mut self, heartbeat_interval: Duration) -> Self {
        self.heartbeat_interval = heartbeat_interval;
        self
    }

    pub fn with_max_log_entries(mut self, max_log_entries: usize) -> Self {
        self.max_log_entries = max_log_entries;
        self
    }
}