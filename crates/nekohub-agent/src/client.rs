use std::{path::PathBuf, time::Duration};

use async_trait::async_trait;
use nekohub_core::{CollectError, Collector, HostTarget, RawHostSample};

#[cfg(unix)]
use crate::protocol::{AgentRequest, AgentResponse, PROTOCOL_VERSION};

#[derive(Debug, Clone)]
pub struct AgentCollector {
    socket_path: PathBuf,
    timeout: Duration,
}

impl AgentCollector {
    pub fn new(socket_path: impl Into<PathBuf>, timeout: Duration) -> Self {
        Self {
            socket_path: socket_path.into(),
            timeout,
        }
    }
}

#[cfg(unix)]
#[async_trait]
impl Collector for AgentCollector {
    async fn collect(&self, _host: &HostTarget) -> Result<RawHostSample, CollectError> {
        use tokio::{
            io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
            net::UnixStream,
        };

        let operation = async {
            let mut stream = UnixStream::connect(&self.socket_path)
                .await
                .map_err(|error| CollectError::Transport(error.to_string()))?;
            let request = serde_json::to_vec(&AgentRequest::Snapshot)
                .map_err(|error| CollectError::Protocol(error.to_string()))?;
            stream
                .write_all(&request)
                .await
                .map_err(|error| CollectError::Transport(error.to_string()))?;
            stream
                .write_all(b"\n")
                .await
                .map_err(|error| CollectError::Transport(error.to_string()))?;

            let mut response = String::new();
            BufReader::new(stream)
                .read_line(&mut response)
                .await
                .map_err(|error| CollectError::Transport(error.to_string()))?;
            match serde_json::from_str::<AgentResponse>(&response)
                .map_err(|error| CollectError::Protocol(error.to_string()))?
            {
                AgentResponse::Sample {
                    protocol, sample, ..
                } if protocol == PROTOCOL_VERSION => Ok(*sample),
                AgentResponse::Error { message, .. } => Err(CollectError::Remote(message)),
                _ => Err(CollectError::Protocol("unsupported agent response".into())),
            }
        };

        tokio::time::timeout(self.timeout, operation)
            .await
            .map_err(|_| CollectError::Timeout)?
    }
}

#[cfg(not(unix))]
#[async_trait]
impl Collector for AgentCollector {
    async fn collect(&self, _host: &HostTarget) -> Result<RawHostSample, CollectError> {
        let _ = (&self.socket_path, self.timeout);
        Err(CollectError::Transport(
            "the nekoHub agent requires a Unix-like system".into(),
        ))
    }
}
