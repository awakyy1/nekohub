use std::{
    collections::VecDeque,
    net::SocketAddr,
    os::unix::fs::{FileTypeExt, PermissionsExt},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use nekohub_core::{HostSnapshot, RateTracker, RawHostSample};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, UnixListener, UnixStream},
    sync::{RwLock, broadcast},
};

use crate::{
    linux::NativeLinuxCollector,
    prometheus,
    protocol::{AgentRequest, AgentResponse, PROTOCOL_VERSION},
};

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub socket_path: PathBuf,
    pub sample_interval: Duration,
    pub history_capacity: usize,
    pub metrics_listen: SocketAddr,
}

#[derive(Debug, Default)]
pub(crate) struct AgentState {
    pub sequence: u64,
    pub latest_sample: Option<RawHostSample>,
    pub latest_snapshot: Option<HostSnapshot>,
    pub history: VecDeque<HostSnapshot>,
}

/// Starts metric collection, the local protocol, and the Prometheus endpoint.
///
/// # Errors
///
/// Returns an error when a listener cannot be created or a server task fails.
pub async fn run(config: AgentConfig) -> Result<(), Box<dyn std::error::Error>> {
    prepare_socket(&config.socket_path)?;
    let unix = UnixListener::bind(&config.socket_path)?;
    // The protocol is read-only and exposes data already visible through procfs.
    // A world-readable socket lets local desktop users run the TUI without
    // joining the service account's group.
    std::fs::set_permissions(&config.socket_path, std::fs::Permissions::from_mode(0o666))?;
    let metrics = TcpListener::bind(config.metrics_listen).await?;
    let state = Arc::new(RwLock::new(AgentState::default()));
    let (samples, _) = broadcast::channel(32);

    let collector = collect_loop(
        Arc::clone(&state),
        samples.clone(),
        config.sample_interval,
        config.history_capacity,
    );
    let protocol = serve_protocol(unix, Arc::clone(&state), samples);
    let prometheus = prometheus::serve(metrics, Arc::clone(&state));

    tokio::select! {
        result = collector => result?,
        result = protocol => result?,
        result = prometheus => result?,
        result = tokio::signal::ctrl_c() => result?,
    }

    let _ = std::fs::remove_file(&config.socket_path);
    Ok(())
}

fn prepare_socket(path: &std::path::Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_socket() => std::fs::remove_file(path),
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("refusing to replace non-socket path {}", path.display()),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

async fn collect_loop(
    state: Arc<RwLock<AgentState>>,
    samples: broadcast::Sender<(u64, RawHostSample)>,
    interval: Duration,
    history_capacity: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let collector = NativeLinuxCollector::default();
    let mut tracker = RateTracker::default();
    let mut ticker = tokio::time::interval(interval);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        ticker.tick().await;
        match collector.collect("local") {
            Ok(sample) => {
                let snapshot = tracker.apply(sample.clone());
                let mut current = state.write().await;
                current.sequence = current.sequence.wrapping_add(1);
                current.latest_sample = Some(sample.clone());
                current.latest_snapshot = Some(snapshot.clone());
                current.history.push_back(snapshot);
                while current.history.len() > history_capacity {
                    current.history.pop_front();
                }
                let sequence = current.sequence;
                drop(current);
                let _ = samples.send((sequence, sample));
            }
            Err(error) => eprintln!("collection failed: {error}"),
        }
    }
}

async fn serve_protocol(
    listener: UnixListener,
    state: Arc<RwLock<AgentState>>,
    samples: broadcast::Sender<(u64, RawHostSample)>,
) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let state = Arc::clone(&state);
        let receiver = samples.subscribe();
        tokio::spawn(async move {
            if let Err(error) = handle_client(stream, state, receiver).await {
                eprintln!("agent client failed: {error}");
            }
        });
    }
}

async fn handle_client(
    stream: UnixStream,
    state: Arc<RwLock<AgentState>>,
    mut samples: broadcast::Receiver<(u64, RawHostSample)>,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut request = String::new();
    BufReader::new(reader).read_line(&mut request).await?;
    let request = match serde_json::from_str::<AgentRequest>(&request) {
        Ok(request) => request,
        Err(error) => {
            return write_response(
                &mut writer,
                &AgentResponse::Error {
                    protocol: PROTOCOL_VERSION,
                    message: format!("invalid request: {error}"),
                },
            )
            .await;
        }
    };

    match request {
        AgentRequest::Snapshot => {
            let current = state.read().await;
            let response = current.latest_sample.clone().map_or_else(
                || AgentResponse::Error {
                    protocol: PROTOCOL_VERSION,
                    message: "the first sample is not ready yet".into(),
                },
                |sample| AgentResponse::Sample {
                    protocol: PROTOCOL_VERSION,
                    sequence: current.sequence,
                    sample: Box::new(sample),
                },
            );
            write_response(&mut writer, &response).await
        }
        AgentRequest::History { limit } => {
            let current = state.read().await;
            let limit = limit.unwrap_or(300).min(current.history.len());
            let snapshots = current
                .history
                .iter()
                .skip(current.history.len().saturating_sub(limit))
                .cloned()
                .collect();
            write_response(
                &mut writer,
                &AgentResponse::History {
                    protocol: PROTOCOL_VERSION,
                    snapshots,
                },
            )
            .await
        }
        AgentRequest::Stream => loop {
            match samples.recv().await {
                Ok((sequence, sample)) => {
                    write_response(
                        &mut writer,
                        &AgentResponse::Sample {
                            protocol: PROTOCOL_VERSION,
                            sequence,
                            sample: Box::new(sample),
                        },
                    )
                    .await?;
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => return Ok(()),
            }
        },
    }
}

async fn write_response(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    response: &AgentResponse,
) -> std::io::Result<()> {
    let mut encoded = serde_json::to_vec(response)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    encoded.push(b'\n');
    writer.write_all(&encoded).await
}
