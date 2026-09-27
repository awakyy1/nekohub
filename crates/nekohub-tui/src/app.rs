use std::collections::{HashMap, VecDeque};

use nekohub_core::{HostSnapshot, HostTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Welcome,
    AgentConfirm,
    AgentSetup,
    Home,
    RemotePicker,
    Settings,
    Overview,
    Detail,
}

#[derive(Debug)]
pub struct HostState {
    pub target: HostTarget,
    pub snapshot: Option<HostSnapshot>,
    pub last_error: Option<String>,
    pub cpu_history: VecDeque<u64>,
}

impl HostState {
    fn new(target: HostTarget) -> Self {
        Self {
            target,
            snapshot: None,
            last_error: None,
            cpu_history: VecDeque::with_capacity(120),
        }
    }

    pub fn is_online(&self) -> bool {
        self.snapshot.is_some() && self.last_error.is_none()
    }
}

#[derive(Debug)]
pub struct App {
    pub hosts: Vec<HostState>,
    pub selected: usize,
    pub view: View,
    pub welcome_selected: usize,
    pub agent_confirm_selected: usize,
    pub home_selected: usize,
    pub remote_hosts: Vec<HostTarget>,
    pub remote_selected: usize,
    pub animation_tick: u64,
    pub welcome_notice: Option<String>,
    pub remote_notice: Option<String>,
    pub setup_target_progress: u16,
    pub setup_visible_progress: u16,
    pub setup_message: String,
    pub setup_error: Option<String>,
    remote_return_view: View,
    by_id: HashMap<String, usize>,
}

impl App {
    pub fn new(remote_hosts: Vec<HostTarget>) -> Self {
        Self {
            hosts: Vec::new(),
            selected: 0,
            view: View::Welcome,
            welcome_selected: 0,
            agent_confirm_selected: 1,
            home_selected: 0,
            remote_hosts,
            remote_selected: 0,
            animation_tick: 0,
            welcome_notice: None,
            remote_notice: None,
            setup_target_progress: 0,
            setup_visible_progress: 0,
            setup_message: String::new(),
            setup_error: None,
            remote_return_view: View::Welcome,
            by_id: HashMap::new(),
        }
    }

    pub fn monitoring(targets: Vec<HostTarget>) -> Self {
        let hosts: Vec<_> = targets.into_iter().map(HostState::new).collect();
        let by_id = hosts
            .iter()
            .enumerate()
            .map(|(index, host)| (host.target.id.clone(), index))
            .collect();
        Self {
            hosts,
            selected: 0,
            view: View::Overview,
            welcome_selected: 0,
            agent_confirm_selected: 1,
            home_selected: 0,
            remote_hosts: Vec::new(),
            remote_selected: 0,
            animation_tick: 0,
            welcome_notice: None,
            remote_notice: None,
            setup_target_progress: 0,
            setup_visible_progress: 0,
            setup_message: String::new(),
            setup_error: None,
            remote_return_view: View::Home,
            by_id,
        }
    }

    pub fn home(remote_hosts: Vec<HostTarget>) -> Self {
        let mut app = Self::new(remote_hosts);
        app.view = View::Home;
        app
    }

