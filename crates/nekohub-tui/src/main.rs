mod app;
mod credentials;
mod demo;
mod groups;
mod machines;
mod onboarding;
mod preferences;
mod ssh_terminal;
mod terminal;
mod ui;

use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use app::{App, HomeFocus, SettingsFocus, View};
use clap::Parser;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use nekohub_agent::{AgentCollector, SshAgentCollector};
use nekohub_core::{Collector, HostSnapshot, HostTarget, Inventory, RateTracker};
use ssh_terminal::{
    SshTerminalSession, TerminalEvent, embedded_size, encode_key, is_terminal_close_key,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::{broadcast, mpsc, watch},
};

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
    Error {
        host_id: String,
        message: String,
    },
    SetupProgress {
        progress: u16,
        message: String,
    },
    SetupComplete,
    SetupError {
        message: String,
    },
    RemoteInstallProgress {
        progress: u16,
        message: String,
    },
    RemoteInstallLog(String),
    RemoteInstallComplete {
        target: HostTarget,
        collector: Arc<dyn Collector>,
    },
    RemoteUninstallComplete {
        target: String,
    },
    RemoteInstallError {
        message: String,
    },
    AgentVersion {
        host_id: String,
        version: String,
    },
    AgentUpdateComplete,
    AgentUpdateError {
        message: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let ssh_config = args.ssh_config.clone().unwrap_or_else(default_ssh_config);
    let mut remote_hosts = discover_remote_hosts(&args, &ssh_config);
    machines::merge(&mut remote_hosts, machines::load(&machines::state_path()));
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
        display_name: app::local_machine_name(),
        tags: vec!["local".into()],
    }
}

