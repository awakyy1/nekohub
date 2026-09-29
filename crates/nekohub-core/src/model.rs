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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageEntry {
    pub name: String,
    pub path: String,
    pub allocated_bytes: u64,
    pub file_count: u64,
    pub is_directory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageSnapshot {
    pub root: String,
    pub total_bytes: u64,
    pub used_bytes: u64,
    pub scanned_bytes: u64,
    pub file_count: u64,
    pub unreadable_entries: u64,
    pub truncated: bool,
    pub elapsed_ms: u64,
    pub entries: Vec<StorageEntry>,
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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub name: String,
    pub command: String,
    pub state: String,
    pub cpu_percent: f64,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ContainerSnapshot {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub state: String,
    pub cpu_percent: f64,
    pub memory_used_bytes: u64,
    pub memory_limit_bytes: u64,
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    pub block_read_bytes: u64,
    pub block_write_bytes: u64,
    pub pids: u64,
}

/// Complete, immutable view produced after normalizing a raw sample.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostSnapshot {
    pub host_id: String,
    #[serde(default)]
    pub agent_version: String,
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
    #[serde(default)]
    pub processes: Vec<ProcessSnapshot>,
    #[serde(default)]
    pub containers: Vec<ContainerSnapshot>,
}
