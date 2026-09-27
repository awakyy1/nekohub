mod app;
mod demo;
mod groups;
mod onboarding;
mod terminal;
mod ui;

use std::{path::PathBuf, sync::Arc, time::Duration};

use app::{App, HomeFocus, View};
use clap::Parser;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use nekohub_agent::AgentCollector;
use nekohub_core::{Collector, HostSnapshot, HostTarget, Inventory, RateTracker};
use tokio::sync::{broadcast, mpsc, watch};

#[derive(Debug, Parser)]
#[command(
    name = "nekohub",
    version,
    about = "Linux fleet management, designed for the terminal"
)]
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

    /// Local nekoHub agent socket.
    #[arg(long, default_value = "/run/nekohub/agent.sock")]
    agent_socket: PathBuf,

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
    SetupProgress { progress: u16, message: String },
    SetupComplete,
    SetupError { message: String },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let ssh_config = args.ssh_config.clone().unwrap_or_else(default_ssh_config);
    let remote_hosts = discover_remote_hosts(&args, &ssh_config);
    let timeout = Duration::from_secs(args.timeout);

    if args.once {
        let (targets, collector): (Vec<_>, Arc<dyn Collector>) = if args.local {
            (
                vec![local_target()],
                Arc::new(AgentCollector::new(args.agent_socket.clone(), timeout)),
            )
        } else if args.demo {
            (demo::targets(), Arc::new(demo::DemoCollector::default()))
        } else {
            return Err(
                "remote agent pairing is not available yet; use --local or --demo with --once"
                    .into(),
            );
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
        args.agent_socket,
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
            tokio::time::sleep(Duration::from_millis(1_100)).await;
            let second = collector.collect(&target).await?;
            Ok::<_, nekohub_core::CollectError>((target, tracker.apply(second)))
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
        refresh_every,
        onboarding::state_path(),
    )
    .await
}

async fn run_tui(
    remote_hosts: Vec<HostTarget>,
    agent_socket: PathBuf,
    timeout: Duration,
    refresh_every: Duration,
) -> Result<(), Box<dyn std::error::Error>> {
    let local: Arc<dyn Collector> = Arc::new(AgentCollector::new(agent_socket, timeout));
    let state_path = onboarding::state_path();
    let machine_groups = groups::load(&groups::state_path());
    let mut app = if onboarding::is_complete(&state_path) {
        App::home(remote_hosts, machine_groups)
    } else {
        App::new(remote_hosts, machine_groups)
    };
    run_event_loop(&mut app, None, Some(local), refresh_every, state_path).await
}

