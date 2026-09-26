//! OpenSSH adapter and Linux probe protocol.

use std::{collections::HashMap, path::PathBuf, process::Stdio, time::Duration};

use async_trait::async_trait;
use nekohub_core::{CollectError, Collector, HostTarget, RawHostSample};
use tokio::{io::AsyncWriteExt, process::Command, time::Instant};

const PROBE: &str = include_str!("probe.sh");

#[derive(Debug, Clone)]
pub struct OpenSshCollector {
    executable: String,
    config_path: Option<PathBuf>,
    timeout: Duration,
}

/// Executes the same read-only probe directly on the current Linux machine.
#[derive(Debug, Clone)]
pub struct LocalCollector {
    timeout: Duration,
}

impl LocalCollector {
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl OpenSshCollector {
    pub fn new(timeout: Duration) -> Self {
        Self {
            executable: "ssh".into(),
            config_path: None,
            timeout,
        }
    }

    #[must_use]
    pub fn with_executable(mut self, executable: impl Into<String>) -> Self {
        self.executable = executable.into();
        self
    }

    #[must_use]
    pub fn with_config_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.config_path = Some(path.into());
        self
    }
}

#[async_trait]
impl Collector for OpenSshCollector {
    async fn collect(&self, host: &HostTarget) -> Result<RawHostSample, CollectError> {
        let started = Instant::now();
        let mut command = Command::new(&self.executable);
        command.arg("-T");
        if let Some(path) = &self.config_path {
            command.arg("-F").arg(path);
        }
        let mut child = command
            .arg("-o")
            .arg("BatchMode=yes")
            .arg("-o")
            .arg(format!("ConnectTimeout={}", self.timeout.as_secs().max(1)))
            .arg(&host.alias)
            .arg("sh")
            .arg("-s")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| CollectError::Transport(error.to_string()))?;

        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| CollectError::Transport("cannot open ssh stdin".into()))?;
        stdin
            .write_all(PROBE.as_bytes())
            .await
            .map_err(|error| CollectError::Transport(error.to_string()))?;
        drop(stdin);

        let output = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| CollectError::Timeout)?
            .map_err(|error| CollectError::Transport(error.to_string()))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CollectError::Remote(single_line(&stderr)));
        }
        let stdout = String::from_utf8(output.stdout)
            .map_err(|_| CollectError::Protocol("response is not UTF-8".into()))?;
        parse_probe(&host.id, &stdout, started.elapsed())
    }
}

#[async_trait]
impl Collector for LocalCollector {
    async fn collect(&self, host: &HostTarget) -> Result<RawHostSample, CollectError> {
        let started = Instant::now();
        let mut child = Command::new("sh")
            .arg("-s")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|error| CollectError::Transport(error.to_string()))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| CollectError::Transport("cannot open local probe stdin".into()))?;
        stdin
            .write_all(PROBE.as_bytes())
            .await
            .map_err(|error| CollectError::Transport(error.to_string()))?;
        drop(stdin);

        let output = tokio::time::timeout(self.timeout, child.wait_with_output())
            .await
            .map_err(|_| CollectError::Timeout)?
            .map_err(|error| CollectError::Transport(error.to_string()))?;
        if !output.status.success() {
            return Err(CollectError::Remote(single_line(&String::from_utf8_lossy(
                &output.stderr,
            ))));
        }
        let stdout = String::from_utf8(output.stdout)
            .map_err(|_| CollectError::Protocol("response is not UTF-8".into()))?;
        parse_probe(&host.id, &stdout, started.elapsed())
    }
}

fn single_line(value: &str) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.is_empty() {
        "ssh exited without an error message".into()
    } else {
        compact.chars().take(240).collect()
    }
}

/// Parses the versioned key-value response emitted by the Linux probe.
///
/// # Errors
///
/// Returns [`CollectError::Protocol`] when the response has an unsupported
/// version or a required value is missing or malformed.
pub fn parse_probe(
    host_id: &str,
    input: &str,
    latency: Duration,
) -> Result<RawHostSample, CollectError> {
    let fields: HashMap<&str, &str> = input
        .lines()
        .filter_map(|line| line.split_once('='))
        .collect();
    if fields.get("version") != Some(&"1") {
        return Err(CollectError::Protocol(
            "unsupported or missing version".into(),
        ));
    }

    let text = |key: &str| -> Result<String, CollectError> {
        fields
            .get(key)
            .map(|value| (*value).to_owned())
            .ok_or_else(|| CollectError::Protocol(format!("missing {key}")))
    };
    let integer = |key: &str| -> Result<u64, CollectError> {
        fields
            .get(key)
            .ok_or_else(|| CollectError::Protocol(format!("missing {key}")))?
            .parse()
            .map_err(|_| CollectError::Protocol(format!("invalid {key}")))
    };
    let decimal = |key: &str| -> Result<f64, CollectError> {
        fields
            .get(key)
            .ok_or_else(|| CollectError::Protocol(format!("missing {key}")))?
            .parse()
            .map_err(|_| CollectError::Protocol(format!("invalid {key}")))
    };

    Ok(RawHostSample {
        host_id: host_id.to_owned(),
        collected_at: std::time::SystemTime::now(),
        latency,
        hostname: text("hostname")?,
        os: text("os")?,
        kernel: text("kernel")?,
        uptime_secs: integer("uptime_secs")?,
        cpu_total_ticks: integer("cpu_total_ticks")?,
        cpu_idle_ticks: integer("cpu_idle_ticks")?,
        mem_total_bytes: integer("mem_total_bytes")?,
        mem_available_bytes: integer("mem_available_bytes")?,
        root_total_bytes: integer("root_total_bytes")?,
        root_available_bytes: integer("root_available_bytes")?,
        load: [decimal("load_1")?, decimal("load_5")?, decimal("load_15")?],
        network_rx_bytes: integer("network_rx_bytes")?,
        network_tx_bytes: integer("network_tx_bytes")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_versioned_probe() {
        let input = "version=1\nhostname=web-1\nos=Debian 13\nkernel=6.12\
            \nuptime_secs=42\ncpu_total_ticks=100\ncpu_idle_ticks=80\
            \nmem_total_bytes=1000\nmem_available_bytes=400\
            \nroot_total_bytes=2000\nroot_available_bytes=500\
            \nload_1=0.1\nload_5=0.2\nload_15=0.3\
            \nnetwork_rx_bytes=10\nnetwork_tx_bytes=20\n";
        let sample = parse_probe("web", input, Duration::from_millis(5)).unwrap();
        assert_eq!(sample.hostname, "web-1");
        assert_eq!(sample.mem_total_bytes, 1000);
    }

    #[test]
    fn rejects_unknown_protocol() {
        let error = parse_probe("web", "version=2\n", Duration::ZERO).unwrap_err();
        assert!(matches!(error, CollectError::Protocol(_)));
    }
}