fn local_target_named(display_name: &str) -> HostTarget {
    let mut target = local_target();
    display_name.clone_into(&mut target.display_name);
    target
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
    let preferences = preferences::load(&preferences::state_path());
    let mut app = if onboarding::is_complete(&state_path) {
        App::home(remote_hosts, machine_groups)
    } else {
        App::new(remote_hosts, machine_groups)
    };
    app.background_enabled = preferences.background_enabled;
    app.theme = preferences.theme;
    app.font_profile = preferences.font_profile;
    app.custom_theme = preferences.custom_theme;
    app.local_alias = preferences.local_alias;
    app.last_machine_id = preferences.last_machine_id;
    app.credentials = credentials::load(&credentials::state_path());
    if let Some(alias) = app.local_alias.as_ref() {
        app.local_name.clone_from(alias);
    }
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
    let (terminal_event_tx, mut terminal_event_rx) = mpsc::unbounded_channel();
    let (refresh_tx, _) = broadcast::channel(1);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let mut workers = tokio::task::JoinSet::new();
    let mut ssh_session: Option<SshTerminalSession> = None;

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
    let mut events = EventStream::new();
    let mut redraw = tokio::time::interval(Duration::from_millis(33));
    redraw.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let result = loop {
        tokio::select! {
            _ = redraw.tick() => {
                app.advance_animation();
                if app.view == View::Terminal {
                    let (outer_cols, outer_rows) = crossterm::terminal::size()?;
                    let (rows, cols) = embedded_size(outer_cols, outer_rows);
                    if app.terminal.screen().size() != (rows, cols) {
                        app.terminal.resize(rows, cols);
                        if let Some(session) = ssh_session.as_ref() {
                            if let Err(message) = session.resize(rows, cols) {
                                app.terminal.fail(app.terminal.generation(), message);
                            }
                        }
                    }
                }
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
                CollectionEvent::RemoteInstallProgress { progress, message } => {
                    app.update_remote_install(progress, message);
                }
                CollectionEvent::RemoteInstallLog(line) => app.push_remote_install_log(line),
                CollectionEvent::RemoteInstallComplete { target, collector } => {
                    app.complete_remote_install(&target.alias);
                    if let Err(message) = machines::save(&machines::state_path(), &app.remote_hosts).await {
                        app.fail_remote_install(message);
                    } else {
                        let target = app
                            .remote_hosts
                            .iter()
                            .find(|machine| machine.alias == target.alias)
                            .cloned()
                            .unwrap_or(target);
                        app.start_monitoring(target.clone());
                        save_preferences(app).await;
                        app.view = View::RemoteInstallProgress;
                        spawn_worker(
                            &mut workers, target, collector, refresh_every,
                            &event_tx, &refresh_tx, &shutdown_rx,
                        );
                    }
                }
                CollectionEvent::RemoteUninstallComplete { target } => {
                    app.complete_remote_uninstall(&target);
                    if let Err(message) = machines::save(&machines::state_path(), &app.remote_hosts).await {
                        app.fail_remote_install(message);
                    }
                }
                CollectionEvent::RemoteInstallError { message } => app.fail_remote_install(message),
                CollectionEvent::AgentVersion { host_id, version } => {
                    app.apply_agent_version(&host_id, version);
                }
                CollectionEvent::AgentUpdateComplete => {
                    app.complete_agent_update();
                    let _ = refresh_tx.send(());
                }
                CollectionEvent::AgentUpdateError { message } => app.fail_agent_update(message),
            },
            Some(event) = terminal_event_rx.recv() => match event {
                TerminalEvent::Output { generation, bytes } => {
                    app.terminal.process_output(generation, &bytes);
                }
                TerminalEvent::Exit { generation, status } => {
                    if generation == app.terminal.generation() {
                        app.terminal.finish(generation, status);
                        ssh_session = None;
                    }
                }
                TerminalEvent::Error { generation, message } => {
                    if generation == app.terminal.generation() {
                        app.terminal.fail(generation, message);
                        ssh_session = None;
                    }
                }
            },
            Some(input) = events.next() => {
                let input = input?;
                let input = if let Event::Mouse(mouse) = input {
                    let (width, height) = crossterm::terminal::size()?;
                    let area = ratatui::layout::Rect::new(0, 0, width, height);
                    let Some(key) = ui::mouse_key(app, area, mouse) else {
                        continue;
                    };
                    Event::Key(key)
                } else {
                    input
                };
                match input {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if app.view == View::Terminal {
                        if is_terminal_close_key(key) {
                            ssh_session = None;
                            app.close_terminal();
                        } else if let Some(session) = ssh_session.as_mut() {
                            let bytes = encode_key(key);
                            if !bytes.is_empty() {
                                if let Err(message) = session.send(&bytes) {
                                    app.terminal.fail(app.terminal.generation(), message);
                                    ssh_session = None;
                                }
                            }
                        }
                        continue;
                    }
                    if matches!(key.code, KeyCode::Char('q'))
                        && !matches!(app.view, View::CreateGroup | View::GroupAssign | View::MachineAlias | View::RemoteInstall | View::RemoteConnect | View::RemoteUninstallConfirm | View::ThemeImport | View::CredentialKeyEdit | View::AgentUpdateAuth | View::AgentUpdateProgress | View::TerminalPassword | View::TerminalSavePassword)
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
                            KeyCode::Tab | KeyCode::Down | KeyCode::Char('j') => {
                                app.toggle_home_focus();
                            }
                            KeyCode::BackTab | KeyCode::Up | KeyCode::Char('k') => {
                                app.previous_home_focus();
                            }
                            KeyCode::Char('m') => app.open_remote_picker(),
                            KeyCode::Char('s') => app.open_settings(),
                            KeyCode::Right | KeyCode::Char('l') => {
                                match app.home_focus {
                                    HomeFocus::Navigation => app.next_home_nav(),
                                    HomeFocus::Machines => app.next_home_machine(),
                                    HomeFocus::Groups => app.next_home_item(),
                                }
                            }
                            KeyCode::Left | KeyCode::Char('h') => {
                                match app.home_focus {
                                    HomeFocus::Navigation => app.previous_home_nav(),
                                    HomeFocus::Machines => app.previous_home_machine(),
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
                            KeyCode::Enter if app.home_focus == HomeFocus::Machines => {
                                let machine_index = app.selected_home_machine_index();
                                if machine_index == 0 {
                                    if let Some(collector) = local_collector.as_ref() {
                                        let target = local_target_named(&app.local_name);
                                        app.start_monitoring(target.clone());
                                        save_preferences(app).await;
                                        spawn_worker(
                                            &mut workers, target, Arc::clone(collector), refresh_every,
                                            &event_tx, &refresh_tx, &shutdown_rx,
                                        );
                                    }
                                } else {
                                    app.remote_selected = machine_index;
                                    if app.selected_installed_remote().is_some() {
                                        app.begin_remote_connect();
                                    } else {
                                        app.open_remote_picker();
                                        app.remote_selected = machine_index;
                                    }
                                }
                            }
                            KeyCode::Enter if app.home_selected + 1 == app.home_item_count() => {
                                app.begin_group_creation();
                            }
                            KeyCode::Enter => app.open_group(),
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
                        View::GroupDetail => match key.code {
                            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => app.close_group(),
                            KeyCode::Down | KeyCode::Char('j') => app.next_group_machine(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_group_machine(),
                            KeyCode::Enter => {
                                let selected = app.selected_group_machine_id().map(str::to_owned);
                                if selected.as_deref() == Some("local") {
                                    if let Some(collector) = local_collector.as_ref() {
                                        let target = local_target_named(&app.local_name);
                                        app.start_monitoring(target.clone());
                                        save_preferences(app).await;
                                        spawn_worker(
                                            &mut workers, target, Arc::clone(collector), refresh_every,
                                            &event_tx, &refresh_tx, &shutdown_rx,
                                        );
                                    }
                                } else if let Some(id) = selected {
                                    if let Some(index) = app
                                        .remote_hosts
                                        .iter()
                                        .position(|machine| machine.id == id)
                                    {
                                        app.remote_selected = index + 1;
                                        if app.selected_installed_remote().is_some() {
                                            app.begin_remote_connect();
                                        } else {
                                            app.open_remote_picker();
                                            app.remote_selected = index + 1;
                                        }
                                    }
                                }
                            }
                            _ => {}
                        },
                        View::RemotePicker => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next_remote(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_remote(),
                            KeyCode::Esc => app.close_remote_picker(),
                            KeyCode::Enter if app.remote_selected == 0 => {
                                if let Some(collector) = local_collector.as_ref() {
                                    let target = local_target_named(&app.local_name);
                                    app.start_monitoring(target.clone());
                                    save_preferences(app).await;
                                    spawn_worker(
                                        &mut workers, target, Arc::clone(collector), refresh_every,
                                        &event_tx, &refresh_tx, &shutdown_rx,
                                    );
                                }
                            }
                            KeyCode::Enter if app.remote_install_selected() => {
                                app.begin_remote_install();
                            }
                            KeyCode::Enter if app.selected_installed_remote().is_some() => {
                                app.begin_remote_connect();
                            }
                            KeyCode::Enter => app.explain_remote_pairing(),
                            KeyCode::Char('a') => app.begin_machine_alias(),
                            KeyCode::Char('g') => app.begin_group_assignment(),
                            KeyCode::Char('u') | KeyCode::Delete => app.begin_remote_uninstall(),
                            _ => {}
                        },
                        View::MachineAlias => match key.code {
                            KeyCode::Esc => app.cancel_machine_alias(),
                            KeyCode::Backspace => app.pop_machine_alias_character(),
                            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                app.clear_machine_alias();
                            }
                            KeyCode::Enter => {
                                let local = app.remote_selected == 0;
                                if app.apply_machine_alias().is_ok() {
                                    if local {
                                        save_preferences(app).await;
                                    } else if let Err(message) = machines::save(
                                        &machines::state_path(),
                                        &app.remote_hosts,
                                    ).await {
                                        app.remote_notice = Some(message);
                                    }
                                }
                            }
                            KeyCode::Char(character) => {
                                app.push_machine_alias_character(character);
                            }
                            _ => {}
                        },
                        View::GroupAssign => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next_group_assignment(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_group_assignment(),
                            KeyCode::Enter => {
                                app.apply_group_assignment();
                                if let Err(message) = groups::save(&groups::state_path(), &app.machine_groups).await {
                                    app.remote_notice = Some(message);
                                }
                            }
                            KeyCode::Esc => app.cancel_group_assignment(),
                            _ => {}
                        },
                        View::RemoteInstall => match key.code {
                            KeyCode::Esc => app.cancel_remote_install(),
                            KeyCode::Tab | KeyCode::BackTab => app.toggle_remote_install_field(),
                            KeyCode::Backspace => app.pop_remote_install_character(),
                            KeyCode::Enter => {
                                if let Ok(target) = app.remote_install_target() {
                                    let password = app.take_remote_password();
                                    app.start_remote_install_progress();
                                    workers.spawn(install_remote_agent(target, password, event_tx.clone()));
                                }
                            }
                            KeyCode::Char(character) => {
                                app.push_remote_install_character(character);
                            }
                            _ => {}
                        },
                        View::RemoteConnect => match key.code {
                            KeyCode::Esc => app.cancel_remote_connect(),
                            KeyCode::Backspace => app.pop_remote_install_character(),
                            KeyCode::Enter => {
                                if let Some(target) = app.selected_installed_remote().cloned() {
                                    let password = app.take_remote_password();
                                    let probe_password = password.clone();
                                    let probe_target = target.alias.clone();
                                    let collector: Arc<dyn Collector> = Arc::new(
                                        SshAgentCollector::new(
                                            target.alias.clone(), password, Duration::from_secs(12),
                                        )
                                    );
                                    app.start_monitoring(target.clone());
                                    save_preferences(app).await;
                                    spawn_worker(
                                        &mut workers, target, collector, refresh_every,
                                        &event_tx, &refresh_tx, &shutdown_rx,
                                    );
                                    workers.spawn(probe_remote_agent_version(
                                        probe_target,
                                        probe_password,
                                        event_tx.clone(),
                                    ));
                                }
                            }
                            KeyCode::Char(character) => app.push_remote_install_character(character),
                            _ => {}
                        },
                        View::RemoteUninstallConfirm => match key.code {
                            KeyCode::Esc => app.cancel_remote_uninstall(),
                            KeyCode::Backspace => app.pop_remote_install_character(),
                            KeyCode::Enter => {
                                let target = app.remote_install_draft.clone();
                                let password = app.take_remote_password();
                                app.start_remote_uninstall_progress();
                                workers.spawn(uninstall_remote_agent(target, password, event_tx.clone()));
                            }
                            KeyCode::Char(character) => app.push_remote_install_character(character),
                            _ => {}
                        },
                        View::RemoteInstallProgress => match key.code {
                            KeyCode::Enter | KeyCode::Esc
                                if app.remote_install_complete || app.remote_install_error.is_some() =>
                            {
                                app.close_remote_install_progress();
                            }
                            _ => {}
                        },
                        View::AgentUpdateAuth => match key.code {
                            KeyCode::Esc => app.cancel_agent_update(),
                            KeyCode::Backspace => app.pop_remote_install_character(),
                            KeyCode::Enter => {
                                let target = app.remote_install_draft.clone();
                                let password = app.take_remote_password();
                                app.start_agent_update_progress();
                                workers.spawn(update_remote_agent(
                                    target,
                                    password,
                                    event_tx.clone(),
                                ));
                            }
                            KeyCode::Char(character)
                                if !key.modifiers.intersects(
                                    KeyModifiers::CONTROL | KeyModifiers::ALT,
                                ) =>
                            {
                                app.push_remote_install_character(character);
                            }
                            _ => {}
                        },
                        View::AgentUpdateProgress => match key.code {
                            KeyCode::Enter | KeyCode::Esc
                                if app.remote_install_complete
                                    || app.remote_install_error.is_some() =>
                            {
                                app.close_agent_update_progress();
                            }
                            _ => {}
                        },
                        View::Settings => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next_setting(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_setting(),
                            KeyCode::Tab | KeyCode::BackTab => app.toggle_settings_focus(),
                            KeyCode::Right | KeyCode::Enter
                                if app.settings_focus == SettingsFocus::Sidebar =>
                            {
                                app.enter_settings_content();
                            }
                            KeyCode::Left | KeyCode::Char('h') | KeyCode::Esc
                                if app.settings_focus == SettingsFocus::Content =>
                            {
                                app.leave_settings_content();
                            }
                            KeyCode::Enter | KeyCode::Char(' ')
                                if app.settings_focus == SettingsFocus::Content
                                    && app.settings_selected == 0
                                    && app.settings_item_selected == 0 =>
                            {
                                app.toggle_background();
                                save_preferences(app).await;
                            }
                            KeyCode::Enter | KeyCode::Char(' ')
                                if app.settings_focus == SettingsFocus::Content
                                    && app.settings_selected == 0
                                    && app.settings_item_selected == 1 =>
                            {
                                app.next_font_profile();
                                save_preferences(app).await;
                            }
                            KeyCode::Enter | KeyCode::Char(' ')
                                if app.settings_focus == SettingsFocus::Content
                                    && app.settings_selected == 1
                                    && app.settings_item_selected < 4 =>
                            {
                                app.apply_builtin_theme(
                                    preferences::Theme::ALL[app.settings_item_selected],
                                );
                                save_preferences(app).await;
                            }
                            KeyCode::Enter | KeyCode::Char(' ')
                                if app.settings_focus == SettingsFocus::Content
                                    && app.settings_selected == 1
                                    && app.settings_item_selected == 4 =>
                            {
                                app.begin_theme_import();
                            }
                            KeyCode::Enter
                                if app.settings_focus == SettingsFocus::Content
                                    && app.settings_selected == 2 =>
                            {
                                app.begin_credential_key_edit();
                            }
                            KeyCode::Char('p')
                                if app.settings_focus == SettingsFocus::Content
                                    && app.settings_selected == 2 =>
                            {
                                app.remove_selected_password();
                                save_credentials(app);
                            }
                            KeyCode::Char('x')
                                if app.settings_focus == SettingsFocus::Content
                                    && app.settings_selected == 2 =>
                            {
                                app.remove_selected_identity_file();
                                save_credentials(app);
                            }
                            KeyCode::Esc => app.open_home(),
                            _ => {}
                        },
                        View::ThemeImport => match key.code {
                            KeyCode::Esc => app.cancel_theme_import(),
                            KeyCode::Backspace => app.pop_theme_path_character(),
                            KeyCode::Enter => {
                                match preferences::import_theme(std::path::Path::new(
                                    app.theme_import_draft.trim(),
                                )) {
                                    Ok(theme) => {
                                        app.apply_custom_theme(theme);
                                        save_preferences(app).await;
                                    }
                                    Err(message) => app.theme_import_error = Some(message),
                                }
                            }
                            KeyCode::Char(character) => app.push_theme_path_character(character),
                            _ => {}
                        },
                        View::CredentialKeyEdit => match key.code {
                            KeyCode::Esc => app.cancel_credential_key_edit(),
                            KeyCode::Backspace => app.pop_credential_key_character(),
                            KeyCode::Enter => match app.save_credential_key() {
                                Ok(()) => save_credentials(app),
                                Err(message) => app.credential_key_error = Some(message),
                            },
                            KeyCode::Char(character)
                                if !key.modifiers.intersects(
                                    KeyModifiers::CONTROL | KeyModifiers::ALT,
                                ) =>
                            {
                                app.push_credential_key_character(character);
                            }
                            _ => {}
                        },
                        View::Overview => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next_monitor_section(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_monitor_section(),
                            KeyCode::Char('n') => app.next(),
                            KeyCode::Char('p') => app.previous(),
                            KeyCode::Enter if app.monitor_selected == 0 => app.open_detail(),
                            KeyCode::Enter if app.monitor_selected == 7 => {
                                app.begin_terminal_password();
                            }
                            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => app.open_home(),
                            KeyCode::Char('r') => { let _ = refresh_tx.send(()); },
                            KeyCode::Char('"') => {
                                app.register_agent_update_quote();
                            }
                            _ => {}
                        },
                        View::Detail => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous(),
                            KeyCode::Esc | KeyCode::Left | KeyCode::Char('h') => app.open_overview(),
                            KeyCode::Char('r') => { let _ = refresh_tx.send(()); },
                            _ => {}
                        },
                        View::TerminalPassword => match key.code {
                            KeyCode::Esc => app.close_terminal(),
                            KeyCode::Backspace => app.pop_terminal_password_character(),
                            KeyCode::Enter => {
                                if app.should_confirm_terminal_password_save() {
                                    app.begin_terminal_password_save();
                                } else {
                                    ssh_session = spawn_ssh_terminal(app, &terminal_event_tx);
                                }
                            }
                            KeyCode::Char(character)
                                if !key.modifiers.intersects(
                                    KeyModifiers::CONTROL | KeyModifiers::ALT,
                                ) => {
                                app.push_terminal_password_character(character);
                            }
                            _ => {}
                        },
                        View::TerminalSavePassword => match key.code {
                            KeyCode::Esc => app.view = View::TerminalPassword,
                            KeyCode::Left
                            | KeyCode::Right
                            | KeyCode::Up
                            | KeyCode::Down
                            | KeyCode::Tab
                            | KeyCode::BackTab => app.toggle_terminal_password_save(),
                            KeyCode::Enter => {
                                app.save_terminal_password();
                                if app.terminal_save_selected == 0 {
                                    save_credentials(app);
                                }
                                ssh_session = spawn_ssh_terminal(app, &terminal_event_tx);
                            }
                            _ => {}
                        },
                        View::Terminal => unreachable!("terminal input is handled before navigation"),
                    }
                }
                Event::Paste(text) if app.view == View::Terminal => {
                    if let Some(session) = ssh_session.as_mut() {
                        let bytes = if app.terminal.screen().bracketed_paste() {
                            format!("\x1b[200~{text}\x1b[201~").into_bytes()
                        } else {
                            text.into_bytes()
                        };
                        if let Err(message) = session.send(&bytes) {
                            app.terminal.fail(app.terminal.generation(), message);
                            ssh_session = None;
                        }
                    }
                }
                Event::Resize(outer_cols, outer_rows) if app.view == View::Terminal => {
                    let (rows, cols) = embedded_size(outer_cols, outer_rows);
                    app.terminal.resize(rows, cols);
                    if let Some(session) = ssh_session.as_ref() {
                        if let Err(message) = session.resize(rows, cols) {
                            app.terminal.fail(app.terminal.generation(), message);
                        }
                    }
                }
                    _ => {}
                }
            },
        }
    };

    let _ = shutdown_tx.send(true);
    workers.abort_all();
    while workers.join_next().await.is_some() {}
    result
}

