mod app;
mod demo;
mod terminal;
mod ui;

use std::{path::PathBuf, sync::Arc, time::Duration};

use app::{App, View};
use clap::Parser;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use fleet_core::{Collector, HostSnapshot, HostTarget, Inventory, RateTracker};
use fleet_ssh::{LocalCollector, OpenSshCollector};
use futures_util::StreamExt;
use tokio::sync::{broadcast, mpsc, watch};

#[derive(Debug, Parser)]
#[command(version, about = "Linux fleet management, designed for the terminal")]
struct Args {
    /// Open the dashboard with deterministic development data.
    #[arg(long)]
    demo: bool,

    /// Collect two samples and print a non-interactive summary.
    #[arg(long)]
    once: bool,

    /// Monitor this machine when used together with --once.
    #[arg(long)]
    local: bool,

    /// OpenSSH alias. Repeat to add choices to the remote machine picker.
    #[arg(long = "host", value_name = "ALIAS")]
    hosts: Vec<String>,

    /// OpenSSH config used for discovery and connection policy.
    #[arg(long, value_name = "PATH")]
    ssh_config: Option<PathBuf>,

    /// Seconds between completed collection attempts.
    #[arg(long, default_value_t = 2, value_parser = clap::value_parser!(u64).range(1..))]
    refresh: u64,

    /// Whole-attempt timeout in seconds.
    #[arg(long, default_value_t = 8, value_parser = clap::value_parser!(u64).range(1..))]
    timeout: u64,
}

enum CollectionEvent {
    Snapshot(HostSnapshot),
    Error { host_id: String, message: String },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let ssh_config = args.ssh_config.clone().unwrap_or_else(default_ssh_config);
    let remote_hosts = discover_remote_hosts(&args, &ssh_config);
    let timeout = Duration::from_secs(args.timeout);

    if args.once {
        let (targets, collector): (Vec<_>, Arc<dyn Collector>) = if args.local {
            (vec![local_target()], Arc::new(LocalCollector::new(timeout)))
        } else if args.demo {
            (demo::targets(), Arc::new(demo::DemoCollector::default()))
        } else {
            (
                remote_hosts,
                Arc::new(OpenSshCollector::new(timeout).with_config_path(&ssh_config)),
            )
        };
        if targets.is_empty() {
            return Err("--once needs --local, --demo, or at least one SSH host".into());
        }
        return run_once(targets, collector).await;
    }

    if args.demo {
        return run_demo(Duration::from_secs(args.refresh)).await;
    }

    run_tui(
        remote_hosts,
        ssh_config,
        timeout,
        Duration::from_secs(args.refresh),
    )
    .await
}

fn discover_remote_hosts(args: &Args, ssh_config: &std::path::Path) -> Vec<HostTarget> {
    if !args.hosts.is_empty() {
        return args
            .hosts
            .iter()
            .cloned()
            .map(HostTarget::from_alias)
            .collect();
    }
    Inventory::from_ssh_config(ssh_config)
        .map(|inventory| inventory.hosts)
        .unwrap_or_default()
}

fn local_target() -> HostTarget {
    HostTarget {
        id: "local".into(),
        alias: "localhost".into(),
        display_name: "This machine".into(),
        tags: vec!["local".into()],
    }
}

fn default_ssh_config() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".ssh")
        .join("config")
}

async fn run_once(
    targets: Vec<HostTarget>,
    collector: Arc<dyn Collector>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut jobs = tokio::task::JoinSet::new();
    for target in targets {
        let collector = Arc::clone(&collector);
        jobs.spawn(async move {
            let mut tracker = RateTracker::default();
            let first = collector.collect(&target).await?;
            tracker.apply(first);
            tokio::time::sleep(Duration::from_millis(350)).await;
            let second = collector.collect(&target).await?;
            Ok::<_, fleet_core::CollectError>((target, tracker.apply(second)))
        });
    }
    let mut failed = false;
    while let Some(result) = jobs.join_next().await {
        match result? {
            Ok((target, snapshot)) => {
                let cpu = snapshot
                    .cpu_percent
                    .map_or_else(|| "--".into(), |value| format!("{value:.1}%"));
                println!(
                    "{:<24} online  cpu {:>6}  mem {:>5.1}%  root {:>5.1}%  {:>4}ms",
                    target.display_name,
                    cpu,
                    snapshot.memory.percent().unwrap_or_default(),
                    snapshot.root_disk.percent().unwrap_or_default(),
                    snapshot.latency_ms
                );
            }
            Err(error) => {
                failed = true;
                eprintln!("offline: {error}");
            }
        }
    }
    if failed {
        Err("one or more hosts failed".into())
    } else {
        Ok(())
    }
}

async fn run_demo(refresh_every: Duration) -> Result<(), Box<dyn std::error::Error>> {
    let targets = demo::targets();
    let collector: Arc<dyn Collector> = Arc::new(demo::DemoCollector::default());
    let mut app = App::monitoring(targets.clone());
    run_event_loop(
        &mut app,
        Some((targets, collector)),
        None,
        None,
        refresh_every,
    )
    .await
}

async fn run_tui(
    remote_hosts: Vec<HostTarget>,
    ssh_config: PathBuf,
    timeout: Duration,
    refresh_every: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let local: Arc<dyn Collector> = Arc::new(LocalCollector::new(timeout));
    let remote: Arc<dyn Collector> =
        Arc::new(OpenSshCollector::new(timeout).with_config_path(ssh_config));
    let mut app = App::new(remote_hosts);
    run_event_loop(&mut app, None, Some(local), Some(remote), refresh_every).await
}