#[allow(clippy::too_many_lines)]
async fn run_event_loop(
    app: &mut App,
    initial: Option<(Vec<HostTarget>, Arc<dyn Collector>)>,
    local_collector: Option<Arc<dyn Collector>>,
    refresh_every: Duration,
    onboarding_state_path: PathBuf,
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
                app.advance_animation();
                if let Err(error) = terminal.terminal_mut().draw(|frame| ui::render(frame, app)) {
                    break Err(error.into());
                }
            }
            Some(event) = event_rx.recv() => match event {
                CollectionEvent::Snapshot(snapshot) => app.apply_snapshot(snapshot),
                CollectionEvent::Error { host_id, message } => app.apply_error(&host_id, message),
                CollectionEvent::SetupProgress { progress, message } => {
                    app.update_agent_setup(progress, message);
                }
                CollectionEvent::SetupComplete => app.open_home(),
                CollectionEvent::SetupError { message } => app.fail_agent_setup(message),
            },
            Some(input) = input.next() => match input? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if matches!(key.code, KeyCode::Char('q')) && app.view != View::CreateGroup
                        || matches!(key.code, KeyCode::Char('c')) && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break Ok(());
                    }
                    if app.view != View::CreateGroup
                        && matches!(
                            app.view,
                            View::Home
                                | View::RemotePicker
                                | View::Settings
                                | View::Overview
                                | View::Detail
                        )
                    {
                        match key.code {
                            KeyCode::Char('1') => {
                                app.open_home();
                                continue;
                            }
                            KeyCode::Char('2') if app.view != View::RemotePicker => {
                                app.open_remote_picker();
                                continue;
                            }
                            KeyCode::Char('3') => {
                                app.open_settings();
                                continue;
                            }
                            _ => {}
                        }
                    }
                    match app.view {
                        View::Welcome => match key.code {
                            KeyCode::Up | KeyCode::Down | KeyCode::Tab | KeyCode::BackTab
                            | KeyCode::Char('j' | 'k') => app.toggle_welcome_choice(),
                            KeyCode::Enter if app.welcome_selected == 0 => {
                                app.open_agent_confirmation();
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
                        View::AgentConfirm => match key.code {
                            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down
                            | KeyCode::Tab | KeyCode::BackTab | KeyCode::Char('h' | 'j' | 'k' | 'l') => {
                                app.toggle_agent_confirmation();
                            }
                            KeyCode::Enter if app.agent_confirm_selected == 0 => {
                                if let Some(collector) = local_collector.as_ref() {
                                    begin_agent_setup(
                                        app, &mut workers, Arc::clone(collector), &event_tx,
                                        onboarding_state_path.clone(),
                                    );
                                }
                            }
                            KeyCode::Enter | KeyCode::Esc => app.open_welcome(),
                            _ => {}
                        },
                        View::AgentSetup => match key.code {
                            KeyCode::Enter if app.setup_error.is_some() => {
                                if let Some(collector) = local_collector.as_ref() {
                                    begin_agent_setup(
                                        app, &mut workers, Arc::clone(collector), &event_tx,
                                        onboarding_state_path.clone(),
                                    );
                                }
                            }
                            KeyCode::Esc if app.setup_error.is_some() => app.open_welcome(),
                            _ => {}
                        },
                        View::Home => match key.code {
                            KeyCode::Tab | KeyCode::BackTab => app.toggle_home_focus(),
                            KeyCode::Char('m') => app.open_remote_picker(),
                            KeyCode::Char('s') => app.open_settings(),
                            KeyCode::Down | KeyCode::Right | KeyCode::Char('j' | 'l') => {
                                match app.home_focus {
                                    HomeFocus::Navigation => app.next_home_nav(),
                                    HomeFocus::Groups => app.next_home_item(),
                                }
                            }
                            KeyCode::Up | KeyCode::Left | KeyCode::Char('h' | 'k') => {
                                match app.home_focus {
                                    HomeFocus::Navigation => app.previous_home_nav(),
                                    HomeFocus::Groups => app.previous_home_item(),
                                }
                            }
                            KeyCode::Enter if app.home_focus == HomeFocus::Navigation => {
                                match app.home_nav_selected {
                                    0 => app.toggle_home_focus(),
                                    1 => app.open_remote_picker(),
                                    _ => app.open_settings(),
                                }
                            }
                            KeyCode::Enter if app.home_selected == 0 => {
                                if let Some(collector) = local_collector.as_ref() {
                                    let target = local_target();
                                    app.start_monitoring(target.clone());
                                    spawn_worker(
                                        &mut workers, target, Arc::clone(collector), refresh_every,
                                        &event_tx, &refresh_tx, &shutdown_rx,
                                    );
                                }
                            }
                            KeyCode::Enter if app.home_selected + 1 == app.home_item_count() => {
                                app.begin_group_creation();
                            }
                            KeyCode::Enter => app.show_empty_group_notice(),
                            _ => {}
                        },
                        View::CreateGroup => match key.code {
                            KeyCode::Esc => app.cancel_group_creation(),
                            KeyCode::Backspace => app.pop_group_character(),
                            KeyCode::Enter => {
                                if app.create_group().is_ok() {
                                    if let Err(message) = groups::save(
                                        &groups::state_path(),
                                        &app.machine_groups,
                                    ).await {
                                        app.show_home_notice(message);
                                    }
                                }
                            }
                            KeyCode::Char(character) => app.push_group_character(character),
                            _ => {}
                        },
                        View::RemotePicker => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next_remote(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_remote(),
                            KeyCode::Esc => app.close_remote_picker(),
                            KeyCode::Enter if app.remote_selected == 0 => {
                                if let Some(collector) = local_collector.as_ref() {
                                    let target = local_target();
                                    app.start_monitoring(target.clone());
                                    spawn_worker(
                                        &mut workers, target, Arc::clone(collector), refresh_every,
                                        &event_tx, &refresh_tx, &shutdown_rx,
                                    );
                                }
                            }
                            KeyCode::Enter => app.explain_remote_pairing(),
                            _ => {}
                        },
                        View::Settings => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next_setting(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_setting(),
                            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => app.open_home(),
                            _ => {}
                        },
                        View::Overview => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous(),
                            KeyCode::Enter => app.open_detail(),
                            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => app.open_home(),
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

fn begin_agent_setup(
    app: &mut App,
    workers: &mut tokio::task::JoinSet<()>,
    collector: Arc<dyn Collector>,
    event_tx: &mpsc::Sender<CollectionEvent>,
    state_path: PathBuf,
) {
    app.start_agent_setup();
    workers.spawn(agent_setup(
        local_target(),
        collector,
        event_tx.clone(),
        state_path,
    ));
}

async fn agent_setup(
    target: HostTarget,
    collector: Arc<dyn Collector>,
    event_tx: mpsc::Sender<CollectionEvent>,
    state_path: PathBuf,
) {
    let result = perform_agent_setup(&target, collector, &event_tx, &state_path).await;
    let event = match result {
        Ok(_) => CollectionEvent::SetupComplete,
        Err(message) => CollectionEvent::SetupError { message },
    };
    let _ = event_tx.send(event).await;
}

async fn perform_agent_setup(
    target: &HostTarget,
    collector: Arc<dyn Collector>,
    event_tx: &mpsc::Sender<CollectionEvent>,
    state_path: &std::path::Path,
) -> Result<HostSnapshot, String> {
    send_setup_progress(event_tx, 14, "Locating the nekoHub agent package").await;
    tokio::time::sleep(Duration::from_millis(240)).await;

    send_setup_progress(event_tx, 32, "Connecting to the local agent service").await;
    let first = collector
        .collect(target)
        .await
        .map_err(|error| format!("Could not reach the local agent: {error}"))?;
    let mut tracker = RateTracker::default();
    tracker.apply(first);

    send_setup_progress(event_tx, 58, "Agent installed · service is running").await;
    tokio::time::sleep(Duration::from_millis(320)).await;
    send_setup_progress(event_tx, 72, "Validating CPU, memory, disk and network").await;
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    let second = collector
        .collect(target)
        .await
        .map_err(|error| format!("The agent stopped responding during validation: {error}"))?;
    let snapshot = tracker.apply(second);
    if snapshot.hostname.is_empty()
        || snapshot.memory.total == 0
        || snapshot.root_disk.total == 0
        || snapshot.cpu_percent.is_none()
    {
        return Err("The agent returned an incomplete Linux metrics sample".into());
    }

    send_setup_progress(event_tx, 90, "Saving first-run setup").await;
    onboarding::mark_complete(state_path)
        .await
        .map_err(|error| format!("Could not save setup state: {error}"))?;
    send_setup_progress(event_tx, 100, "Agent ready · opening your dashboard").await;
    tokio::time::sleep(Duration::from_millis(650)).await;
    Ok(snapshot)
}

async fn send_setup_progress(
    event_tx: &mpsc::Sender<CollectionEvent>,
    progress: u16,
    message: &str,
) {
    let _ = event_tx
        .send(CollectionEvent::SetupProgress {
            progress,
            message: message.into(),
        })
        .await;
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
            () = tokio::time::sleep(refresh_every) => {}
            _ = refresh_rx.recv() => {}
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() {
                    return;
                }
            }
        }
    }
}