async fn save_preferences(app: &mut App) {
    let updated = preferences::Preferences {
        background_enabled: app.background_enabled,
        theme: app.theme,
        font_profile: app.font_profile,
        custom_theme: app.custom_theme.clone(),
        local_alias: app.local_alias.clone(),
        last_machine_id: app.last_machine_id.clone(),
    };
    if let Err(message) = preferences::save(&preferences::state_path(), updated).await {
        app.settings_notice = Some(message);
    }
}

fn save_credentials(app: &mut App) {
    if let Err(message) = credentials::save(&credentials::state_path(), &app.credentials) {
        app.settings_notice = Some(message);
    }
}

fn spawn_ssh_terminal(
    app: &mut App,
    event_tx: &mpsc::UnboundedSender<TerminalEvent>,
) -> Option<SshTerminalSession> {
    let (outer_cols, outer_rows) = match crossterm::terminal::size() {
        Ok(size) => size,
        Err(error) => {
            let generation = app.terminal.generation();
            app.terminal
                .fail(generation, format!("Could not read terminal size: {error}"));
            return None;
        }
    };
    let (rows, cols) = embedded_size(outer_cols, outer_rows);
    let request = app.start_terminal(rows, cols);
    match SshTerminalSession::spawn(request, rows, cols, event_tx.clone()) {
        Ok(session) => Some(session),
        Err(message) => {
            let generation = app.terminal.generation();
            app.terminal.fail(generation, message);
            None
        }
    }
}

