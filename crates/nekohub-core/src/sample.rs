use std::time::{Duration, SystemTime};

use crate::{HostSnapshot, Throughput, Usage};
use serde::{Deserialize, Serialize};

/// Cumulative counters and point-in-time gauges returned by the remote probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawHostSample {
    pub host_id: String,
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

        let snapshot = HostSnapshot {
            host_id: raw.host_id.clone(),
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
        tracker.apply(raw(1, 100, 50, 10));
        let snapshot = tracker.apply(raw(3, 300, 130, 210));
        assert_eq!(snapshot.cpu_percent, Some(60.0));
        assert!((snapshot.network.read_per_sec - 100.0).abs() < f64::EPSILON);
        assert_eq!(snapshot.memory.percent(), Some(75.0));
    }
}
