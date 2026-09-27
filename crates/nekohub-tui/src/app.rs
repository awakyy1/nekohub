use std::collections::{HashMap, VecDeque};

use nekohub_core::{HostSnapshot, HostTarget};

use crate::{
    groups::MachineGroup,
    machines::INSTALLED_TAG,
    preferences::{FontProfile, Theme},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Welcome,
    AgentConfirm,
    AgentSetup,
    Home,
    CreateGroup,
    RemotePicker,
    RemoteInstall,
    Settings,
    Overview,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeFocus {
    Navigation,
    Groups,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavigationMotion {
    pub from: usize,
    pub to: usize,
    pub frame: u8,
    pub total_frames: u8,
}

#[derive(Debug)]
pub struct HostState {
    pub target: HostTarget,
    pub snapshot: Option<HostSnapshot>,
    pub last_error: Option<String>,
    pub cpu_history: VecDeque<u64>,
    pub network_rx_history: VecDeque<u64>,
    pub network_tx_history: VecDeque<u64>,
}

impl HostState {
    fn new(target: HostTarget) -> Self {
        Self {
            target,
            snapshot: None,
            last_error: None,
            cpu_history: VecDeque::with_capacity(120),
            network_rx_history: VecDeque::with_capacity(120),
            network_tx_history: VecDeque::with_capacity(120),
        }
    }

    pub fn is_online(&self) -> bool {
        self.snapshot.is_some() && self.last_error.is_none()
    }
}

#[derive(Debug)]
pub struct App {
    pub local_name: String,
    pub hosts: Vec<HostState>,
    pub selected: usize,
    pub view: View,
    pub welcome_selected: usize,
    pub agent_confirm_selected: usize,
    pub home_focus: HomeFocus,
    pub home_nav_selected: usize,
    pub home_selected: usize,
    pub settings_selected: usize,
    pub monitor_selected: usize,
    pub navigation_motion: Option<NavigationMotion>,
    pub background_enabled: bool,
    pub theme: Theme,
    pub font_profile: FontProfile,
    pub settings_notice: Option<String>,
    pub machine_groups: Vec<MachineGroup>,
    pub group_draft: String,
    pub group_error: Option<String>,
    pub home_notice: Option<String>,
    pub remote_hosts: Vec<HostTarget>,
    pub remote_selected: usize,
    pub remote_install_draft: String,
    pub remote_install_error: Option<String>,
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
    pub fn new(remote_hosts: Vec<HostTarget>, machine_groups: Vec<MachineGroup>) -> Self {
        Self {
            local_name: local_machine_name(),
            hosts: Vec::new(),
            selected: 0,
            view: View::Welcome,
            welcome_selected: 0,
            agent_confirm_selected: 1,
            home_focus: HomeFocus::Groups,
            home_nav_selected: 0,
            home_selected: 0,
            settings_selected: 0,
            monitor_selected: 0,
            navigation_motion: None,
            background_enabled: true,
            theme: Theme::default(),
            font_profile: FontProfile::default(),
            settings_notice: None,
            machine_groups,
            group_draft: String::new(),
            group_error: None,
            home_notice: None,
            remote_hosts,
            remote_selected: 0,
            remote_install_draft: String::new(),
            remote_install_error: None,
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
            local_name: local_machine_name(),
            hosts,
            selected: 0,
            view: View::Overview,
            welcome_selected: 0,
            agent_confirm_selected: 1,
            home_focus: HomeFocus::Groups,
            home_nav_selected: 0,
            home_selected: 0,
            settings_selected: 0,
            monitor_selected: 0,
            navigation_motion: None,
            background_enabled: true,
            theme: Theme::default(),
            font_profile: FontProfile::default(),
            settings_notice: None,
            machine_groups: Vec::new(),
            group_draft: String::new(),
            group_error: None,
            home_notice: None,
            remote_hosts: Vec::new(),
            remote_selected: 0,
            remote_install_draft: String::new(),
            remote_install_error: None,
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

    pub fn home(remote_hosts: Vec<HostTarget>, machine_groups: Vec<MachineGroup>) -> Self {
        let mut app = Self::new(remote_hosts, machine_groups);
        app.view = View::Home;
        app
    }

    pub fn start_monitoring(&mut self, target: HostTarget) {
        let from = shell_navigation_index(self.view).unwrap_or(1);
        self.hosts = vec![HostState::new(target)];
        self.selected = 0;
        self.monitor_selected = 0;
        self.by_id = self
            .hosts
            .iter()
            .enumerate()
            .map(|(index, host)| (host.target.id.clone(), index))
            .collect();
        self.start_navigation_motion(from, 1, 10);
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
        if host.network_rx_history.len() == 120 {
            host.network_rx_history.pop_front();
            host.network_tx_history.pop_front();
        }
        host.network_rx_history
            .push_back(snapshot.network.read_per_sec.max(0.0).round() as u64);
        host.network_tx_history
            .push_back(snapshot.network.write_per_sec.max(0.0).round() as u64);
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
            self.start_navigation_motion(1, 1, 8);
            self.view = View::Detail;
        }
    }

    pub fn open_overview(&mut self) {
        self.start_navigation_motion(1, 1, 8);
        self.view = View::Overview;
    }

    pub fn open_welcome(&mut self) {
        self.view = View::Welcome;
    }

    pub fn open_home(&mut self) {
        let from = shell_navigation_index(self.view).unwrap_or(0);
        self.hosts.clear();
        self.by_id.clear();
        self.selected = 0;
        self.home_focus = HomeFocus::Groups;
        self.home_nav_selected = 0;
        self.home_selected = 0;
        self.home_notice = None;
        self.start_navigation_motion(from, 0, 8);
        self.view = View::Home;
    }

    pub fn next_home_item(&mut self) {
        self.home_selected = (self.home_selected + 1) % self.home_item_count();
        self.home_notice = None;
    }

    pub fn previous_home_item(&mut self) {
        self.home_selected = self
            .home_selected
            .checked_sub(1)
            .unwrap_or(self.home_item_count() - 1);
        self.home_notice = None;
    }

    pub fn home_item_count(&self) -> usize {
        self.machine_groups.len() + 2
    }

    pub fn toggle_home_focus(&mut self) {
        self.home_focus = match self.home_focus {
            HomeFocus::Navigation => HomeFocus::Groups,
            HomeFocus::Groups => HomeFocus::Navigation,
        };
        self.home_notice = None;
    }

    pub fn next_home_nav(&mut self) {
        self.home_nav_selected = (self.home_nav_selected + 1) % 3;
    }

    pub fn previous_home_nav(&mut self) {
        self.home_nav_selected = self.home_nav_selected.checked_sub(1).unwrap_or(2);
    }

    pub fn begin_group_creation(&mut self) {
        self.group_draft.clear();
        self.group_error = None;
        self.view = View::CreateGroup;
    }

    pub fn push_group_character(&mut self, character: char) {
        if self.group_draft.chars().count() < 32 && !character.is_control() {
            self.group_draft.push(character);
            self.group_error = None;
        }
    }

    pub fn pop_group_character(&mut self) {
        self.group_draft.pop();
        self.group_error = None;
    }

    pub fn cancel_group_creation(&mut self) {
        self.group_draft.clear();
        self.group_error = None;
        self.view = View::Home;
    }

    pub fn create_group(&mut self) -> Result<(), String> {
        let name = self.group_draft.trim();
        if name.is_empty() {
            let message = "Give this group a name.".to_owned();
            self.group_error = Some(message.clone());
            return Err(message);
        }
        if self
            .machine_groups
            .iter()
            .any(|group| group.name.eq_ignore_ascii_case(name))
        {
            let message = "A group with this name already exists.".to_owned();
            self.group_error = Some(message.clone());
            return Err(message);
        }
        self.machine_groups
            .push(MachineGroup::empty(name.to_owned()));
        self.home_selected = self.machine_groups.len();
        self.group_draft.clear();
        self.group_error = None;
        self.home_notice = Some("Group created. Add machines from Machines.".into());
        self.view = View::Home;
        Ok(())
    }

    pub fn show_empty_group_notice(&mut self) {
        self.home_notice = Some("This group is empty. Add machines from Machines.".into());
    }

    pub fn show_home_notice(&mut self, message: String) {
        self.home_notice = Some(message);
    }

    pub fn open_settings(&mut self) {
        let from = shell_navigation_index(self.view).unwrap_or(2);
        self.start_navigation_motion(from, 2, 8);
        self.view = View::Settings;
    }

    pub fn next_setting(&mut self) {
        self.settings_selected = (self.settings_selected + 1) % 5;
        self.settings_notice = None;
    }

    pub fn previous_setting(&mut self) {
        self.settings_selected = self.settings_selected.checked_sub(1).unwrap_or(4);
        self.settings_notice = None;
    }

    pub fn toggle_background(&mut self) {
        self.background_enabled = !self.background_enabled;
        self.settings_notice = Some(if self.background_enabled {
            "nekoHub background enabled.".into()
        } else {
            "Terminal transparency is now visible.".into()
        });
    }

    pub fn next_theme(&mut self) {
        self.theme = self.theme.next();
        self.settings_notice = Some(format!("{} theme applied.", self.theme.label()));
    }

    pub fn previous_theme(&mut self) {
        self.theme = self.theme.previous();
        self.settings_notice = Some(format!("{} theme applied.", self.theme.label()));
    }

    pub fn next_font_profile(&mut self) {
        self.font_profile = self.font_profile.next();
        self.settings_notice = Some(format!(
            "{} interface lettering applied.",
            self.font_profile.label()
        ));
    }

    pub fn previous_font_profile(&mut self) {
        self.font_profile = self.font_profile.previous();
        self.settings_notice = Some(format!(
            "{} interface lettering applied.",
            self.font_profile.label()
        ));
    }

    pub fn open_agent_confirmation(&mut self) {
        self.agent_confirm_selected = 1;
        self.view = View::AgentConfirm;
    }

    pub fn toggle_agent_confirmation(&mut self) {
        self.agent_confirm_selected = 1 - self.agent_confirm_selected;
    }

    pub fn open_remote_picker(&mut self) {
        let from = shell_navigation_index(self.view).unwrap_or(1);
        self.remote_return_view = self.view;
        self.start_navigation_motion(from, 1, 8);
        self.view = View::RemotePicker;
        self.remote_notice = None;
    }

    pub fn close_remote_picker(&mut self) {
        let target = self.remote_return_view;
        let to = shell_navigation_index(target).unwrap_or(0);
        self.start_navigation_motion(1, to, 8);
        self.view = target;
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
        let motion_finished = if let Some(motion) = &mut self.navigation_motion {
            motion.frame = motion.frame.saturating_add(1);
            motion.frame >= motion.total_frames
        } else {
            false
        };
        if motion_finished {
            self.navigation_motion = None;
        }
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
        self.remote_selected = (self.remote_selected + 1) % self.remote_item_count();
        self.remote_notice = None;
    }

    pub fn previous_remote(&mut self) {
        self.remote_selected = self
            .remote_selected
            .checked_sub(1)
            .unwrap_or(self.remote_item_count() - 1);
        self.remote_notice = None;
    }

    pub fn remote_item_count(&self) -> usize {
        self.remote_hosts.len() + 2
    }

    pub fn remote_install_selected(&self) -> bool {
        self.remote_selected + 1 == self.remote_item_count()
    }

    pub fn begin_remote_install(&mut self) {
        self.remote_install_draft.clear();
        self.remote_install_error = None;
        self.remote_notice = None;
        self.view = View::RemoteInstall;
    }

    pub fn push_remote_install_character(&mut self, character: char) {
        if self.remote_install_draft.chars().count() < 128 && !character.is_control() {
            self.remote_install_draft.push(character);
            self.remote_install_error = None;
        }
    }

    pub fn pop_remote_install_character(&mut self) {
        self.remote_install_draft.pop();
        self.remote_install_error = None;
    }

    pub fn cancel_remote_install(&mut self) {
        self.remote_install_draft.clear();
        self.remote_install_error = None;
        self.view = View::RemotePicker;
    }

    pub fn remote_install_target(&mut self) -> Result<String, String> {
        let target = self.remote_install_draft.trim();
        let valid_characters = target
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".-_@:%[]".contains(character));
        let valid_shape = target.split_once('@').is_some_and(|(user, host)| {
            !user.is_empty() && !host.is_empty() && !host.contains('@')
        });
        if !valid_characters || !valid_shape || target.starts_with('-') {
            let message = "Use the SSH destination format user@machine.".to_owned();
            self.remote_install_error = Some(message.clone());
            return Err(message);
        }
        Ok(target.to_owned())
    }

    pub fn complete_remote_install(&mut self, target: &str) {
        if let Some(machine) = self
            .remote_hosts
            .iter_mut()
            .find(|machine| machine.alias == target)
        {
            if !machine.tags.iter().any(|tag| tag == INSTALLED_TAG) {
                machine.tags.push(INSTALLED_TAG.into());
            }
        } else {
            let mut machine = HostTarget::from_alias(target);
            machine.tags.push(INSTALLED_TAG.into());
            self.remote_hosts.push(machine);
        }
        self.remote_selected = self
            .remote_hosts
            .iter()
            .position(|machine| machine.alias == target)
            .map_or(0, |index| index + 1);
        self.remote_install_draft.clear();
        self.remote_install_error = None;
        self.remote_notice = Some(format!(
            "Agent installed on {target}. The machine is now registered in nekoHub."
        ));
        self.view = View::RemotePicker;
    }

    pub fn fail_remote_install(&mut self, message: String) {
        self.remote_install_error = Some(message);
        self.view = View::RemoteInstall;
    }

    pub fn explain_remote_pairing(&mut self) {
        let installed = self
            .remote_selected
            .checked_sub(1)
            .and_then(|index| self.remote_hosts.get(index))
            .is_some_and(|machine| machine.tags.iter().any(|tag| tag == INSTALLED_TAG));
        self.remote_notice = Some(if installed {
            "The agent is installed and registered. Secure metric pairing is the next connection step; SSH is not used for monitoring.".into()
        } else {
            "This SSH host does not have a registered nekoHub agent yet. Select “Install agent over SSH” below to prepare it.".into()
        });
    }

    pub fn online_count(&self) -> usize {
        self.hosts.iter().filter(|host| host.is_online()).count()
    }

    pub fn next_monitor_section(&mut self) {
        self.monitor_selected = (self.monitor_selected + 1) % 7;
    }

    pub fn previous_monitor_section(&mut self) {
        self.monitor_selected = self.monitor_selected.checked_sub(1).unwrap_or(6);
    }

    fn start_navigation_motion(&mut self, from: usize, to: usize, total_frames: u8) {
        self.navigation_motion = Some(NavigationMotion {
            from,
            to,
            frame: 0,
            total_frames,
        });
    }
}

fn shell_navigation_index(view: View) -> Option<usize> {
    match view {
        View::Home | View::CreateGroup => Some(0),
        View::RemotePicker | View::RemoteInstall | View::Overview | View::Detail => Some(1),
        View::Settings => Some(2),
        View::Welcome | View::AgentConfirm | View::AgentSetup => None,
    }
}

pub fn local_machine_name() -> String {
    std::fs::read_to_string("/proc/sys/kernel/hostname")
        .ok()
        .and_then(|name| non_empty_machine_name(&name))
        .or_else(|| {
            std::env::var("HOSTNAME")
                .ok()
                .and_then(|name| non_empty_machine_name(&name))
        })
        .or_else(|| {
            std::env::var("COMPUTERNAME")
                .ok()
                .and_then(|name| non_empty_machine_name(&name))
        })
        .unwrap_or_else(|| "Local machine".to_owned())
}

fn non_empty_machine_name(name: &str) -> Option<String> {
    let name = name.trim();
    (!name.is_empty()).then(|| name.to_owned())
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
        let mut app = App::new(Vec::new(), Vec::new());
        app.start_agent_setup();
        app.update_agent_setup(70, "Validating metrics".into());
        app.advance_animation();
        assert!(app.setup_visible_progress > 0);
        assert!(app.setup_visible_progress < 70);
    }

    #[test]
    fn agent_confirmation_defaults_to_no() {
        let mut app = App::new(Vec::new(), Vec::new());
        app.open_agent_confirmation();
        assert_eq!(app.agent_confirm_selected, 1);
        app.toggle_agent_confirmation();
        assert_eq!(app.agent_confirm_selected, 0);
    }

    #[test]
    fn home_menu_wraps_and_opens_after_setup() {
        let mut app = App::new(Vec::new(), Vec::new());
        app.open_home();
        assert_eq!(app.view, View::Home);
        app.previous_home_item();
        assert_eq!(app.home_selected, 1);
        app.next_home_item();
        assert_eq!(app.home_selected, 0);
    }

    #[test]
    fn creates_named_machine_groups() {
        let mut app = App::home(Vec::new(), Vec::new());
        app.begin_group_creation();
        for character in "Production".chars() {
            app.push_group_character(character);
        }
        app.create_group().unwrap();
        assert_eq!(app.machine_groups[0].name, "Production");
        assert_eq!(app.view, View::Home);
        assert_eq!(app.home_selected, 1);
    }

    #[test]
    fn appearance_background_can_be_toggled() {
        let mut app = App::home(Vec::new(), Vec::new());
        assert!(app.background_enabled);
        app.toggle_background();
        assert!(!app.background_enabled);
        assert!(app.settings_notice.is_some());
    }

    #[test]
    fn top_level_navigation_animates_and_settles() {
        let mut app = App::home(Vec::new(), Vec::new());
        app.open_settings();
        let motion = app.navigation_motion.expect("navigation should animate");
        assert_eq!((motion.from, motion.to), (0, 2));
        for _ in 0..motion.total_frames {
            app.advance_animation();
        }
        assert!(app.navigation_motion.is_none());
    }

    #[test]
    fn machine_sections_wrap() {
        let mut app = App::monitoring(vec![HostTarget::from_alias("local")]);
        app.previous_monitor_section();
        assert_eq!(app.monitor_selected, 6);
        app.next_monitor_section();
        assert_eq!(app.monitor_selected, 0);
    }

    #[test]
    fn remote_install_requires_user_and_machine() {
        let mut app = App::home(Vec::new(), Vec::new());
        app.begin_remote_install();
        for character in "root@edge-01".chars() {
            app.push_remote_install_character(character);
        }
        assert_eq!(app.remote_install_target().unwrap(), "root@edge-01");
        app.remote_install_draft = "edge-01".into();
        assert!(app.remote_install_target().is_err());
    }

    #[test]
    fn completed_remote_install_registers_agent() {
        let mut app = App::home(Vec::new(), Vec::new());
        app.complete_remote_install("ops@server");
        assert_eq!(app.remote_hosts.len(), 1);
        assert!(
            app.remote_hosts[0]
                .tags
                .iter()
                .any(|tag| tag == INSTALLED_TAG)
        );
        assert_eq!(app.remote_selected, 1);
    }

    #[test]
    fn theme_and_lettering_choices_cycle() {
        let mut app = App::home(Vec::new(), Vec::new());
        app.next_theme();
        app.next_font_profile();
        assert_eq!(app.theme, Theme::Blue);
        assert_eq!(app.font_profile, FontProfile::Compact);
    }
}