async fn install_remote_agent(
    target: String,
    password: Option<String>,
    event_tx: mpsc::Sender<CollectionEvent>,
) {
    const REMOTE_INSTALL: &str = r#"set -eu
progress() { printf 'NEKOHUB_PROGRESS:%s:%s\n' "$1" "$2"; }
progress 12 'Connected to remote machine'
run_root() {
  if [ "$(id -u)" -eq 0 ]; then
    "$@"
  elif [ "${NEKOHUB_SUDO_STDIN:-0}" = 1 ]; then
    sudo -S -p '' "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo -n "$@"
  else
    echo 'root access or sudo is required to install nekohub-agent' >&2
    return 1
  fi
}
if ! command -v apt-get >/dev/null 2>&1; then
  echo 'remote installation currently supports Debian and Ubuntu' >&2
  exit 1
fi
if [ "$(dpkg --print-architecture)" != amd64 ]; then
  echo 'the nekoHub APT repository currently supports amd64 machines' >&2
  exit 1
fi
progress 24 'Compatibility checks passed'
installer=$(mktemp)
if command -v curl >/dev/null 2>&1; then
  curl -fsSL https://awakyy1.github.io/nekohub/install.sh -o "$installer"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$installer" https://awakyy1.github.io/nekohub/install.sh
else
  echo 'curl or wget is required to install the nekoHub repository' >&2
  exit 1
fi
run_root sh "$installer"
rm -f "$installer"
progress 55 'Signed APT repository configured'
run_root apt-get install -y nekohub-agent
progress 82 'Agent package installed'
if command -v systemctl >/dev/null 2>&1; then
  run_root systemctl enable --now nekohub-agent.service
  run_root systemctl is-active --quiet nekohub-agent.service
fi
progress 96 'Agent service is running'
test -S /run/nekohub/agent.sock
if command -v curl >/dev/null 2>&1; then
  curl -fsS http://127.0.0.1:9876/metrics >/dev/null
fi
echo 'nekoHub agent is ready.'"#;

    let result = async {
        run_remote_command(&target, REMOTE_INSTALL, password.as_deref(), &event_tx).await?;
        let _ = event_tx
            .send(CollectionEvent::RemoteInstallProgress {
                progress: 97,
                message: "Opening encrypted metrics channel".into(),
            })
            .await;
        let host = HostTarget::from_alias(target);
        let collector: Arc<dyn Collector> = Arc::new(SshAgentCollector::new(
            host.alias.clone(),
            password,
            Duration::from_secs(12),
        ));
        collector
            .collect(&host)
            .await
            .map_err(|error| format!("Agent installed, but metric pairing failed: {error}"))?;
        let _ = event_tx
            .send(CollectionEvent::RemoteInstallProgress {
                progress: 99,
                message: "First live metric sample received".into(),
            })
            .await;
        Ok::<_, String>((host, collector))
    }
    .await;
    let event = match result {
        Ok((target, collector)) => CollectionEvent::RemoteInstallComplete { target, collector },
        Err(message) => CollectionEvent::RemoteInstallError { message },
    };
    let _ = event_tx.send(event).await;
}

