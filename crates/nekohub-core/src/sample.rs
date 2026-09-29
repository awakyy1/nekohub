use std::time::{Duration, SystemTime};

use crate::{ContainerSnapshot, HostSnapshot, ProcessSnapshot, Throughput, Usage};
use serde::{Deserialize, Serialize};

/// Cumulative counters and point-in-time gauges returned by the remote probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawHostSample {
    pub host_id: String,
    #[serde(default)]
    pub agent_version: String,
    pub collected_at: SystemTime,
    pub latency: Duration,
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub uptime_secs: u64,
    pub cpu_total_ticks: u64,
    pub cpu_idle_ticks: u64,
    pub mem_total_bytes: u64,
    pub mem_available_bytes: u64,
    pub root_total_bytes: u64,
    pub root_available_bytes: u64,
    pub load: [f64; 3],
    pub network_rx_bytes: u64,
    pub network_tx_bytes: u64,
    #[serde(default)]
    pub processes: Vec<RawProcessSample>,
    #[serde(default)]
    pub containers: Vec<ContainerSnapshot>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RawProcessSample {
    pub pid: u32,
    pub name: String,
    pub command: String,
    pub state: String,
    pub cpu_ticks: u64,
    pub memory_bytes: u64,
}

/// Calculates rates while retaining only the previous successful sample.
#[derive(Debug, Default)]
pub struct RateTracker {
    previous: Option<RawHostSample>,
}

impl RateTracker {
    #[allow(clippy::cast_precision_loss)]
    pub fn apply(&mut self, raw: RawHostSample) -> HostSnapshot {
        let (cpu_percent, network) = self
            .previous
            .as_ref()
            .and_then(|previous| {
                let elapsed = raw
                    .collected_at
                    .duration_since(previous.collected_at)
                    .ok()?
                    .as_secs_f64();
                (elapsed > 0.0).then(|| {
                    let total = raw.cpu_total_ticks.saturating_sub(previous.cpu_total_ticks);
                    let idle = raw.cpu_idle_ticks.saturating_sub(previous.cpu_idle_ticks);
                    let cpu = (total > 0)
                        .then(|| (1.0 - idle as f64 / total as f64).clamp(0.0, 1.0) * 100.0);
                    let throughput = Throughput {
                        read_per_sec: raw
                            .network_rx_bytes
                            .saturating_sub(previous.network_rx_bytes)
                            as f64
                            / elapsed,
                        write_per_sec: raw
                            .network_tx_bytes
                            .saturating_sub(previous.network_tx_bytes)
                            as f64
                            / elapsed,
                    };
                    (cpu, throughput)
                })
            })
            .unwrap_or((None, Throughput::default()));

        let total_tick_delta = self.previous.as_ref().map_or(0, |previous| {
            raw.cpu_total_ticks.saturating_sub(previous.cpu_total_ticks)
        });
        let previous_processes = self.previous.as_ref().map(|previous| {
            previous
                .processes
                .iter()
                .map(|process| (process.pid, process.cpu_ticks))
                .collect::<std::collections::HashMap<_, _>>()
        });
        let mut processes = raw
            .processes
            .iter()
            .map(|process| {
                let cpu_percent = previous_processes
                    .as_ref()
                    .and_then(|previous| previous.get(&process.pid))
                    .filter(|_| total_tick_delta > 0)
                    .map_or(0.0, |previous_ticks| {
                        process.cpu_ticks.saturating_sub(*previous_ticks) as f64
                            / total_tick_delta as f64
                            * 100.0
                    });
                ProcessSnapshot {
                    pid: process.pid,
                    name: process.name.clone(),
                    command: process.command.clone(),
                    state: process.state.clone(),
                    cpu_percent,
                    memory_bytes: process.memory_bytes,
                }
            })
            .collect::<Vec<_>>();
        processes.sort_by(|left, right| {
            right
                .cpu_percent
                .total_cmp(&left.cpu_percent)
                .then_with(|| right.memory_bytes.cmp(&left.memory_bytes))
        });
        processes.truncate(200);

        let snapshot = HostSnapshot {
            host_id: raw.host_id.clone(),
            agent_version: raw.agent_version.clone(),
            collected_at: raw.collected_at,
            latency_ms: u64::try_from(raw.latency.as_millis()).unwrap_or(u64::MAX),
            hostname: raw.hostname.clone(),
            os: raw.os.clone(),
            kernel: raw.kernel.clone(),
            uptime_secs: raw.uptime_secs,
            cpu_percent,
            memory: Usage {
                used: raw.mem_total_bytes.saturating_sub(raw.mem_available_bytes),
                total: raw.mem_total_bytes,
            },
            root_disk: Usage {
                used: raw
                    .root_total_bytes
                    .saturating_sub(raw.root_available_bytes),
                total: raw.root_total_bytes,
            },
            load: raw.load,
            network,
            processes,
            containers: raw.containers.clone(),
        };
        self.previous = Some(raw);
        snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(at: u64, total: u64, idle: u64, rx: u64) -> RawHostSample {
        RawHostSample {
            host_id: "host".into(),
            agent_version: "0.14.1".into(),
            collected_at: SystemTime::UNIX_EPOCH + Duration::from_secs(at),
            latency: Duration::from_millis(12),
            hostname: "host".into(),
            os: "Linux".into(),
            kernel: "6.x".into(),
            uptime_secs: 10,
            cpu_total_ticks: total,
            cpu_idle_ticks: idle,
            mem_total_bytes: 100,
            mem_available_bytes: 25,
            root_total_bytes: 200,
            root_available_bytes: 50,
            load: [0.1, 0.2, 0.3],
            network_rx_bytes: rx,
            network_tx_bytes: 0,
            processes: Vec::new(),
            containers: Vec::new(),
        }
    }

    #[test]
    fn first_sample_has_no_invented_cpu_rate() {
        let mut tracker = RateTracker::default();
        assert_eq!(tracker.apply(raw(1, 100, 50, 10)).cpu_percent, None);
    }

    #[test]
    fn derives_rates_from_successive_samples() {
        let mut tracker = RateTracker::default();
        let mut first = raw(1, 100, 50, 10);
        first.processes.push(RawProcessSample {
            pid: 42,
            name: "worker".into(),
            command: "worker --serve".into(),
            state: "R".into(),
            cpu_ticks: 10,
            memory_bytes: 1024,
        });
        tracker.apply(first);
        let mut second = raw(3, 300, 130, 210);
        second.processes.push(RawProcessSample {
            pid: 42,
            name: "worker".into(),
            command: "worker --serve".into(),
            state: "R".into(),
            cpu_ticks: 50,
            memory_bytes: 2048,
        });
        let snapshot = tracker.apply(second);
        assert_eq!(snapshot.cpu_percent, Some(60.0));
        assert!((snapshot.network.read_per_sec - 100.0).abs() < f64::EPSILON);
        assert_eq!(snapshot.memory.percent(), Some(75.0));
        assert!((snapshot.processes[0].cpu_percent - 20.0).abs() < f64::EPSILON);
        assert_eq!(snapshot.processes[0].memory_bytes, 2048);
    }
}
