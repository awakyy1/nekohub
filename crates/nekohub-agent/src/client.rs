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

#[cfg(unix)]
#[async_trait]
impl Collector for SshAgentCollector {
    async fn collect(&self, host: &HostTarget) -> Result<RawHostSample, CollectError> {
        use std::process::Stdio;

        let mut tunnel = self.tunnel.lock().await;
        let running = match tunnel.as_mut() {
            Some(child) => child
                .try_wait()
                .map_err(|error| CollectError::Transport(error.to_string()))?
                .is_none(),
            None => false,
        };
        if !running {
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
                    "ServerAliveInterval=15",
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
            tokio::time::timeout(self.timeout, ready)
                .await
                .map_err(|_| CollectError::Timeout)??;
        }
        drop(tunnel);

        let mut sample = AgentCollector::new(&self.socket_path, self.timeout)
            .collect(host)
            .await?;
        sample.host_id.clone_from(&host.id);
        Ok(sample)
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