async fn uninstall_remote_agent(
    target: String,
    password: Option<String>,
    event_tx: mpsc::Sender<CollectionEvent>,
) {
    const REMOTE_UNINSTALL: &str = r#"set -eu
progress() { printf 'NEKOHUB_PROGRESS:%s:%s\n' "$1" "$2"; }
run_root() {
  if [ "$(id -u)" -eq 0 ]; then "$@";
  elif [ "${NEKOHUB_SUDO_STDIN:-0}" = 1 ]; then sudo -S -p '' "$@";
  elif command -v sudo >/dev/null 2>&1; then sudo -n "$@";
  else echo 'root access or sudo is required' >&2; return 1; fi
}
progress 15 'Connected to remote machine'
if ! command -v apt-get >/dev/null 2>&1; then
  echo 'remote removal currently supports Debian and Ubuntu' >&2; exit 1
fi
progress 35 'Stopping nekoHub agent'
if command -v systemctl >/dev/null 2>&1; then
  run_root systemctl disable --now nekohub-agent.service 2>/dev/null || true
fi
progress 60 'Removing agent package'
run_root apt-get remove -y nekohub-agent
progress 95 'Agent removed'
echo 'nekoHub agent was uninstalled.'"#;
    let result =
        run_remote_command(&target, REMOTE_UNINSTALL, password.as_deref(), &event_tx).await;
    let event = match result {
        Ok(()) => CollectionEvent::RemoteUninstallComplete { target },
        Err(message) => CollectionEvent::RemoteInstallError { message },
    };
    let _ = event_tx.send(event).await;
}

