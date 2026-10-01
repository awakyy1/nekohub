use std::{path::PathBuf, time::Duration};

use async_trait::async_trait;
use nekohub_core::{CollectError, Collector, HostTarget, RawHostSample};

#[cfg(unix)]
use nekohub_core::StorageSnapshot;

#[cfg(unix)]
use crate::protocol::{AgentRequest, AgentResponse, PROTOCOL_VERSION};

#[derive(Debug, Clone)]
pub struct AgentCollector {
    socket_path: PathBuf,
    timeout: Duration,
}

pub struct SshAgentCollector {
    target: String,
    password: Option<String>,
    socket_path: PathBuf,
    timeout: Duration,
    #[cfg(unix)]
    tunnel: tokio::sync::Mutex<Option<tokio::process::Child>>,
}

impl SshAgentCollector {
    pub fn new(target: impl Into<String>, password: Option<String>, timeout: Duration) -> Self {
        let target = target.into();
        let safe_target = target
            .chars()
            .map(|character| {
                if character.is_ascii_alphanumeric() {
                    character
                } else {
                    '-'
                }
            })
            .collect::<String>();
        Self {
            target,
            password,
            socket_path: std::env::temp_dir()
                .join(format!("nekohub-{}-{safe_target}.sock", std::process::id())),
            timeout,
            #[cfg(unix)]
            tunnel: tokio::sync::Mutex::new(None),
        }
    }
}