    pub fn start_monitoring(&mut self, target: HostTarget) {
        self.hosts = vec![HostState::new(target)];
        self.selected = 0;
        self.by_id = self
            .hosts
            .iter()
            .enumerate()
            .map(|(index, host)| (host.target.id.clone(), index))
            .collect();
        self.view = View::Overview;
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn apply_snapshot(&mut self, snapshot: HostSnapshot) {
        let Some(index) = self.by_id.get(&snapshot.host_id).copied() else {
            return;
        };
        let host = &mut self.hosts[index];
        if let Some(cpu) = snapshot.cpu_percent {
            if host.cpu_history.len() == 120 {
                host.cpu_history.pop_front();
            }
            host.cpu_history
                .push_back(cpu.clamp(0.0, 100.0).round() as u64);
        }
        host.snapshot = Some(snapshot);
        host.last_error = None;
    }

    pub fn apply_error(&mut self, host_id: &str, error: String) {
        if let Some(index) = self.by_id.get(host_id).copied() {
            self.hosts[index].last_error = Some(error);
        }
    }

    pub fn next(&mut self) {
        if !self.hosts.is_empty() {
            self.selected = (self.selected + 1) % self.hosts.len();
        }
    }

    pub fn previous(&mut self) {
        if !self.hosts.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.hosts.len() - 1);
        }
    }

    pub fn selected(&self) -> Option<&HostState> {
        self.hosts.get(self.selected)
    }

    pub fn open_detail(&mut self) {
        if !self.hosts.is_empty() {
            self.view = View::Detail;
        }
    }

    pub fn open_overview(&mut self) {
        self.view = View::Overview;
    }

    pub fn open_welcome(&mut self) {
        self.view = View::Welcome;
    }

    pub fn open_home(&mut self) {
        self.hosts.clear();
        self.by_id.clear();
        self.selected = 0;
        self.home_selected = 0;
        self.view = View::Home;
    }

    pub fn next_home_item(&mut self) {
        self.home_selected = (self.home_selected + 1) % 3;
    }

    pub fn previous_home_item(&mut self) {
        self.home_selected = self.home_selected.checked_sub(1).unwrap_or(2);
    }

    pub fn open_settings(&mut self) {
        self.view = View::Settings;
    }

    pub fn open_agent_confirmation(&mut self) {
        self.agent_confirm_selected = 1;
        self.view = View::AgentConfirm;
    }

    pub fn toggle_agent_confirmation(&mut self) {
        self.agent_confirm_selected = 1 - self.agent_confirm_selected;
    }

    pub fn open_remote_picker(&mut self) {
        self.remote_return_view = self.view;
        self.view = View::RemotePicker;
        self.remote_notice = None;
    }

    pub fn close_remote_picker(&mut self) {
        self.view = self.remote_return_view;
        self.remote_notice = None;
    }

    pub fn start_agent_setup(&mut self) {
        self.view = View::AgentSetup;
        self.setup_target_progress = 8;
        self.setup_visible_progress = 0;
        self.setup_message = "Preparing local agent setup".into();
        self.setup_error = None;
    }

    pub fn update_agent_setup(&mut self, progress: u16, message: String) {
        self.setup_target_progress = progress.min(100);
        self.setup_message = message;
        self.setup_error = None;
    }

    pub fn fail_agent_setup(&mut self, message: String) {
        self.setup_error = Some(message);
    }

    pub fn advance_animation(&mut self) {
        self.animation_tick = self.animation_tick.wrapping_add(1);
        if self.view == View::AgentSetup && self.setup_visible_progress < self.setup_target_progress
        {
            let remaining = self.setup_target_progress - self.setup_visible_progress;
            let step = remaining.div_ceil(5).max(1);
            self.setup_visible_progress = self
                .setup_visible_progress
                .saturating_add(step)
                .min(self.setup_target_progress);
        }
    }

    pub fn toggle_welcome_choice(&mut self) {
        self.welcome_selected = 1 - self.welcome_selected;
        self.welcome_notice = None;
    }

    pub fn next_remote(&mut self) {
        if !self.remote_hosts.is_empty() {
            self.remote_selected = (self.remote_selected + 1) % self.remote_hosts.len();
        }
    }

    pub fn previous_remote(&mut self) {
        if !self.remote_hosts.is_empty() {
            self.remote_selected = self
                .remote_selected
                .checked_sub(1)
                .unwrap_or(self.remote_hosts.len() - 1);
        }
    }

    pub fn explain_remote_pairing(&mut self) {
        self.remote_notice = Some(
            "Remote monitoring will use the nekoHub agent. Secure pairing arrives in the next version; SSH metric collection is intentionally disabled.".into(),
        );
    }

    pub fn online_count(&self) -> usize {
        self.hosts.iter().filter(|host| host.is_online()).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_wraps() {
        let mut app = App::monitoring(vec![
            HostTarget::from_alias("a"),
            HostTarget::from_alias("b"),
        ]);
        app.previous();
        assert_eq!(app.selected, 1);
        app.next();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn setup_progress_animates_toward_target() {
        let mut app = App::new(Vec::new());
        app.start_agent_setup();
        app.update_agent_setup(70, "Validating metrics".into());
        app.advance_animation();
        assert!(app.setup_visible_progress > 0);
        assert!(app.setup_visible_progress < 70);
    }

    #[test]
    fn agent_confirmation_defaults_to_no() {
        let mut app = App::new(Vec::new());
        app.open_agent_confirmation();
        assert_eq!(app.agent_confirm_selected, 1);
        app.toggle_agent_confirmation();
        assert_eq!(app.agent_confirm_selected, 0);
    }

    #[test]
    fn home_menu_wraps_and_opens_after_setup() {
        let mut app = App::new(Vec::new());
        app.open_home();
        assert_eq!(app.view, View::Home);
        app.previous_home_item();
        assert_eq!(app.home_selected, 2);
        app.next_home_item();
        assert_eq!(app.home_selected, 0);
    }
}