async fn update_remote_agent(
    target: String,
    password: Option<String>,
    event_tx: mpsc::Sender<CollectionEvent>,
) {
    const REMOTE_UPDATE: &str = r#"set -eu
progress() { printf 'NEKOHUB_PROGRESS:%s:%s\n' "$1" "$2"; }
run_root() {
  if [ "$(id -u)" -eq 0 ]; then "$@";
  elif [ "${NEKOHUB_SUDO_STDIN:-0}" = 1 ]; then sudo -S -p '' "$@";
  elif command -v sudo >/dev/null 2>&1; then sudo -n "$@";
  else echo 'root access or sudo is required' >&2; return 1; fi
}
if ! command -v apt-get >/dev/null 2>&1; then
  echo 'agent updates currently support Debian and Ubuntu' >&2; exit 1
fi
progress 12 'Connected to remote machine'
installer=$(mktemp)
if command -v curl >/dev/null 2>&1; then
  curl -fsSL https://awakyy1.github.io/nekohub/install.sh -o "$installer"
elif command -v wget >/dev/null 2>&1; then
  wget -qO "$installer" https://awakyy1.github.io/nekohub/install.sh
else
  echo 'curl or wget is required to update the repository' >&2; exit 1
fi
progress 30 'Refreshing signed nekoHub repository'
run_root sh "$installer"
rm -f "$installer"
progress 55 'Installing the latest agent'
run_root env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a apt-get update
if ! run_root env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a apt-get install -y nekohub-agent; then
  installed=$(dpkg-query -W -f='${Version}' nekohub-agent 2>/dev/null || true)
  candidate=$(apt-cache policy nekohub-agent | sed -n 's/^[[:space:]]*Candidate:[[:space:]]*//p')
  if [ -z "$installed" ] || [ -z "$candidate" ] || ! dpkg --compare-versions "$installed" ge "$candidate"; then
    echo 'apt could not install the latest nekohub-agent package' >&2
    exit 1
  fi
  echo 'apt returned an error after installing the current agent; continuing after version verification.'
fi
progress 68 'Checking container runtime access'
if getent group docker >/dev/null 2>&1 && ! id -nG nekohub-agent | grep -qw docker; then
  if ! run_root usermod -aG docker nekohub-agent; then
    echo 'Could not grant the agent access to the Docker group.' >&2
    exit 1
  fi
fi
progress 78 'Container runtime access checked'
progress 82 'Checking metrics service'
if ! run_root systemctl is-active --quiet nekohub-agent.service || [ ! -S /run/nekohub/agent.sock ]; then
  progress 86 'Starting metrics service'
  if ! run_root systemctl restart nekohub-agent.service; then
    echo 'systemd could not restart nekohub-agent.service; collecting service diagnostics.' >&2
    run_root systemctl status --no-pager --full nekohub-agent.service >&2 || true
    run_root journalctl -u nekohub-agent.service -n 30 --no-pager >&2 || true
    exit 1
  fi
fi
if ! run_root systemctl is-active --quiet nekohub-agent.service; then
  echo 'nekohub-agent.service is not active after package installation.' >&2
  run_root systemctl status --no-pager --full nekohub-agent.service >&2 || true
  run_root journalctl -u nekohub-agent.service -n 30 --no-pager >&2 || true
  exit 1
fi
progress 96 'Waiting for the new metrics stream'
attempt=0
while [ ! -S /run/nekohub/agent.sock ] && [ "$attempt" -lt 20 ]; do
  attempt=$((attempt + 1))
  sleep 0.5
done
if [ ! -S /run/nekohub/agent.sock ]; then
  echo 'nekohub-agent.service is active but its metrics socket did not appear.' >&2
  run_root systemctl status --no-pager --full nekohub-agent.service >&2 || true
  run_root journalctl -u nekohub-agent.service -n 30 --no-pager >&2 || true
  exit 1
fi
installed=$(dpkg-query -W -f='${Version}' nekohub-agent)
printf 'NEKOHUB_AGENT_VERSION:%s\n' "$installed"
echo 'nekoHub agent update complete.'"#;
    let result = run_remote_command(&target, REMOTE_UPDATE, password.as_deref(), &event_tx).await;
    let event = match result {
        Ok(()) => CollectionEvent::AgentUpdateComplete,
        Err(message) => CollectionEvent::AgentUpdateError { message },
    };
    let _ = event_tx.send(event).await;
}

