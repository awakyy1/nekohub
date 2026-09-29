use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Instant, SystemTime},
};

use nekohub_core::{CollectError, ContainerSnapshot, RawHostSample, RawProcessSample};

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
        let processes = collect_processes(&self.procfs);
        let containers = collect_containers();

        Ok(RawHostSample {
            host_id: host_id.to_owned(),
            agent_version: env!("CARGO_PKG_VERSION").into(),
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
            processes,
            containers,
        })
    }
}

fn collect_processes(procfs: &Path) -> Vec<RawProcessSample> {
    let Ok(entries) = fs::read_dir(procfs) else {
        return Vec::new();
    };
    let mut processes = entries
        .flatten()
        .filter_map(|entry| {
            let pid = entry.file_name().to_string_lossy().parse::<u32>().ok()?;
            read_process(&entry.path(), pid)
        })
        .collect::<Vec<_>>();
    processes.sort_by(|left, right| {
        right
            .cpu_ticks
            .cmp(&left.cpu_ticks)
            .then_with(|| right.memory_bytes.cmp(&left.memory_bytes))
    });
    processes.truncate(512);
    processes
}

fn read_process(path: &Path, pid: u32) -> Option<RawProcessSample> {
    let stat = fs::read_to_string(path.join("stat")).ok()?;
    let name_start = stat.find('(')?.saturating_add(1);
    let name_end = stat.rfind(')')?;
    let name = stat.get(name_start..name_end)?.to_owned();
    let fields = stat
        .get(name_end.saturating_add(2)..)?
        .split_whitespace()
        .collect::<Vec<_>>();
    let state = fields.first()?.to_string();
    let cpu_ticks = fields
        .get(11)?
        .parse::<u64>()
        .ok()?
        .saturating_add(fields.get(12)?.parse::<u64>().ok()?);
    let memory_bytes = fs::read_to_string(path.join("status"))
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                line.strip_prefix("VmRSS:")?
                    .split_whitespace()
                    .next()?
                    .parse::<u64>()
                    .ok()
            })
        })
        .unwrap_or_default()
        .saturating_mul(1024);
    let command = fs::read(path.join("cmdline"))
        .ok()
        .map(|bytes| {
            String::from_utf8_lossy(&bytes)
                .trim_end_matches('\0')
                .replace('\0', " ")
        })
        .filter(|command| !command.is_empty())
        .unwrap_or_else(|| name.clone());
    Some(RawProcessSample {
        pid,
        name,
        command,
        state,
        cpu_ticks,
        memory_bytes,
    })
}

fn collect_containers() -> Vec<ContainerSnapshot> {
    let mut containers = ["docker", "podman"]
        .into_iter()
        .flat_map(container_stats)
        .collect::<Vec<_>>();
    containers.sort_by(|left, right| right.cpu_percent.total_cmp(&left.cpu_percent));
    containers
}

fn container_stats(engine: &str) -> Vec<ContainerSnapshot> {
    let output = Command::new(engine)
        .args([
            "stats",
            "--no-stream",
            "--format",
            "{{.ID}}\t{{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}\t{{.BlockIO}}\t{{.PIDs}}",
        ])
        .output();
    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| parse_container_stat(engine, line))
        .collect()
}

#[allow(clippy::similar_names)]
fn parse_container_stat(engine: &str, line: &str) -> Option<ContainerSnapshot> {
    let fields = line.split('\t').collect::<Vec<_>>();
    if fields.len() != 7 {
        return None;
    }
    let (memory_used_bytes, memory_limit_bytes) = parse_usage_pair(fields[3]);
    let (network_rx_bytes, network_tx_bytes) = parse_usage_pair(fields[4]);
    let (block_read_bytes, block_write_bytes) = parse_usage_pair(fields[5]);
    Some(ContainerSnapshot {
        id: fields[0].to_owned(),
        name: fields[1].to_owned(),
        engine: engine.to_owned(),
        state: "running".into(),
        cpu_percent: fields[2].trim_end_matches('%').parse().unwrap_or_default(),
        memory_used_bytes,
        memory_limit_bytes,
        network_rx_bytes,
        network_tx_bytes,
        block_read_bytes,
        block_write_bytes,
        pids: fields[6].parse().unwrap_or_default(),
    })
}

fn parse_usage_pair(value: &str) -> (u64, u64) {
    let mut values = value.split('/').map(|part| parse_size(part.trim()));
    (
        values.next().unwrap_or_default(),
        values.next().unwrap_or_default(),
    )
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn parse_size(value: &str) -> u64 {
    const UNITS: [(&str, f64); 9] = [
        ("TiB", 1_099_511_627_776.0),
        ("GiB", 1_073_741_824.0),
        ("MiB", 1_048_576.0),
        ("KiB", 1_024.0),
        ("TB", 1_000_000_000_000.0),
        ("GB", 1_000_000_000.0),
        ("MB", 1_000_000.0),
        ("kB", 1_000.0),
        ("B", 1.0),
    ];
    for (suffix, multiplier) in UNITS {
        if let Some(number) = value.strip_suffix(suffix) {
            return number.trim().parse::<f64>().map_or(0, |number| {
                (number * multiplier).clamp(0.0, u64::MAX as f64) as u64
            });
        }
    }
    0
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

    #[test]
    fn parses_container_resource_units() {
        let container = parse_container_stat(
            "docker",
            "a1b2c3\tapi\t12.5%\t256MiB / 1GiB\t1.5MB / 800kB\t12MB / 4MB\t9",
        )
        .unwrap();

        assert_eq!(container.name, "api");
        assert_eq!(container.memory_used_bytes, 256 * 1024 * 1024);
        assert_eq!(container.memory_limit_bytes, 1024 * 1024 * 1024);
        assert_eq!(container.pids, 9);
    }
}