async fn run_event_loop(
    app: &mut App,
    initial: Option<(Vec<HostTarget>, Arc<dyn Collector>)>,
    local_collector: Option<Arc<dyn Collector>>,
    remote_collector: Option<Arc<dyn Collector>>,
    refresh_every: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let (event_tx, mut event_rx) = mpsc::channel(32);
    let (refresh_tx, _) = broadcast::channel(1);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut workers = tokio::task::JoinSet::new();

    if let Some((targets, collector)) = initial {
        for target in targets {
            spawn_worker(
                &mut workers,
                target,
                Arc::clone(&collector),
                refresh_every,
                &event_tx,
                &refresh_tx,
                &shutdown_rx,
            );
        }
    }

    let mut terminal = terminal::TerminalGuard::enter()?;
    let mut input = EventStream::new();
    let mut redraw = tokio::time::interval(Duration::from_millis(90));
    redraw.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let result = loop {
        tokio::select! {
            _ = redraw.tick() => {
                app.animation_tick = app.animation_tick.wrapping_add(1);
                if let Err(error) = terminal.terminal_mut().draw(|frame| ui::render(frame, app)) {
                    break Err(error.into());
                }
            }
            Some(event) = event_rx.recv() => match event {
                CollectionEvent::Snapshot(snapshot) => app.apply_snapshot(snapshot),
                CollectionEvent::Error { host_id, message } => app.apply_error(&host_id, message),
            },
            Some(input) = input.next() => match input? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if matches!(key.code, KeyCode::Char('q'))
                        || matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break Ok(());
                    }
                    match app.view {
                        View::Welcome => match key.code {
                            KeyCode::Up | KeyCode::Down | KeyCode::Tab | KeyCode::BackTab
                            | KeyCode::Char('j') | KeyCode::Char('k') => app.toggle_welcome_choice(),
                            KeyCode::Enter if app.welcome_selected == 0 => {
                                if let Some(collector) = local_collector.as_ref() {
                                    let target = local_target();
                                    app.start_monitoring(target.clone());
                                    spawn_worker(
                                        &mut workers, target, Arc::clone(collector), refresh_every,
                                        &event_tx, &refresh_tx, &shutdown_rx,
                                    );
                                }
                            }
                            KeyCode::Enter => {
                                if app.remote_hosts.is_empty() {
                                    app.welcome_notice = Some(
                                        "No concrete hosts were found in ~/.ssh/config.".into()
                                    );
                                } else {
                                    app.open_remote_picker();
                                }
                            }
                            _ => {}
                        },
                        View::RemotePicker => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next_remote(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_remote(),
                            KeyCode::Esc => app.open_welcome(),
                            KeyCode::Enter => {
                                if let (Some(target), Some(collector)) =
                                    (app.selected_remote(), remote_collector.as_ref())
                                {
                                    app.start_monitoring(target.clone());
                                    spawn_worker(
                                        &mut workers, target, Arc::clone(collector), refresh_every,
                                        &event_tx, &refresh_tx, &shutdown_rx,
                                    );
                                }
                            }
                            _ => {}
                        },
                        View::Overview => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous(),
                            KeyCode::Enter => app.open_detail(),
                            KeyCode::Char('r') => { let _ = refresh_tx.send(()); },
                            _ => {}
                        },
                        View::Detail => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous(),
                            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => app.open_overview(),
                            KeyCode::Char('r') => { let _ = refresh_tx.send(()); },
                            _ => {}
                        },
                    }
                }
                _ => {}
            },
        }
    };

    let _ = shutdown_tx.send(true);
    while workers.join_next().await.is_some() {}
    result
}

#[allow(clippy::too_many_arguments)]
fn spawn_worker(
    workers: &mut tokio::task::JoinSet<()>,
    target: HostTarget,
    collector: Arc<dyn Collector>,
    refresh_every: Duration,
    event_tx: &mpsc::Sender<CollectionEvent>,
    refresh_tx: &broadcast::Sender<()>,
    shutdown_rx: &watch::Receiver<bool>,
) {
    workers.spawn(host_worker(
        target,
        collector,
        refresh_every,
        event_tx.clone(),
        refresh_tx.subscribe(),
        shutdown_rx.clone(),
    ));
}

async fn host_worker(
    target: HostTarget,
    collector: Arc<dyn Collector>,
    refresh_every: Duration,
    event_tx: mpsc::Sender<CollectionEvent>,
    mut refresh_rx: broadcast::Receiver<()>,
    mut shutdown_rx: watch::Receiver<bool>,
) {
    let mut tracker = RateTracker::default();
    loop {
        let result = collector.collect(&target).await;
        let event = match result {
            Ok(raw) => CollectionEvent::Snapshot(tracker.apply(raw)),
            Err(error) => CollectionEvent::Error {
                host_id: target.id.clone(),
                message: error.to_string(),
            },
        };
        if event_tx.send(event).await.is_err() {
            return;
        }

        tokio::select! {
            _ = tokio::time::sleep(refresh_every) => {}
            _ = refresh_rx.recv() => {}
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    return;
                }
            }
        }
    }
}
