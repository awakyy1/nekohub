use nekohub_core::{HostSnapshot, RawHostSample};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "snake_case")]
pub enum AgentRequest {
    Snapshot,
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
    Error {
        protocol: u16,
        message: String,
    },
}
