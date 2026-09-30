#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::time::{Duration, Instant, SystemTime};

use async_trait::async_trait;
use nekohub_core::{
    CollectError, Collector, ContainerSnapshot, HostTarget, RawHostSample, RawProcessSample,
    StorageEntry, StorageSnapshot,
};

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
            agent_version: env!("CARGO_PKG_VERSION").into(),
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
            processes: demo_processes(elapsed, seed),
            containers: demo_containers(wave, seed),
        })
    }

    async fn collect_storage(
        &self,
        _host: &HostTarget,
        path: &str,
    ) -> Result<StorageSnapshot, CollectError> {
        tokio::time::sleep(Duration::from_millis(420)).await;
        let gib = 1024 * 1024 * 1024;
        let items: &[(&str, u64, u64, bool)] = match path {
            "/var" => &[
                ("lib", 9, 28_491, true),
                ("log", 5, 8_217, true),
                ("cache", 3, 10_991, true),
                ("tmp", 1, 592, true),
            ],
            "/var/lib" => &[
                ("docker", 5, 14_890, true),
                ("postgresql", 2, 8_410, true),
                ("apt", 1, 3_180, true),
                ("dpkg", 1, 2_011, true),
            ],
            _ => &[
                ("home", 42, 126_482, true),
                ("usr", 26, 82_104, true),
                ("var", 18, 48_291, true),
                ("opt", 8, 11_804, true),
                ("swapfile", 6, 1, false),
                ("boot", 2, 743, true),
                ("etc", 1, 4_921, true),
            ],
        };
        let entries = items
            .iter()
            .copied()
            .map(|(name, size, file_count, is_directory)| StorageEntry {
                name: name.into(),
                path: if path == "/" {
                    format!("/{name}")
                } else {
                    format!("{}/{name}", path.trim_end_matches('/'))
                },
                allocated_bytes: size * gib,
                file_count,
                is_directory,
            })
            .collect::<Vec<_>>();
        Ok(StorageSnapshot {
            root: path.into(),
            total_bytes: 240 * gib,
            used_bytes: 109 * gib,
            scanned_bytes: entries.iter().map(|entry| entry.allocated_bytes).sum(),
            file_count: entries.iter().map(|entry| entry.file_count).sum(),
            unreadable_entries: 0,
            truncated: false,
            elapsed_ms: 420,
            entries,
        })
    }
}

fn demo_processes(elapsed: f64, seed: u64) -> Vec<RawProcessSample> {
    [
        (912, "postgres", "postgres: writer", "S", 680_u64, 420_u64),
        (1440, "nginx", "nginx: worker process", "S", 410, 86),
        (2217, "node", "node /srv/api/server.js", "R", 950, 310),
        (2841, "redis-server", "redis-server *:6379", "S", 260, 124),
        (3200, "nekohub-agent", "nekohub-agent", "S", 180, 24),
    ]
    .into_iter()
    .map(
        |(pid, name, command, state, activity, memory_mib)| RawProcessSample {
            pid,
            name: name.into(),
            command: command.into(),
            state: state.into(),
            cpu_ticks: (elapsed * (activity + seed) as f64) as u64,
            memory_bytes: memory_mib * 1024 * 1024,
        },
    )
    .collect()
}

fn demo_containers(wave: f64, seed: u64) -> Vec<ContainerSnapshot> {
    [
        ("web", "a13f52ce", 4.0, 380_u64, 1024_u64, 8_u64),
        ("database", "c94a01bd", 7.5, 920, 2048, 18),
        ("cache", "ff12b890", 1.2, 110, 512, 5),
    ]
    .into_iter()
    .map(
        |(name, id, cpu, used_mib, limit_mib, pids)| ContainerSnapshot {
            id: id.into(),
            name: name.into(),
            engine: "docker".into(),
            state: "running".into(),
            cpu_percent: cpu + wave * seed as f64 / 5.0,
            memory_used_bytes: used_mib * 1024 * 1024,
            memory_limit_bytes: limit_mib * 1024 * 1024,
            network_rx_bytes: (wave * 18_000_000.0) as u64,
            network_tx_bytes: (wave * 7_000_000.0) as u64,
            block_read_bytes: (wave * 80_000_000.0) as u64,
            block_write_bytes: (wave * 24_000_000.0) as u64,
            pids,
        },
    )
    .collect()
}

pub fn targets() -> Vec<HostTarget> {
    ["atlas", "boreal", "cirrus", "delta", "ember", "fjord"]
        .into_iter()
        .map(HostTarget::from_alias)
        .collect()
}