async fn run_remote_command(
    target: &str,
    command: &str,
    password: Option<&str>,
    event_tx: &mpsc::Sender<CollectionEvent>,
) -> Result<(), String> {
    let mut process = if password.is_some() {
        let mut process = tokio::process::Command::new("sshpass");
        process.arg("-e").arg("ssh");
        process
    } else {
        tokio::process::Command::new("ssh")
    };
    if let Some(password) = password {
        process.env("SSHPASS", password);
    }
    let remote_command = if password.is_some() {
        format!("NEKOHUB_SUDO_STDIN=1 sh -c {}", shell_single_quote(command))
    } else {
        command.to_owned()
    };
    let mut child = process
        .arg("-T")
        .arg("-o")
        .arg(if password.is_some() {
            "BatchMode=no"
        } else {
            "BatchMode=yes"
        })
        .arg("-o")
        .arg("ConnectTimeout=12")
        .arg("-o")
        .arg("StrictHostKeyChecking=accept-new")
        .arg(target)
        .arg(remote_command)
        .stdin(if password.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("Could not start SSH: {error}"))?;
    if let Some(password) = password
        && let Some(mut stdin) = child.stdin.take()
    {
        let passwords = format!("{password}\n").repeat(12);
        stdin
            .write_all(passwords.as_bytes())
            .await
            .map_err(|error| format!("Could not provide the SSH password: {error}"))?;
    }
    let stdout = child.stdout.take().ok_or("Could not read SSH output")?;
    let stderr = child.stderr.take().ok_or("Could not read SSH errors")?;
    let mut stdout = BufReader::new(stdout).lines();
    let mut stderr = BufReader::new(stderr).lines();
    let mut stdout_open = true;
    let mut stderr_open = true;
    while stdout_open || stderr_open {
        tokio::select! {
            line = stdout.next_line(), if stdout_open => match line {
                Ok(Some(line)) => send_remote_install_line(event_tx, target, line).await,
                Ok(None) => stdout_open = false,
                Err(error) => return Err(format!("Could not read SSH output: {error}")),
            },
            line = stderr.next_line(), if stderr_open => match line {
                Ok(Some(line)) => send_remote_install_line(event_tx, target, line).await,
                Ok(None) => stderr_open = false,
                Err(error) => return Err(format!("Could not read SSH errors: {error}")),
            },
        }
    }
    let status = child
        .wait()
        .await
        .map_err(|error| format!("Could not finish SSH: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "SSH operation failed with exit status {}. See the output above for the failing command.",
            status
                .code()
                .map_or_else(|| "signal".to_owned(), |code| code.to_string())
        ))
    }
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

