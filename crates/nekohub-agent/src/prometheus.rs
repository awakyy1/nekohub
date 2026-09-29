use std::{sync::Arc, time::UNIX_EPOCH};

use nekohub_core::HostSnapshot;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::RwLock,
};

use crate::server::AgentState;

pub async fn serve(listener: TcpListener, state: Arc<RwLock<AgentState>>) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(error) = handle(stream, state).await {
                eprintln!("metrics request failed: {error}");
            }
        });
    }
}

async fn handle(mut stream: TcpStream, state: Arc<RwLock<AgentState>>) -> std::io::Result<()> {
    let mut request = [0_u8; 2048];
    let read = stream.read(&mut request).await?;
    let first_line = String::from_utf8_lossy(&request[..read])
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned();
    let (status, content_type, body) = if first_line.starts_with("GET /metrics ") {
        let state = state.read().await;
        let body = state.latest_snapshot.as_ref().map_or_else(
            || "# nekoHub has not collected a sample yet\n".into(),
            format_metrics,
        );
        ("200 OK", "text/plain; version=0.0.4; charset=utf-8", body)
    } else if first_line.starts_with("GET /healthz ") {
        ("200 OK", "text/plain; charset=utf-8", "ok\n".into())
    } else {
        (
            "404 Not Found",
            "text/plain; charset=utf-8",
            "not found\n".into(),
        )
    };
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(response.as_bytes()).await
}

pub fn format_metrics(snapshot: &HostSnapshot) -> String {
    let cpu_ratio = snapshot.cpu_percent.unwrap_or_default() / 100.0;
    let collected = snapshot
        .collected_at
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |duration| duration.as_secs_f64());
    let host = escape_label(&snapshot.hostname);
    format!(
        concat!(
            "# HELP nekohub_agent_info Static information about the nekoHub agent.\n",
            "# TYPE nekohub_agent_info gauge\n",
            "nekohub_agent_info{{host=\"{host}\",version=\"{version}\"}} 1\n",
            "# HELP nekohub_host_cpu_usage_ratio Aggregate CPU utilization ratio.\n",
            "# TYPE nekohub_host_cpu_usage_ratio gauge\n",
            "nekohub_host_cpu_usage_ratio{{host=\"{host}\"}} {cpu_ratio}\n",
            "# HELP nekohub_host_memory_bytes Host memory by state.\n",
            "# TYPE nekohub_host_memory_bytes gauge\n",
            "nekohub_host_memory_bytes{{host=\"{host}\",state=\"used\"}} {memory_used}\n",
            "nekohub_host_memory_bytes{{host=\"{host}\",state=\"total\"}} {memory_total}\n",
            "# HELP nekohub_host_root_filesystem_bytes Root filesystem by state.\n",
            "# TYPE nekohub_host_root_filesystem_bytes gauge\n",
            "nekohub_host_root_filesystem_bytes{{host=\"{host}\",state=\"used\"}} {disk_used}\n",
            "nekohub_host_root_filesystem_bytes{{host=\"{host}\",state=\"total\"}} {disk_total}\n",
            "# HELP nekohub_host_network_bytes_per_second Aggregate network throughput.\n",
            "# TYPE nekohub_host_network_bytes_per_second gauge\n",
            "nekohub_host_network_bytes_per_second{{host=\"{host}\",direction=\"receive\"}} {network_read}\n",
            "nekohub_host_network_bytes_per_second{{host=\"{host}\",direction=\"transmit\"}} {network_write}\n",
            "# HELP nekohub_agent_last_collection_timestamp_seconds Last successful collection time.\n",
            "# TYPE nekohub_agent_last_collection_timestamp_seconds gauge\n",
            "nekohub_agent_last_collection_timestamp_seconds{{host=\"{host}\"}} {collected}\n"
        ),
        host = host,
        cpu_ratio = cpu_ratio,
        collected = collected,
        version = env!("CARGO_PKG_VERSION"),
        memory_used = snapshot.memory.used,
        memory_total = snapshot.memory.total,
        disk_used = snapshot.root_disk.used,
        disk_total = snapshot.root_disk.total,
        network_read = snapshot.network.read_per_sec,
        network_write = snapshot.network.write_per_sec,
    )
}

fn escape_label(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use std::time::SystemTime;

    use nekohub_core::{Throughput, Usage};

    use super::*;

    #[test]
    fn exposes_prometheus_metrics() {
        let snapshot = HostSnapshot {
            host_id: "local".into(),
            agent_version: env!("CARGO_PKG_VERSION").into(),
            collected_at: SystemTime::UNIX_EPOCH,
            latency_ms: 1,
            hostname: "demo".into(),
            os: "Linux".into(),
            kernel: "6.x".into(),
            uptime_secs: 10,
            cpu_percent: Some(25.0),
            memory: Usage {
                used: 25,
                total: 100,
            },
            root_disk: Usage {
                used: 10,
                total: 100,
            },
            load: [0.1, 0.2, 0.3],
            network: Throughput {
                read_per_sec: 12.0,
                write_per_sec: 4.0,
            },
            processes: Vec::new(),
            containers: Vec::new(),
        };
        let output = format_metrics(&snapshot);
        assert!(output.contains("nekohub_host_cpu_usage_ratio{host=\"demo\"} 0.25"));
        assert!(output.contains("state=\"used\"} 25"));
    }
}
