use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    path::PathBuf,
};

use nekohub_core::{HostSnapshot, HostTarget, StorageSnapshot};

use crate::{
    credentials::CredentialStore,
    groups::MachineGroup,
    machines::{CUSTOM_ALIAS_TAG, INSTALLED_TAG},
    preferences::{CustomTheme, FontProfile, Theme},
    ssh_terminal::{TerminalPanel, TerminalRequest},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Welcome,
    AgentConfirm,
    AgentSetup,
    Home,
    CreateGroup,
    GroupDetail,
    GroupAssign,
    RemotePicker,
    MachineAlias,
    RemoteInstall,
    RemoteConnect,
    RemoteUninstallConfirm,
    RemoteInstallProgress,
    Settings,
    ThemeImport,
    CredentialKeyEdit,
    AgentUpdateAuth,
    AgentUpdateProgress,
    Overview,
    Detail,
    TerminalPassword,
    TerminalSavePassword,
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HomeFocus {
    Navigation,
    Machines,
    Groups,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsFocus {
    Sidebar,
    Content,
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
    pub agent_version_hint: Option<String>,
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
            agent_version_hint: None,
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
#[allow(clippy::struct_excessive_bools)]
pub struct App {
    pub local_name: String,
    pub local_alias: Option<String>,
    pub last_machine_id: Option<String>,
    pub hosts: Vec<HostState>,
    pub selected: usize,
    pub view: View,
    pub welcome_selected: usize,
    pub agent_confirm_selected: usize,
    pub home_focus: HomeFocus,
    pub home_nav_selected: usize,
    pub home_machine_selected: usize,
    pub home_selected: usize,
    pub settings_selected: usize,
    pub settings_focus: SettingsFocus,
    pub settings_item_selected: usize,
    pub credentials: CredentialStore,
    pub credential_key_draft: String,
    pub credential_key_error: Option<String>,
    pub terminal_save_selected: usize,
    pub agent_update_quote_tick: Option<u64>,
    pub monitor_selected: usize,
    pub storage_snapshot: Option<StorageSnapshot>,
    pub storage_loading: bool,
    pub storage_error: Option<String>,
    pub storage_host_id: Option<String>,
    pub storage_hovered: Option<usize>,
    pub storage_path: String,
    pub storage_history: Vec<String>,
    pub terminal: TerminalPanel,
    pub navigation_motion: Option<NavigationMotion>,
    pub background_enabled: bool,
    pub theme: Theme,
    pub font_profile: FontProfile,
    pub custom_theme: Option<CustomTheme>,
    pub settings_notice: Option<String>,
    pub theme_import_draft: String,
    pub theme_import_error: Option<String>,
    pub machine_groups: Vec<MachineGroup>,
    pub group_draft: String,
    pub group_error: Option<String>,
    pub active_group: usize,
    pub group_machine_selected: usize,
    pub group_assign_selected: usize,
    pub home_notice: Option<String>,
    pub remote_hosts: Vec<HostTarget>,
    pub remote_selected: usize,
    pub machine_alias_draft: String,
    pub machine_alias_error: Option<String>,
    pub remote_install_draft: String,
    pub remote_password_draft: String,
    pub remote_install_field: usize,
    pub remote_install_error: Option<String>,
    pub remote_install_progress: u16,
    pub remote_install_target_progress: u16,
    pub remote_install_message: String,
    pub remote_install_logs: VecDeque<String>,
    pub remote_install_complete: bool,
    pub remote_uninstalling: bool,
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
            local_alias: None,
            last_machine_id: None,
            hosts: Vec::new(),
            selected: 0,
            view: View::Welcome,
            welcome_selected: 0,
            agent_confirm_selected: 1,
            home_focus: HomeFocus::Groups,
            home_nav_selected: 0,
            home_machine_selected: 0,
            home_selected: 0,
            settings_selected: 0,
            settings_focus: SettingsFocus::Sidebar,
            settings_item_selected: 0,
            credentials: CredentialStore::default(),
            credential_key_draft: String::new(),
            credential_key_error: None,
            terminal_save_selected: 1,
            agent_update_quote_tick: None,
            monitor_selected: 0,
            storage_snapshot: None,
            storage_loading: false,
            storage_error: None,
            storage_host_id: None,
            storage_hovered: None,
            storage_path: "/".into(),
            storage_history: Vec::new(),
            terminal: TerminalPanel::default(),
            navigation_motion: None,
            background_enabled: true,
            theme: Theme::default(),
            font_profile: FontProfile::default(),
            custom_theme: None,
            settings_notice: None,
            theme_import_draft: String::new(),
            theme_import_error: None,
            machine_groups,
            group_draft: String::new(),
            group_error: None,
            active_group: 0,
            group_machine_selected: 0,
            group_assign_selected: 0,
            home_notice: None,
            remote_hosts,
            remote_selected: 0,
            machine_alias_draft: String::new(),
            machine_alias_error: None,
            remote_install_draft: String::new(),
            remote_password_draft: String::new(),
            remote_install_field: 0,
            remote_install_error: None,
            remote_install_progress: 0,
            remote_install_target_progress: 0,
            remote_install_message: String::new(),
            remote_install_logs: VecDeque::with_capacity(12),
            remote_install_complete: false,
            remote_uninstalling: false,
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
            local_alias: None,
            last_machine_id: None,
            hosts,
            selected: 0,
            view: View::Overview,
            welcome_selected: 0,
            agent_confirm_selected: 1,
            home_focus: HomeFocus::Groups,
            home_nav_selected: 0,
            home_machine_selected: 0,
            home_selected: 0,
            settings_selected: 0,
            settings_focus: SettingsFocus::Sidebar,
            settings_item_selected: 0,
            credentials: CredentialStore::default(),
            credential_key_draft: String::new(),
            credential_key_error: None,
            terminal_save_selected: 1,
            agent_update_quote_tick: None,
            monitor_selected: 0,
            storage_snapshot: None,
            storage_loading: false,
            storage_error: None,
            storage_host_id: None,
            storage_hovered: None,
            storage_path: "/".into(),
            storage_history: Vec::new(),
            terminal: TerminalPanel::default(),
            navigation_motion: None,
            background_enabled: true,
            theme: Theme::default(),
            font_profile: FontProfile::default(),
            custom_theme: None,
            settings_notice: None,
            theme_import_draft: String::new(),
            theme_import_error: None,
            machine_groups: Vec::new(),
            group_draft: String::new(),
            group_error: None,
            active_group: 0,
            group_machine_selected: 0,
            group_assign_selected: 0,
            home_notice: None,
            remote_hosts: Vec::new(),
            remote_selected: 0,
            machine_alias_draft: String::new(),
            machine_alias_error: None,
            remote_install_draft: String::new(),
            remote_password_draft: String::new(),
            remote_install_field: 0,
            remote_install_error: None,
            remote_install_progress: 0,
            remote_install_target_progress: 0,
            remote_install_message: String::new(),
            remote_install_logs: VecDeque::with_capacity(12),
            remote_install_complete: false,
            remote_uninstalling: false,
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
        self.last_machine_id = Some(target.id.clone());
        self.hosts = vec![HostState::new(target)];
        self.selected = 0;
        self.monitor_selected = 0;
        self.storage_snapshot = None;
        self.storage_loading = false;
        self.storage_error = None;
        self.storage_host_id = None;
        self.storage_hovered = None;
        self.storage_path = "/".into();
        self.storage_history.clear();
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
        if !snapshot.agent_version.is_empty() {
            host.agent_version_hint = Some(snapshot.agent_version.clone());
        }
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

    pub fn apply_agent_version(&mut self, host_id: &str, version: String) {
        if version.is_empty() {
            return;
        }
        if let Some(index) = self.by_id.get(host_id).copied() {
            self.hosts[index].agent_version_hint = Some(version);
        }
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
        self.home_machine_selected = 0;
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
        self.machine_groups.len() + 1
    }

    pub fn toggle_home_focus(&mut self) {
        self.home_focus = match self.home_focus {
            HomeFocus::Navigation => HomeFocus::Machines,
            HomeFocus::Machines => HomeFocus::Groups,
            HomeFocus::Groups => HomeFocus::Navigation,
        };
        self.home_notice = None;
    }

    pub fn previous_home_focus(&mut self) {
        self.home_focus = match self.home_focus {
            HomeFocus::Navigation => HomeFocus::Groups,
            HomeFocus::Machines => HomeFocus::Navigation,
            HomeFocus::Groups => HomeFocus::Machines,
        };
        self.home_notice = None;
    }

    pub fn next_home_machine(&mut self) {
        self.home_machine_selected =
            (self.home_machine_selected + 1) % (self.remote_hosts.len() + 1);
    }

    pub fn previous_home_machine(&mut self) {
        self.home_machine_selected = self
            .home_machine_selected
            .checked_sub(1)
            .unwrap_or(self.remote_hosts.len());
    }

    pub fn home_machine_order(&self) -> Vec<usize> {
        let mut order = (0..=self.remote_hosts.len()).collect::<Vec<_>>();
        let recent = self.last_machine_id.as_deref().and_then(|last_id| {
            if last_id == "local" {
                Some(0)
            } else {
                self.remote_hosts
                    .iter()
                    .position(|machine| machine.id == last_id)
                    .map(|index| index + 1)
            }
        });
        if let Some(recent) = recent {
            order.retain(|index| *index != recent);
            order.insert(0, recent);
        }
        order
    }

    pub fn selected_home_machine_index(&self) -> usize {
        self.home_machine_order()
            .get(self.home_machine_selected)
            .copied()
            .unwrap_or_default()
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
        self.home_selected = self.machine_groups.len().saturating_sub(1);
        self.group_draft.clear();
        self.group_error = None;
        self.home_notice = Some("Group created. Add machines from Machines.".into());
        self.view = View::Home;
        Ok(())
    }

    pub fn open_group(&mut self) {
        if self.home_selected < self.machine_groups.len() {
            self.active_group = self.home_selected;
            self.group_machine_selected = self
                .last_machine_id
                .as_ref()
                .and_then(|last_id| {
                    self.machine_groups[self.active_group]
                        .host_ids
                        .iter()
                        .position(|id| id == last_id)
                })
                .unwrap_or_default();
            self.view = View::GroupDetail;
        }
    }

    pub fn close_group(&mut self) {
        self.view = View::Home;
    }

    pub fn next_group_machine(&mut self) {
        let count = self
            .machine_groups
            .get(self.active_group)
            .map_or(0, |group| group.host_ids.len());
        if count > 0 {
            self.group_machine_selected = (self.group_machine_selected + 1) % count;
        }
    }

    pub fn previous_group_machine(&mut self) {
        let count = self
            .machine_groups
            .get(self.active_group)
            .map_or(0, |group| group.host_ids.len());
        if count > 0 {
            self.group_machine_selected = self
                .group_machine_selected
                .checked_sub(1)
                .unwrap_or(count - 1);
        }
    }

    pub fn selected_group_machine_id(&self) -> Option<&str> {
        self.machine_groups
            .get(self.active_group)?
            .host_ids
            .get(self.group_machine_selected)
            .map(String::as_str)
    }

    pub fn begin_group_assignment(&mut self) {
        if self.machine_groups.is_empty() {
            self.remote_notice = Some("Create a group from Home first.".into());
            return;
        }
        if self.remote_selected > self.remote_hosts.len() {
            self.remote_notice = Some("Select a machine first.".into());
            return;
        }
        self.group_assign_selected = 0;
        self.view = View::GroupAssign;
    }

    pub fn next_group_assignment(&mut self) {
        self.group_assign_selected = (self.group_assign_selected + 1) % self.machine_groups.len();
    }

    pub fn previous_group_assignment(&mut self) {
        self.group_assign_selected = self
            .group_assign_selected
            .checked_sub(1)
            .unwrap_or(self.machine_groups.len() - 1);
    }

    pub fn apply_group_assignment(&mut self) {
        let machine_id = if self.remote_selected == 0 {
            "local".to_owned()
        } else {
            self.remote_hosts[self.remote_selected - 1].id.clone()
        };
        let group = &mut self.machine_groups[self.group_assign_selected];
        if let Some(index) = group.host_ids.iter().position(|id| id == &machine_id) {
            group.host_ids.remove(index);
            self.remote_notice = Some(format!("Machine removed from {}.", group.name));
        } else {
            group.host_ids.push(machine_id);
            self.remote_notice = Some(format!("Machine added to {}.", group.name));
        }
        self.view = View::RemotePicker;
    }

    pub fn cancel_group_assignment(&mut self) {
        self.view = View::RemotePicker;
    }

    pub fn begin_machine_alias(&mut self) {
        if self.remote_selected > self.remote_hosts.len() {
            self.remote_notice = Some("Select a machine before editing its alias.".into());
            return;
        }
        self.machine_alias_draft = if self.remote_selected == 0 {
            self.local_name.clone()
        } else {
            self.remote_hosts[self.remote_selected - 1]
                .display_name
                .clone()
        };
        self.machine_alias_error = None;
        self.remote_notice = None;
        self.view = View::MachineAlias;
    }

    pub fn push_machine_alias_character(&mut self, character: char) {
        if self.machine_alias_draft.chars().count() < 32 && !character.is_control() {
            self.machine_alias_draft.push(character);
            self.machine_alias_error = None;
        }
    }

    pub fn pop_machine_alias_character(&mut self) {
        self.machine_alias_draft.pop();
        self.machine_alias_error = None;
    }

    pub fn clear_machine_alias(&mut self) {
        self.machine_alias_draft.clear();
        self.machine_alias_error = None;
    }

    pub fn apply_machine_alias(&mut self) -> Result<(), String> {
        let alias = self.machine_alias_draft.trim();
        if alias.chars().count() > 32 {
            let message = "Aliases can contain at most 32 characters.".to_owned();
            self.machine_alias_error = Some(message.clone());
            return Err(message);
        }
        if self.remote_selected == 0 {
            self.local_alias = (!alias.is_empty()).then(|| alias.to_owned());
            self.local_name = self.local_alias.clone().unwrap_or_else(local_machine_name);
        } else if let Some(machine) = self.remote_hosts.get_mut(self.remote_selected - 1) {
            if alias.is_empty() {
                machine.display_name.clone_from(&machine.alias);
                machine.tags.retain(|tag| tag != CUSTOM_ALIAS_TAG);
            } else {
                alias.clone_into(&mut machine.display_name);
                if !machine.tags.iter().any(|tag| tag == CUSTOM_ALIAS_TAG) {
                    machine.tags.push(CUSTOM_ALIAS_TAG.into());
                }
            }
        }
        self.machine_alias_draft.clear();
        self.machine_alias_error = None;
        self.remote_notice = Some("Machine alias saved.".into());
        self.view = View::RemotePicker;
        Ok(())
    }

    pub fn cancel_machine_alias(&mut self) {
        self.machine_alias_draft.clear();
        self.machine_alias_error = None;
        self.view = View::RemotePicker;
    }

    pub fn show_home_notice(&mut self, message: String) {
        self.home_notice = Some(message);
    }

    pub fn open_settings(&mut self) {
        let from = shell_navigation_index(self.view).unwrap_or(2);
        self.start_navigation_motion(from, 2, 8);
        self.view = View::Settings;
        self.settings_focus = SettingsFocus::Sidebar;
    }

    pub fn next_setting(&mut self) {
        if self.settings_focus == SettingsFocus::Sidebar {
            self.settings_selected = (self.settings_selected + 1) % 6;
        } else {
            self.settings_item_selected =
                (self.settings_item_selected + 1) % self.settings_item_count();
        }
        self.settings_notice = None;
    }

    pub fn previous_setting(&mut self) {
        if self.settings_focus == SettingsFocus::Sidebar {
            self.settings_selected = self.settings_selected.checked_sub(1).unwrap_or(5);
        } else {
            self.settings_item_selected = self
                .settings_item_selected
                .checked_sub(1)
                .unwrap_or(self.settings_item_count() - 1);
        }
        self.settings_notice = None;
    }

    pub fn enter_settings_content(&mut self) {
        self.settings_focus = SettingsFocus::Content;
        self.settings_item_selected = 0;
    }

    pub fn leave_settings_content(&mut self) {
        self.settings_focus = SettingsFocus::Sidebar;
    }

    pub fn toggle_settings_focus(&mut self) {
        if self.settings_focus == SettingsFocus::Sidebar {
            self.enter_settings_content();
        } else {
            self.leave_settings_content();
        }
    }

    fn settings_item_count(&self) -> usize {
        match self.settings_selected {
            0 => 2,
            1 => 5,
            2 => self.credential_machines().len().max(1),
            _ => 1,
        }
    }

    pub fn credential_machines(&self) -> Vec<(String, String)> {
        let mut machines = BTreeMap::new();
        for host in self
            .remote_hosts
            .iter()
            .chain(self.hosts.iter().map(|host| &host.target))
        {
            if host.id != "local" {
                machines.insert(host.alias.clone(), host.display_name.clone());
            }
        }
        for alias in self.credentials.aliases() {
            machines
                .entry(alias.to_owned())
                .or_insert_with(|| alias.to_owned());
        }
        machines.into_iter().collect()
    }

    pub fn selected_credential_alias(&self) -> Option<String> {
        self.credential_machines()
            .get(self.settings_item_selected)
            .map(|(alias, _)| alias.clone())
    }

    pub fn begin_credential_key_edit(&mut self) {
        let Some(alias) = self.selected_credential_alias() else {
            self.settings_notice = Some("No SSH machine is available yet.".into());
            return;
        };
        self.credential_key_draft = self
            .credentials
            .get(&alias)
            .and_then(|credential| credential.identity_file.as_ref())
            .map_or_else(String::new, |path| path.display().to_string());
        self.credential_key_error = None;
        self.view = View::CredentialKeyEdit;
    }

    pub fn push_credential_key_character(&mut self, character: char) {
        if self.credential_key_draft.chars().count() < 512 && !character.is_control() {
            self.credential_key_draft.push(character);
            self.credential_key_error = None;
        }
    }

    pub fn pop_credential_key_character(&mut self) {
        self.credential_key_draft.pop();
        self.credential_key_error = None;
    }

    pub fn save_credential_key(&mut self) -> Result<(), String> {
        let alias = self
            .selected_credential_alias()
            .ok_or_else(|| "No SSH machine is selected.".to_owned())?;
        let draft = self.credential_key_draft.trim();
        if draft.is_empty() {
            return Err("Enter the path to a private SSH key.".into());
        }
        let path = expand_home_path(draft);
        if !path.is_file() {
            return Err("The selected private key file does not exist.".into());
        }
        self.credentials.save_identity_file(&alias, path);
        self.credential_key_draft.clear();
        self.credential_key_error = None;
        self.settings_notice = Some(format!("SSH key saved for {alias}."));
        self.view = View::Settings;
        Ok(())
    }

    pub fn cancel_credential_key_edit(&mut self) {
        self.credential_key_draft.clear();
        self.credential_key_error = None;
        self.view = View::Settings;
    }

    pub fn remove_selected_password(&mut self) {
        let Some(alias) = self.selected_credential_alias() else {
            return;
        };
        let removed = self.credentials.remove_password(&alias);
        self.settings_notice = Some(if removed {
            format!("Saved password removed from {alias}.")
        } else {
            format!("{alias} has no saved password.")
        });
    }

    pub fn remove_selected_identity_file(&mut self) {
        let Some(alias) = self.selected_credential_alias() else {
            return;
        };
        let removed = self.credentials.remove_identity_file(&alias);
        self.settings_notice = Some(if removed {
            format!("SSH key removed from {alias}.")
        } else {
            format!("{alias} has no managed SSH key.")
        });
    }

    pub fn toggle_background(&mut self) {
        self.background_enabled = !self.background_enabled;
        self.settings_notice = Some(if self.background_enabled {
            "nekoHub background enabled.".into()
        } else {
            "Terminal transparency is now visible.".into()
        });
    }

    pub fn apply_builtin_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.settings_notice = Some(format!("{} theme applied.", theme.label()));
    }

    pub fn begin_theme_import(&mut self) {
        self.theme_import_draft.clear();
        self.theme_import_error = None;
        self.view = View::ThemeImport;
    }

    pub fn push_theme_path_character(&mut self, character: char) {
        if self.theme_import_draft.chars().count() < 240 && !character.is_control() {
            self.theme_import_draft.push(character);
            self.theme_import_error = None;
        }
    }

    pub fn pop_theme_path_character(&mut self) {
        self.theme_import_draft.pop();
        self.theme_import_error = None;
    }

    pub fn cancel_theme_import(&mut self) {
        self.theme_import_draft.clear();
        self.theme_import_error = None;
        self.view = View::Settings;
    }

    pub fn apply_custom_theme(&mut self, theme: CustomTheme) {
        let name = theme.name.clone();
        self.custom_theme = Some(theme);
        self.theme = Theme::Custom;
        self.theme_import_draft.clear();
        self.theme_import_error = None;
        self.settings_notice = Some(format!("{name} imported and applied."));
        self.view = View::Settings;
    }

    pub fn next_font_profile(&mut self) {
        self.font_profile = self.font_profile.next();
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
        self.terminal.advance_boot(self.animation_tick);
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
        if self.view == View::RemoteInstallProgress
            && self.remote_install_error.is_none()
            && self.remote_install_progress < self.remote_install_target_progress
        {
            let remaining = self.remote_install_target_progress - self.remote_install_progress;
            let step = remaining.div_ceil(7).max(1);
            self.remote_install_progress = self
                .remote_install_progress
                .saturating_add(step)
                .min(self.remote_install_target_progress);
        }
        if self.view == View::AgentUpdateProgress
            && self.remote_install_error.is_none()
            && self.remote_install_progress < self.remote_install_target_progress
        {
            let remaining = self.remote_install_target_progress - self.remote_install_progress;
            let step = remaining.div_ceil(7).max(1);
            self.remote_install_progress = self
                .remote_install_progress
                .saturating_add(step)
                .min(self.remote_install_target_progress);
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
        self.remote_uninstalling = false;
        self.remote_install_draft.clear();
        self.remote_password_draft.clear();
        self.remote_install_field = 0;
        self.remote_install_error = None;
        self.remote_notice = None;
        self.view = View::RemoteInstall;
    }

    pub fn start_remote_install_progress(&mut self) {
        self.remote_uninstalling = false;
        self.remote_install_progress = 0;
        self.remote_install_target_progress = 2;
        self.remote_install_message = "Starting SSH connection".into();
        self.remote_install_logs.clear();
        self.remote_install_error = None;
        self.remote_install_complete = false;
        self.view = View::RemoteInstallProgress;
    }

    pub fn update_remote_install(&mut self, progress: u16, message: String) {
        self.remote_install_target_progress = progress.min(100);
        self.remote_install_message = message;
    }

    pub fn push_remote_install_log(&mut self, line: String) {
        if self.remote_install_logs.len() == 12 {
            self.remote_install_logs.pop_front();
        }
        self.remote_install_logs.push_back(line);
    }

    pub fn push_remote_install_character(&mut self, character: char) {
        let draft = if self.remote_install_field == 0 {
            &mut self.remote_install_draft
        } else {
            &mut self.remote_password_draft
        };
        if draft.chars().count() < 128 && !character.is_control() {
            draft.push(character);
            self.remote_install_error = None;
        }
    }

    pub fn pop_remote_install_character(&mut self) {
        if self.remote_install_field == 0 {
            self.remote_install_draft.pop();
        } else {
            self.remote_password_draft.pop();
        }
        self.remote_install_error = None;
    }

    pub fn toggle_remote_install_field(&mut self) {
        self.remote_install_field = 1 - self.remote_install_field;
    }

    pub fn take_remote_password(&mut self) -> Option<String> {
        (!self.remote_password_draft.is_empty())
            .then(|| std::mem::take(&mut self.remote_password_draft))
    }

    pub fn cancel_remote_install(&mut self) {
        self.remote_install_draft.clear();
        self.remote_password_draft.clear();
        self.remote_install_error = None;
        self.view = View::RemotePicker;
    }

    pub fn begin_remote_uninstall(&mut self) {
        let Some(target) = self
            .selected_installed_remote()
            .map(|machine| machine.alias.clone())
        else {
            self.remote_notice = Some("Select a machine with an installed agent first.".into());
            return;
        };
        self.remote_install_draft = target;
        self.remote_password_draft.clear();
        self.remote_install_field = 1;
        self.remote_install_error = None;
        self.remote_uninstalling = true;
        self.view = View::RemoteUninstallConfirm;
    }

    pub fn cancel_remote_uninstall(&mut self) {
        self.remote_password_draft.clear();
        self.remote_uninstalling = false;
        self.view = View::RemotePicker;
    }

    pub fn start_remote_uninstall_progress(&mut self) {
        self.remote_install_progress = 0;
        self.remote_install_target_progress = 2;
        self.remote_install_message = "Starting SSH connection".into();
        self.remote_install_logs.clear();
        self.remote_install_error = None;
        self.remote_install_complete = false;
        self.remote_uninstalling = true;
        self.view = View::RemoteInstallProgress;
    }

    pub fn selected_installed_remote(&self) -> Option<&HostTarget> {
        self.remote_selected
            .checked_sub(1)
            .and_then(|index| self.remote_hosts.get(index))
            .filter(|machine| machine.tags.iter().any(|tag| tag == INSTALLED_TAG))
    }

    pub fn begin_remote_connect(&mut self) {
        let Some(target) = self
            .selected_installed_remote()
            .map(|machine| machine.alias.clone())
        else {
            self.explain_remote_pairing();
            return;
        };
        self.remote_install_draft = target;
        self.remote_password_draft.clear();
        self.remote_install_field = 1;
        self.remote_install_error = None;
        self.remote_return_view = self.view;
        self.view = View::RemoteConnect;
    }

    pub fn cancel_remote_connect(&mut self) {
        self.remote_password_draft.clear();
        self.view = self.remote_return_view;
    }

    pub const fn remote_return_view(&self) -> View {
        self.remote_return_view
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
        self.remote_install_progress = 100;
        self.remote_install_target_progress = 100;
        self.remote_install_message = "Agent installed, paired, and streaming metrics".into();
        self.remote_install_complete = true;
        self.remote_install_error = None;
        self.view = View::RemoteInstallProgress;
    }

    pub fn complete_remote_uninstall(&mut self, target: &str) {
        if let Some(machine) = self
            .remote_hosts
            .iter_mut()
            .find(|machine| machine.alias == target)
        {
            machine.tags.retain(|tag| tag != INSTALLED_TAG);
        }
        self.remote_install_progress = 100;
        self.remote_install_target_progress = 100;
        self.remote_install_message = "Agent uninstalled and machine removed from the fleet".into();
        self.remote_install_complete = true;
        self.remote_install_error = None;
        self.view = View::RemoteInstallProgress;
    }

    pub fn fail_remote_install(&mut self, message: String) {
        self.remote_install_error = Some(message);
        self.remote_install_message = "Installation stopped".into();
        self.view = View::RemoteInstallProgress;
    }

    pub fn close_remote_install_progress(&mut self) {
        let was_uninstalling = self.remote_uninstalling;
        if self.remote_install_complete {
            self.remote_notice = Some(if self.remote_uninstalling {
                "Agent uninstalled. The SSH inventory entry remains available for reinstalling."
                    .into()
            } else {
                "Agent installed, paired, and receiving live metrics.".into()
            });
        }
        self.remote_install_draft.clear();
        self.remote_password_draft.clear();
        self.remote_uninstalling = false;
        self.view = if self.remote_install_complete && !was_uninstalling && !self.hosts.is_empty() {
            View::Overview
        } else {
            View::RemotePicker
        };
    }

    pub fn explain_remote_pairing(&mut self) {
        let installed = self
            .remote_selected
            .checked_sub(1)
            .and_then(|index| self.remote_hosts.get(index))
            .is_some_and(|machine| machine.tags.iter().any(|tag| tag == INSTALLED_TAG));
        self.remote_notice = Some(if installed {
            "The agent is installed. Press Enter to open its live metrics.".into()
        } else {
            "This SSH host does not have a registered nekoHub agent yet. Select “Install agent over SSH” below to prepare it.".into()
        });
    }

    pub fn online_count(&self) -> usize {
        self.hosts.iter().filter(|host| host.is_online()).count()
    }

    pub fn next_monitor_section(&mut self) {
        self.monitor_selected = (self.monitor_selected + 1) % 8;
        if self.monitor_selected != 3 {
            self.storage_hovered = None;
        }
    }

    pub fn previous_monitor_section(&mut self) {
        self.monitor_selected = self.monitor_selected.checked_sub(1).unwrap_or(7);
        if self.monitor_selected != 3 {
            self.storage_hovered = None;
        }
    }

    pub fn select_monitor_section(&mut self, index: usize) {
        self.monitor_selected = index.min(7);
        if self.monitor_selected != 3 {
            self.storage_hovered = None;
        }
    }

    pub fn begin_storage_scan(&mut self, host_id: &str, path: &str, force: bool) -> bool {
        if self.storage_loading {
            return false;
        }
        if !force
            && self.storage_host_id.as_deref() == Some(host_id)
            && self
                .storage_snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.root == path)
        {
            return false;
        }
        self.storage_loading = true;
        self.storage_error = None;
        self.storage_hovered = None;
        self.storage_host_id = Some(host_id.to_owned());
        self.storage_path = path.to_owned();
        true
    }

    pub fn complete_storage_scan(&mut self, host_id: &str, snapshot: StorageSnapshot) {
        if self.storage_host_id.as_deref() == Some(host_id) {
            self.storage_path.clone_from(&snapshot.root);
            self.storage_snapshot = Some(snapshot);
            self.storage_loading = false;
            self.storage_error = None;
        }
    }

    pub fn fail_storage_scan(&mut self, host_id: &str, message: String) {
        if self.storage_host_id.as_deref() == Some(host_id) {
            self.storage_loading = false;
            self.storage_error = Some(message);
        }
    }

    pub fn open_storage_entry(&mut self, index: usize) -> bool {
        if self.storage_loading {
            return false;
        }
        let Some(snapshot) = self.storage_snapshot.as_ref() else {
            return false;
        };
        let Some(path) = snapshot
            .entries
            .get(index)
            .filter(|entry| entry.is_directory)
            .map(|entry| entry.path.clone())
        else {
            return false;
        };
        self.storage_history.push(snapshot.root.clone());
        self.storage_path = path;
        self.storage_hovered = None;
        true
    }

    pub fn previous_storage_path(&mut self) -> bool {
        if self.storage_loading {
            return false;
        }
        let Some(path) = self.storage_history.pop() else {
            return false;
        };
        self.storage_path = path;
        self.storage_hovered = None;
        true
    }

    pub fn selected_agent_needs_update(&self) -> bool {
        self.selected_agent_version()
            .is_some_and(|version| version_is_older(version, env!("CARGO_PKG_VERSION")))
    }

    pub fn selected_agent_version(&self) -> Option<&str> {
        let host = self.selected()?;
        host.snapshot
            .as_ref()
            .and_then(|snapshot| {
                (!snapshot.agent_version.is_empty()).then_some(snapshot.agent_version.as_str())
            })
            .or(host.agent_version_hint.as_deref())
    }

    pub fn register_agent_update_quote(&mut self) -> bool {
        if !self.selected_agent_needs_update() {
            self.agent_update_quote_tick = None;
            return false;
        }
        let confirmed = self
            .agent_update_quote_tick
            .is_some_and(|tick| self.animation_tick.wrapping_sub(tick) <= 45);
        if confirmed {
            self.agent_update_quote_tick = None;
            self.begin_agent_update();
        } else {
            self.agent_update_quote_tick = Some(self.animation_tick);
        }
        confirmed
    }

    pub fn agent_update_is_armed(&self) -> bool {
        self.agent_update_quote_tick
            .is_some_and(|tick| self.animation_tick.wrapping_sub(tick) <= 45)
    }

    fn begin_agent_update(&mut self) {
        let Some(target) = self.selected().map(|host| host.target.alias.clone()) else {
            return;
        };
        self.remote_install_draft.clone_from(&target);
        self.remote_password_draft = self
            .credentials
            .get(&target)
            .and_then(|credential| credential.password.clone())
            .unwrap_or_default();
        self.remote_install_field = 1;
        self.remote_install_error = None;
        self.view = View::AgentUpdateAuth;
    }

    pub fn cancel_agent_update(&mut self) {
        self.remote_password_draft.clear();
        self.remote_install_error = None;
        self.view = View::Overview;
    }

    pub fn start_agent_update_progress(&mut self) {
        self.remote_install_progress = 0;
        self.remote_install_target_progress = 4;
        self.remote_install_message = "Connecting to the machine".into();
        self.remote_install_logs.clear();
        self.remote_install_error = None;
        self.remote_install_complete = false;
        self.view = View::AgentUpdateProgress;
    }

    pub fn complete_agent_update(&mut self) {
        self.remote_install_progress = 100;
        self.remote_install_target_progress = 100;
        self.remote_install_message = "Agent updated and restarted".into();
        self.remote_install_complete = true;
        self.remote_install_error = None;
        self.view = View::AgentUpdateProgress;
    }

    pub fn fail_agent_update(&mut self, message: String) {
        self.remote_install_error = Some(message);
        self.remote_install_message = "Update stopped".into();
        self.view = View::AgentUpdateProgress;
    }

    pub fn close_agent_update_progress(&mut self) {
        self.remote_password_draft.clear();
        self.remote_install_error = None;
        self.remote_install_complete = false;
        self.view = View::Overview;
    }

    pub fn begin_terminal_password(&mut self) {
        let Some(target) = self.selected().map(|host| host.target.alias.clone()) else {
            return;
        };
        let credential = self.credentials.get(&target);
        self.terminal.begin_password(
            &target,
            credential.and_then(|credential| credential.password.as_deref()),
            credential.and_then(|credential| credential.identity_file.clone()),
        );
        self.view = View::TerminalPassword;
    }

    pub fn push_terminal_password_character(&mut self, character: char) {
        self.terminal.push_password_character(character);
    }

    pub fn pop_terminal_password_character(&mut self) {
        self.terminal.pop_password_character();
    }

    pub fn start_terminal(&mut self, rows: u16, cols: u16) -> TerminalRequest {
        self.view = View::Terminal;
        self.terminal.start(rows, cols, self.animation_tick)
    }

    pub fn should_confirm_terminal_password_save(&self) -> bool {
        !self.terminal.password.is_empty() && !self.terminal.password_is_saved()
    }

    pub fn begin_terminal_password_save(&mut self) {
        self.terminal_save_selected = 1;
        self.view = View::TerminalSavePassword;
    }

    pub fn toggle_terminal_password_save(&mut self) {
        self.terminal_save_selected = 1 - self.terminal_save_selected;
    }

    pub fn save_terminal_password(&mut self) {
        if self.terminal_save_selected == 0 && !self.terminal.password.is_empty() {
            self.credentials
                .save_password(&self.terminal.target, self.terminal.password.clone());
            self.terminal.mark_password_saved();
        }
    }

    pub fn close_terminal(&mut self) {
        self.terminal.cancel();
        self.view = View::Overview;
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
        View::Home | View::CreateGroup | View::GroupDetail => Some(0),
        View::RemotePicker
        | View::MachineAlias
        | View::GroupAssign
        | View::RemoteInstall
        | View::RemoteConnect
        | View::RemoteUninstallConfirm
        | View::RemoteInstallProgress
        | View::AgentUpdateAuth
        | View::AgentUpdateProgress
        | View::Overview
        | View::Detail
        | View::TerminalPassword
        | View::TerminalSavePassword
        | View::Terminal => Some(1),
        View::Settings | View::ThemeImport | View::CredentialKeyEdit => Some(2),
        View::Welcome | View::AgentConfirm | View::AgentSetup => None,
    }
}

fn version_is_older(installed: &str, current: &str) -> bool {
    fn parts(version: &str) -> [u64; 3] {
        let mut values = version
            .trim_start_matches('v')
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or_default());
        [
            values.next().unwrap_or_default(),
            values.next().unwrap_or_default(),
            values.next().unwrap_or_default(),
        ]
    }
    parts(installed) < parts(current)
}

fn expand_home_path(value: &str) -> PathBuf {
    if let Some(relative) = value.strip_prefix("~/")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(relative);
    }
    PathBuf::from(value)
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
    fn remote_progress_animates_toward_install_stage() {
        let mut app = App::home(Vec::new(), Vec::new());
        app.start_remote_install_progress();
        app.update_remote_install(82, "Agent package installed".into());
        app.advance_animation();
        assert!(app.remote_install_progress > 0);
        assert!(app.remote_install_progress < 82);
        assert_eq!(app.remote_install_target_progress, 82);
    }

    #[test]
    fn home_focus_moves_in_both_directions() {
        let mut app = App::home(Vec::new(), Vec::new());
        assert_eq!(app.home_focus, HomeFocus::Groups);
        app.toggle_home_focus();
        assert_eq!(app.home_focus, HomeFocus::Navigation);
        app.previous_home_focus();
        assert_eq!(app.home_focus, HomeFocus::Groups);
        app.previous_home_focus();
        assert_eq!(app.home_focus, HomeFocus::Machines);
    }

    #[test]
    fn most_recent_machine_is_first_on_home_shelf() {
        let remotes = vec![
            HostTarget::from_alias("edge-01"),
            HostTarget::from_alias("edge-02"),
        ];
        let mut app = App::home(remotes, Vec::new());
        app.start_monitoring(HostTarget::from_alias("edge-02"));
        app.open_home();

        assert_eq!(app.home_machine_order(), [2, 0, 1]);
        assert_eq!(app.selected_home_machine_index(), 2);
    }

    #[test]
    fn group_machines_can_be_selected_for_monitoring() {
        let groups = vec![MachineGroup {
            name: "Production".into(),
            host_ids: vec!["local".into(), "edge-01".into()],
        }];
        let mut app = App::home(vec![HostTarget::from_alias("edge-01")], groups);
        app.open_group();

        assert_eq!(app.selected_group_machine_id(), Some("local"));
        app.next_group_machine();
        assert_eq!(app.selected_group_machine_id(), Some("edge-01"));
        app.next_group_machine();
        assert_eq!(app.selected_group_machine_id(), Some("local"));
        app.previous_group_machine();
        assert_eq!(app.selected_group_machine_id(), Some("edge-01"));
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
        assert_eq!(app.home_selected, 0);
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
        assert_eq!(app.home_selected, 0);
    }

    #[test]
    fn assigns_and_removes_a_machine_from_a_group() {
        let groups = vec![MachineGroup::empty("Production".into())];
        let mut app = App::home(Vec::new(), groups);
        app.remote_selected = 0;
        app.begin_group_assignment();
        app.apply_group_assignment();
        assert_eq!(app.machine_groups[0].host_ids, ["local"]);

        app.begin_group_assignment();
        app.apply_group_assignment();
        assert!(app.machine_groups[0].host_ids.is_empty());
    }

    #[test]
    fn assigns_and_resets_machine_aliases() {
        let mut app = App::home(vec![HostTarget::from_alias("edge-01")], Vec::new());
        app.remote_selected = 1;
        app.begin_machine_alias();
        app.clear_machine_alias();
        for character in "Media server".chars() {
            app.push_machine_alias_character(character);
        }
        app.apply_machine_alias().unwrap();
        assert_eq!(app.remote_hosts[0].display_name, "Media server");
        assert!(
            app.remote_hosts[0]
                .tags
                .iter()
                .any(|tag| tag == CUSTOM_ALIAS_TAG)
        );

        app.begin_machine_alias();
        app.clear_machine_alias();
        app.apply_machine_alias().unwrap();
        assert_eq!(app.remote_hosts[0].display_name, "edge-01");
        assert!(
            app.remote_hosts[0]
                .tags
                .iter()
                .all(|tag| tag != CUSTOM_ALIAS_TAG)
        );
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
        assert_eq!(app.monitor_selected, 7);
        app.next_monitor_section();
        assert_eq!(app.monitor_selected, 0);
    }

    #[test]
    fn storage_directories_open_and_return_to_the_parent() {
        let mut app = App::monitoring(vec![HostTarget::from_alias("local")]);
        app.storage_snapshot = Some(StorageSnapshot {
            root: "/".into(),
            total_bytes: 100,
            used_bytes: 50,
            scanned_bytes: 40,
            file_count: 2,
            unreadable_entries: 0,
            truncated: false,
            elapsed_ms: 1,
            entries: vec![
                nekohub_core::StorageEntry {
                    name: "var".into(),
                    path: "/var".into(),
                    allocated_bytes: 30,
                    file_count: 1,
                    is_directory: true,
                },
                nekohub_core::StorageEntry {
                    name: "swapfile".into(),
                    path: "/swapfile".into(),
                    allocated_bytes: 10,
                    file_count: 1,
                    is_directory: false,
                },
            ],
        });

        assert!(!app.open_storage_entry(1));
        assert!(app.open_storage_entry(0));
        assert_eq!(app.storage_path, "/var");
        assert_eq!(app.storage_history, ["/"]);
        assert!(app.previous_storage_path());
        assert_eq!(app.storage_path, "/");
        assert!(app.storage_history.is_empty());
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
    fn remote_uninstall_removes_agent_registration() {
        let mut machine = HostTarget::from_alias("ops@server");
        machine.tags.push(INSTALLED_TAG.into());
        let mut app = App::home(vec![machine], Vec::new());
        app.remote_selected = 1;

        app.begin_remote_uninstall();
        assert_eq!(app.view, View::RemoteUninstallConfirm);
        app.start_remote_uninstall_progress();
        app.complete_remote_uninstall("ops@server");

        assert!(
            app.remote_hosts[0]
                .tags
                .iter()
                .all(|tag| tag != INSTALLED_TAG)
        );
        assert_eq!(app.remote_install_progress, 100);
    }

    #[test]
    fn theme_and_lettering_choices_cycle() {
        let mut app = App::home(Vec::new(), Vec::new());
        app.apply_builtin_theme(Theme::Blue);
        app.next_font_profile();
        assert_eq!(app.theme, Theme::Blue);
        assert_eq!(app.font_profile, FontProfile::Compact);
    }

    #[test]
    fn saved_terminal_password_is_reused_without_a_second_save_prompt() {
        let mut app = App::monitoring(vec![HostTarget::from_alias("edge-01")]);
        app.credentials
            .save_password("edge-01", "saved-secret".into());

        app.begin_terminal_password();

        assert!(app.terminal.password_is_saved());
        assert!(!app.should_confirm_terminal_password_save());
        let request = app.start_terminal(24, 80);
        assert_eq!(request.password.as_deref(), Some("saved-secret"));
    }

    #[test]
    fn detects_old_agent_versions_and_requires_two_quotes() {
        assert!(version_is_older("0.11.0", "0.14.1"));
        assert!(!version_is_older("0.14.1", "0.14.1"));
        assert!(!version_is_older("0.15.0", "0.14.1"));

        let mut app = App::monitoring(vec![HostTarget::from_alias("edge-01")]);
        app.apply_agent_version("edge-01", "0.11.0".into());
        assert_eq!(app.selected_agent_version(), Some("0.11.0"));
        assert!(app.selected_agent_needs_update());

        app.apply_snapshot(HostSnapshot {
            host_id: "edge-01".into(),
            agent_version: "0.11.0".into(),
            collected_at: std::time::SystemTime::now(),
            latency_ms: 4,
            hostname: "edge-01".into(),
            os: "Linux".into(),
            kernel: "6.x".into(),
            uptime_secs: 10,
            cpu_percent: Some(1.0),
            memory: nekohub_core::Usage::default(),
            root_disk: nekohub_core::Usage::default(),
            load: [0.0; 3],
            network: nekohub_core::Throughput::default(),
            processes: Vec::new(),
            containers: Vec::new(),
        });

        assert!(app.selected_agent_needs_update());
        assert!(!app.register_agent_update_quote());
        assert!(app.agent_update_is_armed());
        assert!(app.register_agent_update_quote());
        assert_eq!(app.view, View::AgentUpdateAuth);
    }
}
