//! Local Linux collector, agent protocol, and TUI client.

mod client;
mod protocol;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
mod prometheus;
#[cfg(target_os = "linux")]
mod server;

pub use client::AgentCollector;
pub use protocol::{AgentRequest, AgentResponse, PROTOCOL_VERSION};

#[cfg(target_os = "linux")]
pub use server::{AgentConfig, run};