#[cfg(unix)]
fn tunnel_error(error: &CollectError) -> bool {
    matches!(
        error,
        CollectError::Timeout | CollectError::Transport(_) | CollectError::Protocol(_)
    )
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
impl AgentCollector {
    async fn request(
        &self,
        request: AgentRequest,
        timeout: Duration,
    ) -> Result<AgentResponse, CollectError> {
        use tokio::{
            io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
            net::UnixStream,
        };

        let operation = async {
            let mut stream = UnixStream::connect(&self.socket_path)
                .await
                .map_err(|error| CollectError::Transport(error.to_string()))?;
            let request = serde_json::to_vec(&request)
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
            serde_json::from_str(&response)
                .map_err(|error| CollectError::Protocol(error.to_string()))
        };
        tokio::time::timeout(timeout, operation)
            .await
            .map_err(|_| CollectError::Timeout)?
    }
}

#[cfg(unix)]
#[async_trait]
impl Collector for AgentCollector {
    async fn collect(&self, _host: &HostTarget) -> Result<RawHostSample, CollectError> {
        match self.request(AgentRequest::Snapshot, self.timeout).await? {
            AgentResponse::Sample {
                protocol, sample, ..
            } if protocol == PROTOCOL_VERSION => Ok(*sample),
            AgentResponse::Error { message, .. } => Err(CollectError::Remote(message)),
            _ => Err(CollectError::Protocol("unsupported agent response".into())),
        }
    }

    async fn collect_storage(
        &self,
        _host: &HostTarget,
        path: &str,
    ) -> Result<StorageSnapshot, CollectError> {
        match self
            .request(
                AgentRequest::Storage { path: path.into() },
                self.timeout.max(Duration::from_secs(120)),
            )
            .await?
        {
            AgentResponse::Storage { protocol, snapshot } if protocol == PROTOCOL_VERSION => {
                Ok(*snapshot)
            }
            AgentResponse::Error { message, .. } => Err(CollectError::Remote(message)),
            _ => Err(CollectError::Protocol("unsupported agent response".into())),
        }
    }
}

#[cfg(unix)]
impl SshAgentCollector {
    async fn reset_tunnel(&self) {
        let mut tunnel = self.tunnel.lock().await;
        if let Some(mut child) = tunnel.take() {
            let _ = child.start_kill();
        }
        let _ = std::fs::remove_file(&self.socket_path);
    }

    async fn ensure_tunnel(&self) -> Result<(), CollectError> {
        use std::process::Stdio;

        let mut tunnel = self.tunnel.lock().await;
        let running = match tunnel.as_mut() {
            Some(child) => child
                .try_wait()
                .map_err(|error| CollectError::Transport(error.to_string()))?
                .is_none(),
            None => false,
        };
        if running {
            return Ok(());
        }
        let _ = std::fs::remove_file(&self.socket_path);
        let mut command = if self.password.is_some() {
            let mut command = tokio::process::Command::new("sshpass");
            command.arg("-e").arg("ssh");
            command
        } else {
            tokio::process::Command::new("ssh")
        };
        if let Some(password) = self.password.as_deref() {
            command.env("SSHPASS", password);
        }
        let child = command
            .args([
                "-N",
                "-T",
                "-o",
                "ExitOnForwardFailure=yes",
                "-o",
                "ServerAliveInterval=5",
                "-o",
                "ServerAliveCountMax=2",
                "-o",
                "TCPKeepAlive=yes",
                "-o",
                "StrictHostKeyChecking=accept-new",
                "-L",
            ])
            .arg(format!(
                "{}:/run/nekohub/agent.sock",
                self.socket_path.display()
            ))
            .arg(&self.target)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| {
                CollectError::Transport(format!("could not start SSH tunnel: {error}"))
            })?;
        *tunnel = Some(child);
        let ready = async {
            loop {
                if self.socket_path.exists() {
                    return Ok(());
                }
                if let Some(status) = tunnel
                    .as_mut()
                    .expect("tunnel was just started")
                    .try_wait()
                    .map_err(|error| CollectError::Transport(error.to_string()))?
                {
                    return Err(CollectError::Transport(format!(
                        "SSH tunnel closed with {status}; check credentials and remote agent status"
                    )));
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        };
        let result = match tokio::time::timeout(self.timeout, ready).await {
            Ok(result) => result,
            Err(_) => Err(CollectError::Timeout),
        };
        if result.is_err() {
            if let Some(mut child) = tunnel.take() {
                let _ = child.start_kill();
            }
            let _ = std::fs::remove_file(&self.socket_path);
        }
        result
    }

    async fn collect_sample(&self, host: &HostTarget) -> Result<RawHostSample, CollectError> {
        self.ensure_tunnel().await?;
        AgentCollector::new(&self.socket_path, self.timeout)
            .collect(host)
            .await
    }

    async fn collect_storage_snapshot(
        &self,
        host: &HostTarget,
        path: &str,
    ) -> Result<StorageSnapshot, CollectError> {
        self.ensure_tunnel().await?;
        AgentCollector::new(&self.socket_path, self.timeout)
            .collect_storage(host, path)
            .await
    }
}

#[cfg(unix)]
#[async_trait]
impl Collector for SshAgentCollector {
    async fn collect(&self, host: &HostTarget) -> Result<RawHostSample, CollectError> {
        let mut sample = match self.collect_sample(host).await {
            Ok(sample) => sample,
            Err(error) if tunnel_error(&error) => {
                self.reset_tunnel().await;
                self.collect_sample(host).await?
            }
            Err(error) => return Err(error),
        };
        sample.host_id.clone_from(&host.id);
        Ok(sample)
    }

    async fn collect_storage(
        &self,
        host: &HostTarget,
        path: &str,
    ) -> Result<StorageSnapshot, CollectError> {
        match self.collect_storage_snapshot(host, path).await {
            Ok(snapshot) => Ok(snapshot),
            Err(error) if tunnel_error(&error) => {
                self.reset_tunnel().await;
                self.collect_storage_snapshot(host, path).await
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(not(unix))]
#[async_trait]
impl Collector for SshAgentCollector {
    async fn collect(&self, _host: &HostTarget) -> Result<RawHostSample, CollectError> {
        let _ = (
            &self.target,
            &self.password,
            &self.socket_path,
            self.timeout,
        );
        Err(CollectError::Transport(
            "SSH agent tunnels require a Unix-like system".into(),
        ))
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
