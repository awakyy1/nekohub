use nekohub_core::{HostSnapshot, RawHostSample, StorageSnapshot};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 3;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
pub enum AgentRequest {
    Snapshot,
    Storage { path: String },
    History { limit: Option<usize> },
    Stream,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentResponse {
    Sample {
        protocol: u16,
        sequence: u64,
        sample: Box<RawHostSample>,
    },
    History {
        protocol: u16,
        snapshots: Vec<HostSnapshot>,
    },
    Storage {
        protocol: u16,
        snapshot: Box<StorageSnapshot>,
    },
    Error {
        protocol: u16,
        message: String,
    },
}
