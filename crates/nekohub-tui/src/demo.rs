#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use nekohub_core::{CollectError, Collector, HostTarget, RawHostSample};

#[derive(Debug)]
pub struct DemoCollector {
    started: Instant,
}

impl Default for DemoCollector {
    fn default() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

#[async_trait]
impl Collector for DemoCollector {
    async fn collect(&self, host: &HostTarget) -> Result<RawHostSample, CollectError> {
        tokio::time::sleep(Duration::from_millis(35 + host.id.len() as u64 * 7)).await;
        let elapsed = self.started.elapsed().as_secs_f64().max(1.0);
        let seed = host.id.bytes().map(u64::from).sum::<u64>() % 37;
        let wave = f64::midpoint((elapsed / 3.0 + seed as f64).sin(), 1.0);
        let cpu = 8.0 + wave * (25.0 + seed as f64);
        let total_ticks = (elapsed * 1_000.0) as u64;
        let idle_ticks = (total_ticks as f64 * (1.0 - cpu / 100.0)) as u64;
        let mem_total = 16 * 1024 * 1024 * 1024;
        let mem_used = ((0.28 + wave * 0.35) * mem_total as f64) as u64;
        let disk_total = 240 * 1024 * 1024 * 1024;
        let disk_used = ((0.22 + seed as f64 / 100.0) * disk_total as f64) as u64;

        Ok(RawHostSample {
            host_id: host.id.clone(),
            collected_at: SystemTime::now(),
            latency: Duration::from_millis(12 + seed),
            hostname: host.alias.clone(),
            os: "Demo Linux 1.0".into(),
            kernel: "6.12.0-demo".into(),
            uptime_secs: 86_400 * (3 + seed),
            cpu_total_ticks: total_ticks,
            cpu_idle_ticks: idle_ticks,
            mem_total_bytes: mem_total,
            mem_available_bytes: mem_total - mem_used,
            root_total_bytes: disk_total,
            root_available_bytes: disk_total - disk_used,
            load: [cpu / 30.0, cpu / 35.0, cpu / 40.0],
            network_rx_bytes: (elapsed * (50_000.0 + seed as f64 * 1_000.0)) as u64,
            network_tx_bytes: (elapsed * (18_000.0 + seed as f64 * 700.0)) as u64,
        })
    }
}

pub fn targets() -> Vec<HostTarget> {
    ["atlas", "boreal", "cirrus", "delta", "ember", "fjord"]
        .into_iter()
        .map(HostTarget::from_alias)
        .collect()
}
