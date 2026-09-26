use std::{
    fs,
    path::{Path, PathBuf},
    time::{Instant, SystemTime},
};

use nekohub_core::{CollectError, RawHostSample};

#[derive(Debug, Clone)]
pub struct NativeLinuxCollector {
    procfs: PathBuf,
    sysfs: PathBuf,
    etc_dir: PathBuf,
    root_filesystem: PathBuf,
}

impl Default for NativeLinuxCollector {
    fn default() -> Self {
        Self {
            procfs: "/proc".into(),
            sysfs: "/sys".into(),
            etc_dir: "/etc".into(),
            root_filesystem: "/".into(),
        }
    }
}

impl NativeLinuxCollector {
    #[allow(clippy::similar_names)]
    pub fn collect(&self, host_id: &str) -> Result<RawHostSample, CollectError> {
        let started = Instant::now();
        let stat = read(self.procfs.join("stat"))?;
        let meminfo = read(self.procfs.join("meminfo"))?;
        let loadavg = read(self.procfs.join("loadavg"))?;
        let uptime = read(self.procfs.join("uptime"))?;
        let (cpu_total_ticks, cpu_idle_ticks) = parse_cpu(&stat)?;
        let (mem_total_bytes, mem_available_bytes) = parse_memory(&meminfo)?;
        let load = parse_load(&loadavg)?;
        let uptime_secs = parse_uptime(&uptime)?;
        let (network_rx_bytes, network_tx_bytes) = network_totals(&self.sysfs)?;
        let root_total_bytes = fs2::total_space(&self.root_filesystem)
            .map_err(|error| source_error("root filesystem total", &error))?;
        let root_available_bytes = fs2::available_space(&self.root_filesystem)
            .map_err(|error| source_error("root filesystem available", &error))?;

        Ok(RawHostSample {
            host_id: host_id.to_owned(),
            collected_at: SystemTime::now(),
            latency: started.elapsed(),
            hostname: read(self.procfs.join("sys/kernel/hostname"))?
                .trim()
                .to_owned(),
            os: read_os_release(&self.etc_dir).unwrap_or_else(|| "Linux".into()),
            kernel: read(self.procfs.join("sys/kernel/osrelease"))?
                .trim()
                .to_owned(),
            uptime_secs,
            cpu_total_ticks,
            cpu_idle_ticks,
            mem_total_bytes,
            mem_available_bytes,
            root_total_bytes,
            root_available_bytes,
            load,
            network_rx_bytes,
            network_tx_bytes,
        })
    }
}

fn read(path: impl AsRef<Path>) -> Result<String, CollectError> {
    let path = path.as_ref();
    fs::read_to_string(path).map_err(|error| {
        CollectError::Transport(format!("cannot read {}: {error}", path.display()))
    })
}

fn source_error(source: &str, error: &std::io::Error) -> CollectError {
    CollectError::Transport(format!("cannot read {source}: {error}"))
}

fn parse_cpu(input: &str) -> Result<(u64, u64), CollectError> {
    let line = input
        .lines()
        .find(|line| line.starts_with("cpu "))
        .ok_or_else(|| CollectError::Protocol("missing aggregate CPU counters".into()))?;
    let values = line
        .split_whitespace()
        .skip(1)
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| CollectError::Protocol("invalid CPU counter".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    if values.len() < 4 {
        return Err(CollectError::Protocol(
            "incomplete aggregate CPU counters".into(),
        ));
    }
    let total = values.iter().copied().sum();
    let idle = values[3].saturating_add(values.get(4).copied().unwrap_or_default());
    Ok((total, idle))
}

fn parse_memory(input: &str) -> Result<(u64, u64), CollectError> {
    let mut total = None;
    let mut available = None;
    for line in input.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let kib = value
            .split_whitespace()
            .next()
            .and_then(|value| value.parse::<u64>().ok());
        match key {
            "MemTotal" => total = kib,
            "MemAvailable" => available = kib,
            _ => {}
        }
    }
    let total = total.ok_or_else(|| CollectError::Protocol("missing MemTotal".into()))?;
    let available =
        available.ok_or_else(|| CollectError::Protocol("missing MemAvailable".into()))?;
    Ok((total.saturating_mul(1024), available.saturating_mul(1024)))
}

fn parse_load(input: &str) -> Result<[f64; 3], CollectError> {
    let values = input
        .split_whitespace()
        .take(3)
        .map(|value| {
            value
                .parse::<f64>()
                .map_err(|_| CollectError::Protocol("invalid load average".into()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    values
        .try_into()
        .map_err(|_| CollectError::Protocol("incomplete load average".into()))
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn parse_uptime(input: &str) -> Result<u64, CollectError> {
    input
        .split_whitespace()
        .next()
        .ok_or_else(|| CollectError::Protocol("missing uptime".into()))?
        .parse::<f64>()
        .map(|value| value.max(0.0).floor() as u64)
        .map_err(|_| CollectError::Protocol("invalid uptime".into()))
}

fn network_totals(sys_root: &Path) -> Result<(u64, u64), CollectError> {
    let mut receive = 0_u64;
    let mut transmit = 0_u64;
    let interfaces = sys_root.join("class/net");
    for entry in fs::read_dir(&interfaces).map_err(|error| {
        CollectError::Transport(format!("cannot read {}: {error}", interfaces.display()))
    })? {
        let entry = entry.map_err(|error| CollectError::Transport(error.to_string()))?;
        if entry.file_name() == "lo" {
            continue;
        }
        let statistics = entry.path().join("statistics");
        receive = receive.saturating_add(parse_counter(&statistics.join("rx_bytes"))?);
        transmit = transmit.saturating_add(parse_counter(&statistics.join("tx_bytes"))?);
    }
    Ok((receive, transmit))
}

fn parse_counter(path: &Path) -> Result<u64, CollectError> {
    read(path)?
        .trim()
        .parse()
        .map_err(|_| CollectError::Protocol(format!("invalid counter in {}", path.display())))
}

fn read_os_release(etc_root: &Path) -> Option<String> {
    let content = fs::read_to_string(etc_root.join("os-release")).ok()?;
    content.lines().find_map(|line| {
        line.strip_prefix("PRETTY_NAME=")
            .map(|value| value.trim_matches('"').replace("\\\"", "\""))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kernel_counters() {
        assert_eq!(parse_cpu("cpu  10 2 3 20 5 1 1 0\n").unwrap(), (42, 25));
        assert_eq!(
            parse_memory("MemTotal: 100 kB\nMemAvailable: 25 kB\n").unwrap(),
            (102_400, 25_600)
        );
        let load = parse_load("0.10 0.20 0.30 1/100 1\n").unwrap();
        assert!((load[0] - 0.1).abs() < f64::EPSILON);
        assert!((load[1] - 0.2).abs() < f64::EPSILON);
        assert!((load[2] - 0.3).abs() < f64::EPSILON);
        assert_eq!(parse_uptime("42.99 10.0\n").unwrap(), 42);
    }

    #[test]
    fn rejects_incomplete_cpu_sample() {
        assert!(parse_cpu("cpu 1 2\n").is_err());
    }
}
