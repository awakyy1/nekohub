use std::collections::{HashMap, VecDeque};

use fleet_core::{HostSnapshot, HostTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Welcome,
    RemotePicker,
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
    pub remote_hosts: Vec<HostTarget>,
    pub remote_selected: usize,
    pub animation_tick: u64,
    pub welcome_notice: Option<String>,
    by_id: HashMap<String, usize>,
}

impl App {
    pub fn new(remote_hosts: Vec<HostTarget>) -> Self {
        Self {
            hosts: Vec::new(),
            selected: 0,
            view: View::Welcome,
            welcome_selected: 0,
            remote_hosts,
            remote_selected: 0,
            animation_tick: 0,
            welcome_notice: None,
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
            remote_hosts: Vec::new(),
            remote_selected: 0,
            animation_tick: 0,
            welcome_notice: None,
            by_id,
        }
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

    pub fn apply_snapshot(&mut self, snapshot: HostSnapshot) {
        let Some(index) = self.by_id.get(&snapshot.host_id).copied() else {
            return;
        };
        let host = &mut self.hosts[index];
        if let Some(cpu) = snapshot.cpu_percent {
            if host.cpu_history.len() == 120 {
                host.cpu_history.pop_front();
            }
            host.cpu_history.push_back(cpu.round() as u64);
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

    pub fn open_remote_picker(&mut self) {
        self.view = View::RemotePicker;
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

    pub fn selected_remote(&self) -> Option<HostTarget> {
        self.remote_hosts.get(self.remote_selected).cloned()
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
}
