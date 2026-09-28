mod app;
mod demo;
mod groups;
mod machines;
mod onboarding;
mod preferences;
mod terminal;
mod ui;

use std::{path::PathBuf, process::Stdio, sync::Arc, time::Duration};

use app::{App, HomeFocus, SettingsFocus, View};
use clap::Parser;
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use nekohub_agent::AgentCollector;
use nekohub_core::{Collector, HostSnapshot, HostTarget, Inventory, RateTracker};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
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
    Error { host_id: String, message: String },
    SetupProgress { progress: u16, message: String },
    SetupComplete,
    SetupError { message: String },
    RemoteInstallProgress { progress: u16, message: String },
    RemoteInstallLog(String),
    RemoteInstallComplete { target: String },
    RemoteUninstallComplete { target: String },
    RemoteInstallError { message: String },
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
    let mut events = EventStream::new();
    let mut redraw = tokio::time::interval(Duration::from_millis(33));
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
                CollectionEvent::RemoteInstallProgress { progress, message } => {
                    app.update_remote_install(progress, message);
                }
                CollectionEvent::RemoteInstallLog(line) => app.push_remote_install_log(line),
                CollectionEvent::RemoteInstallComplete { target } => {
                    app.complete_remote_install(&target);
                    if let Err(message) = machines::save(&machines::state_path(), &app.remote_hosts).await {
                        app.fail_remote_install(message);
                    }
                }
                CollectionEvent::RemoteUninstallComplete { target } => {
                    app.complete_remote_uninstall(&target);
                    if let Err(message) = machines::save(&machines::state_path(), &app.remote_hosts).await {
                        app.fail_remote_install(message);
                    }
                }
                CollectionEvent::RemoteInstallError { message } => app.fail_remote_install(message),
            },
            Some(input) = events.next() => match input? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    if matches!(key.code, KeyCode::Char('q'))
                        && !matches!(app.view, View::CreateGroup | View::RemoteInstall | View::RemoteUninstallConfirm | View::ThemeImport)
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
                            KeyCode::Enter if app.remote_install_selected() => {
                                app.begin_remote_install();
                            }
                            KeyCode::Enter => app.explain_remote_pairing(),
                            KeyCode::Char('u') | KeyCode::Delete => app.begin_remote_uninstall(),
                            _ => {}
                        },
                        View::RemoteInstall => match key.code {
                            KeyCode::Esc => app.cancel_remote_install(),
                            KeyCode::Backspace => app.pop_remote_install_character(),
                            KeyCode::Enter => {
                                if let Ok(target) = app.remote_install_target() {
                                    app.start_remote_install_progress();
                                    workers.spawn(install_remote_agent(target, event_tx.clone()));
                                }
                            }
                            KeyCode::Char(character) => {
                                app.push_remote_install_character(character);
                            }
                            _ => {}
                        },
                        View::RemoteUninstallConfirm => match key.code {
                            KeyCode::Esc => app.cancel_remote_uninstall(),
                            KeyCode::Enter => {
                                let target = app.remote_install_draft.clone();
                                app.start_remote_uninstall_progress();
                                workers.spawn(uninstall_remote_agent(target, event_tx.clone()));
                            }
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
                        View::Settings => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next_setting(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_setting(),
                            KeyCode::Tab | KeyCode::BackTab => app.toggle_settings_focus(),
                            KeyCode::Right | KeyCode::Enter
                                if app.settings_focus == SettingsFocus::Sidebar =>
                            {
                                app.enter_settings_content();
                            }
                            KeyCode::Left | KeyCode::Char('h')
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
                            KeyCode::Esc if app.settings_focus == SettingsFocus::Content => {
                                app.leave_settings_content();
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
                        View::Overview => match key.code {
                            KeyCode::Down | KeyCode::Char('j') => app.next_monitor_section(),
                            KeyCode::Up | KeyCode::Char('k') => app.previous_monitor_section(),
                            KeyCode::Char('n') => app.next(),
                            KeyCode::Char('p') => app.previous(),
                            KeyCode::Enter if app.monitor_selected == 0 => app.open_detail(),
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
    };
    if let Err(message) = preferences::save(&preferences::state_path(), updated).await {
        app.settings_notice = Some(message);
    }
}

async fn install_remote_agent(target: String, event_tx: mpsc::Sender<CollectionEvent>) {
    const REMOTE_INSTALL: &str = r#"set -eu
progress() { printf 'NEKOHUB_PROGRESS:%s:%s\n' "$1" "$2"; }
progress 12 'Connected to remote machine'
run_root() {
  if [ "$(id -u)" -eq 0 ]; then
    "$@"
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
if command -v curl >/dev/null 2>&1; then
  curl -fsSL https://awakyy1.github.io/nekohub/install.sh | run_root sh
elif command -v wget >/dev/null 2>&1; then
  wget -qO- https://awakyy1.github.io/nekohub/install.sh | run_root sh
else
  echo 'curl or wget is required to install the nekoHub repository' >&2
  exit 1
fi
progress 55 'Signed APT repository configured'
run_root apt-get install -y nekohub-agent
progress 82 'Agent package installed'
if command -v systemctl >/dev/null 2>&1; then
  run_root systemctl enable --now nekohub-agent.service
  run_root systemctl is-active --quiet nekohub-agent.service
fi
progress 96 'Agent service is running'
echo 'nekoHub agent is ready.'"#;

    let result = run_remote_command(&target, REMOTE_INSTALL, &event_tx).await;
    let event = match result {
        Ok(()) => CollectionEvent::RemoteInstallComplete { target },
        Err(message) => CollectionEvent::RemoteInstallError { message },
    };
    let _ = event_tx.send(event).await;
}

async fn uninstall_remote_agent(target: String, event_tx: mpsc::Sender<CollectionEvent>) {
    const REMOTE_UNINSTALL: &str = r#"set -eu
progress() { printf 'NEKOHUB_PROGRESS:%s:%s\n' "$1" "$2"; }
run_root() {
  if [ "$(id -u)" -eq 0 ]; then "$@";
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
    let result = run_remote_command(&target, REMOTE_UNINSTALL, &event_tx).await;
    let event = match result {
        Ok(()) => CollectionEvent::RemoteUninstallComplete { target },
        Err(message) => CollectionEvent::RemoteInstallError { message },
    };
    let _ = event_tx.send(event).await;
}

async fn run_remote_command(
    target: &str,
    command: &str,
    event_tx: &mpsc::Sender<CollectionEvent>,
) -> Result<(), String> {
    let mut child = tokio::process::Command::new("ssh")
        .arg("-T")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=12")
        .arg(target)
        .arg(command)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("Could not start SSH: {error}"))?;
    let stdout = child.stdout.take().ok_or("Could not read SSH output")?;
    let stderr = child.stderr.take().ok_or("Could not read SSH errors")?;
    let mut stdout = BufReader::new(stdout).lines();
    let mut stderr = BufReader::new(stderr).lines();
    let mut stdout_open = true;
    let mut stderr_open = true;
    while stdout_open || stderr_open {
        tokio::select! {
            line = stdout.next_line(), if stdout_open => match line {
                Ok(Some(line)) => send_remote_install_line(event_tx, line).await,
                Ok(None) => stdout_open = false,
                Err(error) => return Err(format!("Could not read SSH output: {error}")),
            },
            line = stderr.next_line(), if stderr_open => match line {
                Ok(Some(line)) => send_remote_install_line(event_tx, line).await,
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
        Err(
            "SSH operation failed. Key/agent authentication and passwordless sudo are required."
                .into(),
        )
    }
}

async fn send_remote_install_line(event_tx: &mpsc::Sender<CollectionEvent>, line: String) {
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
    let compact = line.split_whitespace().collect::<Vec<_>>().join(" ");
    if !compact.is_empty() {
        let _ = event_tx
            .send(CollectionEvent::RemoteInstallLog(
                compact.chars().take(140).collect(),
            ))
            .await;
    }
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
