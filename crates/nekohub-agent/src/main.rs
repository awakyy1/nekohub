#[cfg(target_os = "linux")]
use std::{net::SocketAddr, path::PathBuf, time::Duration};

#[cfg(target_os = "linux")]
use clap::Parser;

#[cfg(target_os = "linux")]
#[derive(Debug, Parser)]
#[command(
    name = "nekohub-agent",
    version,
    about = "Native Linux metrics agent for nekoHub"
)]
struct Args {
    #[arg(long, default_value = "/run/nekohub/agent.sock")]
    socket: PathBuf,

    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u64).range(1..))]
    interval: u64,

    #[arg(long, default_value_t = 1_800)]
    history: usize,

    #[arg(long, default_value = "127.0.0.1:9876")]
    metrics_listen: SocketAddr,
}

#[cfg(target_os = "linux")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    nekohub_agent::run(nekohub_agent::AgentConfig {
        socket_path: args.socket,
        sample_interval: Duration::from_secs(args.interval),
        history_capacity: args.history,
        metrics_listen: args.metrics_listen,
    })
    .await
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("nekohub-agent currently supports Linux only");
    std::process::exit(1);
}