async fn send_remote_install_line(
    event_tx: &mpsc::Sender<CollectionEvent>,
    host_id: &str,
    line: String,
) {
    if let Some(payload) = line.strip_prefix("NEKOHUB_PROGRESS:")
        && let Some((progress, message)) = payload.split_once(':')
        && let Ok(progress) = progress.parse()
    {
        let _ = event_tx
            .send(CollectionEvent::RemoteInstallProgress {
                progress,
                message: message.into(),
            })
            .await;
        return;
    }
    if let Some(version) = line.strip_prefix("NEKOHUB_AGENT_VERSION:") {
        let version = version.trim();
        if !version.is_empty() {
            let _ = event_tx
                .send(CollectionEvent::AgentVersion {
                    host_id: host_id.to_owned(),
                    version: version.to_owned(),
                })
                .await;
        }
        return;
    }
    let compact = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if !compact.is_empty() {
        let _ = event_tx
            .send(CollectionEvent::RemoteInstallLog(
                compact.chars().take(140).collect(),
            ))
            .await;
    }
}

async fn probe_remote_agent_version(
    target: String,
    password: Option<String>,
    event_tx: mpsc::Sender<CollectionEvent>,
) {
    const VERSION_PROBE: &str = r#"version=$(dpkg-query -W -f='${Version}' nekohub-agent 2>/dev/null || true)
if [ -n "$version" ]; then
  printf 'NEKOHUB_AGENT_VERSION:%s\n' "$version"
fi"#;
    let _ = run_remote_command(&target, VERSION_PROBE, password.as_deref(), &event_tx).await;
}

fn begin_agent_setup(
    app: &mut App,
    workers: &mut tokio::task::JoinSet<()>,
    collector: Arc<dyn Collector>,
    event_tx: &mpsc::Sender<CollectionEvent>,
    state_path: PathBuf,
) {
    let target = local_target_named(&app.local_name);
    app.start_agent_setup();
    workers.spawn(agent_setup(target, collector, event_tx.clone(), state_path));
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
