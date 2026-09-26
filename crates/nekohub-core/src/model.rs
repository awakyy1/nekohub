use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Stable identity and connection alias for a host.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostTarget {
    pub id: String,
    pub alias: String,
    pub display_name: String,
    pub tags: Vec<String>,
}

impl HostTarget {
    pub fn from_alias(alias: impl Into<String>) -> Self {
        let alias = alias.into();
        Self {
            id: alias.clone(),
            display_name: alias.clone(),
            alias,
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub used: u64,
    pub total: u64,
}

impl Usage {
    #[allow(clippy::cast_precision_loss)]
    pub fn percent(self) -> Option<f64> {
        (self.total > 0).then(|| self.used as f64 / self.total as f64 * 100.0)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Throughput {
    pub read_per_sec: f64,
    pub write_per_sec: f64,
}

/// Complete, immutable view produced after normalizing a raw sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSnapshot {
    pub host_id: String,
    pub collected_at: SystemTime,
    pub latency_ms: u64,
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub uptime_secs: u64,
    pub cpu_percent: Option<f64>,
    pub memory: Usage,
    pub root_disk: Usage,
    pub load: [f64; 3],
    pub network: Throughput,
}
