#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Gauge, Paragraph, Row, Sparkline, Table, Widget,
        Wrap,
    },
};

use crate::{
    app::{App, HomeFocus, HostState, SettingsFocus, View},
    machines::INSTALLED_TAG,
    preferences::{CustomTheme, FontProfile, Theme},
    ssh_terminal::TerminalPhase,
};

const AMBER: Color = Color::Rgb(126, 213, 177);
const ORANGE: Color = Color::Rgb(216, 184, 122);
const DIM: Color = Color::Rgb(91, 108, 101);
const GREEN: Color = Color::Rgb(113, 221, 175);
const RED: Color = Color::Rgb(224, 112, 112);
const CYAN: Color = Color::Rgb(135, 164, 215);
const INK: Color = Color::Rgb(11, 16, 14);
const SURFACE: Color = Color::Rgb(16, 23, 20);
const LINE: Color = Color::Rgb(43, 56, 51);
const TEXT: Color = Color::Rgb(215, 224, 219);

pub fn mouse_key(app: &mut App, area: Rect, event: MouseEvent) -> Option<KeyEvent> {
    if event.kind == MouseEventKind::Moved && app.view == View::Overview {
        let hovered = if app.monitor_selected == 3 {
            storage_entry_at(area, app, event.column, event.row)
        } else {
            None
        };
        app.storage_hovered = hovered;
        return None;
    }
    if event.kind == MouseEventKind::Down(MouseButton::Right)
        && app.view == View::Overview
        && app.monitor_selected == 3
        && app.previous_storage_path()
    {
        return Some(mouse_key_event(KeyCode::Enter));
    }
    match event.kind {
        MouseEventKind::ScrollUp => return Some(mouse_key_event(KeyCode::Up)),
        MouseEventKind::ScrollDown => return Some(mouse_key_event(KeyCode::Down)),
        MouseEventKind::Down(MouseButton::Right) => return Some(mouse_key_event(KeyCode::Esc)),
        MouseEventKind::Down(MouseButton::Left) => {}
        _ => return None,
    }
    let x = event.column;
    let y = event.row;
    if let Some(index) = chrome_nav_hit(area, x, y) {
        return Some(mouse_key_event(KeyCode::Char(char::from(
            b'1' + index as u8,
        ))));
    }
    if app.view == View::Overview && app.monitor_selected == 3 {
        let entry = storage_entry_at(area, app, x, y);
        app.storage_hovered = entry;
        if entry.is_some_and(|index| app.open_storage_entry(index)) {
            return Some(mouse_key_event(KeyCode::Enter));
        }
    }
    match app.view {
        View::Welcome => welcome_mouse_key(app, area, x, y),
        View::AgentConfirm => Some(confirmation_mouse_key(app, area, x)),
        View::Home => home_mouse_key(app, area, x, y),
        View::GroupDetail => group_detail_mouse_key(app, area, y),
        View::RemotePicker => remote_picker_mouse_key(app, area, x, y),
        View::Settings => settings_mouse_key(app, area, x, y),
        View::Overview => monitoring_mouse_key(app, area, x, y),
        View::TerminalSavePassword => {
            app.terminal_save_selected = usize::from(x >= area.x + area.width / 2);
            Some(mouse_key_event(KeyCode::Enter))
        }
        View::GroupAssign => group_assignment_mouse_key(app, area, y),
        View::RemoteInstall => remote_install_mouse_key(app, area, y),
        View::AgentSetup | View::AgentUpdateProgress => {
            modal_footer_key(area, y, area.height.min(24))
        }
        View::CreateGroup | View::RemoteConnect | View::AgentUpdateAuth => {
            modal_footer_key(area, y, area.height.min(17))
        }
        View::MachineAlias => modal_footer_key(area, y, area.height.min(13)),
        View::RemoteUninstallConfirm => modal_footer_key(area, y, area.height.min(14)),
        View::RemoteInstallProgress => modal_footer_key(area, y, area.height.min(26)),
        View::ThemeImport => modal_footer_key(area, y, area.height.min(20)),
        View::CredentialKeyEdit => modal_footer_key(area, y, area.height.min(15)),
        View::TerminalPassword => modal_footer_key(area, y, 9_u16.min(area.height)),
        View::Detail | View::Terminal => None,
    }
}

fn mouse_key_event(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn chrome_content(area: Rect) -> Rect {
    Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(10),
        area.width.saturating_sub(2),
        area.height.saturating_sub(11),
    )
}

fn chrome_nav_hit(area: Rect, x: u16, y: u16) -> Option<usize> {
    if y < area.y.saturating_add(2) || y >= area.y.saturating_add(5) {
        return None;
    }
    let start = area.x.saturating_add(44);
    let width = area.right().saturating_sub(1).saturating_sub(start);
    if x < start || width < 3 {
        return None;
    }
    Some(usize::from((x - start) * 3 / width).min(2))
}

fn welcome_mouse_key(app: &mut App, area: Rect, x: u16, y: u16) -> Option<KeyEvent> {
    let inner = Rect::new(
        area.x + 1,
        area.y + 1,
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let content_height = if app.welcome_notice.is_some() { 14 } else { 12 };
    let top = inner.y + inner.height.saturating_sub(content_height) / 2;
    let button_x = inner.x + inner.width.saturating_sub(42) / 2;
    if x < button_x || x >= button_x.saturating_add(42.min(inner.width)) {
        return None;
    }
    app.welcome_selected = if (top + 3..top + 6).contains(&y) {
        0
    } else if (top + 7..top + 10).contains(&y) {
        1
    } else {
        return None;
    };
    Some(mouse_key_event(KeyCode::Enter))
}

fn confirmation_mouse_key(app: &mut App, area: Rect, x: u16) -> KeyEvent {
    app.agent_confirm_selected = usize::from(x >= area.x + area.width / 2);
    mouse_key_event(KeyCode::Enter)
}

fn home_mouse_key(app: &mut App, area: Rect, x: u16, y: u16) -> Option<KeyEvent> {
    let content = chrome_content(area);
    let shelf_y = content.y + 3;
    if (shelf_y..shelf_y + 6).contains(&y) {
        let machine_order = app.home_machine_order();
        let visible = usize::from(content.width / 24)
            .max(1)
            .min(machine_order.len());
        let first = app
            .home_machine_selected
            .saturating_sub(visible / 2)
            .min(machine_order.len().saturating_sub(visible));
        let card = usize::from(x.saturating_sub(content.x) / 23);
        if card < visible {
            app.home_focus = HomeFocus::Machines;
            app.home_machine_selected = first + card;
            return Some(mouse_key_event(KeyCode::Enter));
        }
    }
    let groups_y = content.y + 12;
    if y >= groups_y {
        const CARD_WIDTH: u16 = 30;
        const CARD_HEIGHT: u16 = 8;
        const GAP_X: u16 = 2;
        const GAP_Y: u16 = 1;
        let columns = usize::from(((content.width + GAP_X) / (CARD_WIDTH + GAP_X)).max(1));
        let visible_rows = usize::from(
            ((content.bottom().saturating_sub(groups_y) + GAP_Y) / (CARD_HEIGHT + GAP_Y)).max(1),
        );
        let capacity = columns * visible_rows;
        let first = app.home_selected / capacity * capacity;
        let column = usize::from(x.saturating_sub(content.x) / (CARD_WIDTH + GAP_X));
        let row = usize::from(y.saturating_sub(groups_y) / (CARD_HEIGHT + GAP_Y));
        let index = first + row * columns + column;
        if column < columns && index < app.home_item_count() {
            app.home_focus = HomeFocus::Groups;
            app.home_selected = index;
            return Some(mouse_key_event(KeyCode::Enter));
        }
    }
    None
}

fn group_detail_mouse_key(app: &mut App, area: Rect, y: u16) -> Option<KeyEvent> {
    let group = app.machine_groups.get(app.active_group)?;
    let modal_height = area.height.min(22);
    let modal_top = area.y + area.height.saturating_sub(modal_height) / 2;
    let capacity = usize::from(modal_height.saturating_sub(8)).max(1);
    let selected = app
        .group_machine_selected
        .min(group.host_ids.len().saturating_sub(1));
    let first = selected
        .saturating_sub(capacity / 2)
        .min(group.host_ids.len().saturating_sub(capacity));
    let index = first + usize::from(y.saturating_sub(modal_top.saturating_add(4)));
    if index < group.host_ids.len() {
        app.group_machine_selected = index;
        Some(mouse_key_event(KeyCode::Enter))
    } else {
        None
    }
}

fn group_assignment_mouse_key(app: &mut App, area: Rect, y: u16) -> Option<KeyEvent> {
    let height = (app.machine_groups.len() as u16 + 8)
        .min(area.height)
        .max(12);
    let top = area.y + area.height.saturating_sub(height) / 2;
    let index = usize::from(y.saturating_sub(top.saturating_add(3)));
    if index < app.machine_groups.len() {
        app.group_assign_selected = index;
        Some(mouse_key_event(KeyCode::Enter))
    } else {
        None
    }
}

fn remote_install_mouse_key(app: &mut App, area: Rect, y: u16) -> Option<KeyEvent> {
    let height = area.height.min(22);
    let top = area.y + area.height.saturating_sub(height) / 2;
    if (top + 6..top + 9).contains(&y) {
        app.remote_install_field = 0;
        None
    } else if (top + 9..top + 12).contains(&y) {
        app.remote_install_field = 1;
        None
    } else {
        modal_footer_key(area, y, height)
    }
}

fn modal_footer_key(area: Rect, y: u16, height: u16) -> Option<KeyEvent> {
    let top = area.y + area.height.saturating_sub(height) / 2;
    (y >= top.saturating_add(height.saturating_sub(2))).then(|| mouse_key_event(KeyCode::Enter))
}

fn remote_picker_mouse_key(app: &mut App, area: Rect, x: u16, y: u16) -> Option<KeyEvent> {
    let content = chrome_content(area);
    let panel_y = content.y + 6;
    let install_y = content.bottom().saturating_sub(4);
    if y < install_y.saturating_add(2) && y >= install_y {
        app.remote_selected = app.remote_item_count() - 1;
        return Some(mouse_key_event(KeyCode::Enter));
    }
    if y >= install_y.saturating_add(2) {
        let relative_x = x.saturating_sub(content.x);
        let action = if relative_x < content.width / 3 {
            'u'
        } else if relative_x < content.width / 3 * 2 {
            'g'
        } else {
            'a'
        };
        return Some(mouse_key_event(KeyCode::Char(action)));
    }
    let index = usize::from(y.saturating_sub(panel_y.saturating_add(3)));
    if index <= app.remote_hosts.len() {
        app.remote_selected = index;
        return Some(mouse_key_event(KeyCode::Enter));
    }
    None
}

fn settings_mouse_key(app: &mut App, area: Rect, x: u16, y: u16) -> Option<KeyEvent> {
    let content = chrome_content(area);
    if x < content.x.saturating_add(30) {
        let index = usize::from(y.saturating_sub(content.y) / 3);
        if index < 6 {
            app.settings_selected = index;
            app.settings_focus = SettingsFocus::Sidebar;
            return Some(mouse_key_event(KeyCode::Enter));
        }
        return None;
    }
    app.settings_focus = SettingsFocus::Content;
    if app.settings_selected == 2 && y >= content.bottom().saturating_sub(3) {
        return Some(mouse_key_event(KeyCode::Char(
            if x < content.x + content.width / 3 * 2 {
                'p'
            } else {
                'x'
            },
        )));
    }
    app.settings_item_selected = match app.settings_selected {
        0 => usize::from(y >= content.y + content.height / 2),
        1 if y >= content.y + 4 && y < content.y + 12 => usize::from(
            (x.saturating_sub(content.x + 32)) * 4 / content.width.saturating_sub(32).max(1),
        )
        .min(3),
        1 => 4,
        2 => usize::from(y.saturating_sub(content.y + 5)),
        _ => 0,
    };
    Some(mouse_key_event(KeyCode::Enter))
}

fn monitoring_mouse_key(app: &mut App, area: Rect, x: u16, y: u16) -> Option<KeyEvent> {
    let content = chrome_content(area);
    let body_y = content.y.saturating_add(3);
    if x < content.x.saturating_add(29) && y >= body_y.saturating_add(5) {
        let index = usize::from((y - body_y - 5) / 3);
        if index < 8 {
            app.select_monitor_section(index);
            return matches!(index, 3 | 7).then(|| mouse_key_event(KeyCode::Enter));
        }
    }
    None
}

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    render_content(frame, app);
    frame.render_widget(
        ThemeOverlay {
            palette: theme_palette(app.theme, app.custom_theme.as_ref()),
            font_profile: app.font_profile,
        },
        area,
    );
}

#[allow(clippy::too_many_lines)]
fn render_content(frame: &mut Frame<'_>, app: &App) {
    match app.view {
        View::Welcome => {
            render_welcome(frame, frame.area(), app);
            return;
        }
        View::AgentConfirm => {
            render_welcome(frame, frame.area(), app);
            render_agent_confirmation(frame, frame.area(), app);
            return;
        }
        View::AgentSetup => {
            render_agent_setup(frame, frame.area(), app);
            return;
        }
        View::Home => {
            render_home(frame, frame.area(), app);
            return;
        }
        View::CreateGroup => {
            render_home(frame, frame.area(), app);
            render_create_group(frame, frame.area(), app);
            return;
        }
        View::GroupDetail => {
            render_home(frame, frame.area(), app);
            render_group_detail(frame, frame.area(), app);
            return;
        }
        View::GroupAssign => {
            render_remote_picker(frame, frame.area(), app);
            render_group_assignment(frame, frame.area(), app);
            return;
        }
        View::RemotePicker => {
            render_remote_picker(frame, frame.area(), app);
            return;
        }
        View::MachineAlias => {
            render_remote_picker(frame, frame.area(), app);
            render_machine_alias(frame, frame.area(), app);
            return;
        }
        View::RemoteInstall => {
            render_remote_picker(frame, frame.area(), app);
            render_remote_install(frame, frame.area(), app);
            return;
        }
        View::RemoteConnect => {
            match app.remote_return_view() {
                View::Home => render_home(frame, frame.area(), app),
                View::GroupDetail => {
                    render_home(frame, frame.area(), app);
                    render_group_detail(frame, frame.area(), app);
                }
                _ => render_remote_picker(frame, frame.area(), app),
            }
            render_remote_connect(frame, frame.area(), app);
            return;
        }
        View::RemoteUninstallConfirm => {
            render_remote_picker(frame, frame.area(), app);
            render_remote_uninstall_confirmation(frame, frame.area(), app);
            return;
        }
        View::RemoteInstallProgress => {
            render_remote_picker(frame, frame.area(), app);
            render_remote_install_progress(frame, frame.area(), app);
            return;
        }
        View::Settings => {
            render_settings(frame, frame.area(), app);
            return;
        }
        View::ThemeImport => {
            render_settings(frame, frame.area(), app);
            render_theme_import(frame, frame.area(), app);
            return;
        }
        View::CredentialKeyEdit => {
            render_settings(frame, frame.area(), app);
            render_credential_key_edit(frame, frame.area(), app);
            return;
        }
        View::Overview
        | View::AgentUpdateAuth
        | View::AgentUpdateProgress
        | View::Detail
        | View::TerminalPassword
        | View::TerminalSavePassword
        | View::Terminal => {}
    }
    let footer = if app.view == View::Terminal {
        "F10 close terminal  ·  Ctrl+] alternative  ·  input goes directly over SSH"
    } else if app.view == View::TerminalPassword {
        "Type password  ·  Enter connect  ·  Esc cancel"
    } else if app.view == View::TerminalSavePassword {
        "←→ choose  ·  Enter confirm  ·  Esc back"
    } else {
        "↑↓ sections  ·  Enter open  ·  n/p machine  ·  r refresh  ·  Esc home  ·  q quit"
    };
    let content = render_app_chrome(frame, frame.area(), app, 1, footer);
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(10)])
        .split(content);
    render_monitoring_bar(frame, sections[0], app);
    match app.view {
        View::Overview
        | View::AgentUpdateAuth
        | View::AgentUpdateProgress
        | View::TerminalPassword
        | View::TerminalSavePassword
        | View::Terminal => {
            render_overview(frame, sections[1], app);
        }
        View::Detail => render_detail(frame, sections[1], app.selected()),
        View::Welcome
        | View::AgentConfirm
        | View::AgentSetup
        | View::Home
        | View::CreateGroup
        | View::GroupDetail
        | View::GroupAssign
        | View::RemotePicker
        | View::MachineAlias
        | View::RemoteInstall
        | View::RemoteConnect
        | View::RemoteUninstallConfirm
        | View::RemoteInstallProgress
        | View::Settings
        | View::ThemeImport
        | View::CredentialKeyEdit => {
            unreachable!("setup views return before shell render")
        }
    }
    if app.view == View::TerminalSavePassword {
        render_terminal_password_save(frame, frame.area(), app);
    } else if app.view == View::AgentUpdateAuth {
        render_agent_update_auth(frame, frame.area(), app);
    } else if app.view == View::AgentUpdateProgress {
        render_agent_update_progress(frame, frame.area(), app);
    }
}

struct ThemeOverlay {
    palette: ThemePalette,
    font_profile: FontProfile,
}

impl Widget for ThemeOverlay {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        for y in area.y..area.bottom() {
            for x in area.x..area.right() {
                let Some(cell) = buffer.cell_mut((x, y)) else {
                    continue;
                };
                let style = cell.style();
                let recolor = |color| match color {
                    AMBER => self.palette.primary,
                    ORANGE => self.palette.secondary,
                    CYAN => self.palette.tertiary,
                    GREEN => self.palette.success,
                    INK => self.palette.background,
                    SURFACE => self.palette.surface,
                    LINE => self.palette.border,
                    TEXT => self.palette.text,
                    DIM => self.palette.muted,
                    other => other,
                };
                if let Some(foreground) = style.fg {
                    cell.set_fg(recolor(foreground));
                }
                if let Some(background) = style.bg {
                    cell.set_bg(recolor(background));
                }
                let replacement = match (self.font_profile, cell.symbol()) {
                    (FontProfile::Compact, "◆") => Some("▸"),
                    (FontProfile::Compact, "◇") => Some("▹"),
                    (FontProfile::Compact, "●") => Some("▪"),
                    (FontProfile::Compact, "○") => Some("▫"),
                    (FontProfile::Compact, "›") => Some("→"),
                    (FontProfile::Ascii, "◆") => Some("#"),
                    (FontProfile::Ascii, "◇") => Some("+"),
                    (FontProfile::Ascii, "●" | "•") => Some("*"),
                    (FontProfile::Ascii, "○") => Some("o"),
                    (FontProfile::Ascii, "›" | "→") => Some(">"),
                    (FontProfile::Ascii, "━") => Some("-"),
                    (FontProfile::Ascii, "·") => Some("."),
                    _ => None,
                };
                if let Some(symbol) = replacement {
                    cell.set_symbol(symbol);
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ThemePalette {
    primary: Color,
    secondary: Color,
    tertiary: Color,
    success: Color,
    background: Color,
    surface: Color,
    border: Color,
    text: Color,
    muted: Color,
}

fn theme_palette(theme: Theme, custom: Option<&CustomTheme>) -> ThemePalette {
    match theme {
        Theme::Pink => ThemePalette {
            primary: Color::Rgb(239, 139, 181),
            secondary: Color::Rgb(244, 174, 151),
            tertiary: Color::Rgb(190, 159, 235),
            success: Color::Rgb(124, 218, 181),
            background: Color::Rgb(20, 14, 18),
            surface: Color::Rgb(34, 23, 31),
            border: Color::Rgb(80, 51, 69),
            text: Color::Rgb(241, 224, 233),
            muted: Color::Rgb(145, 111, 130),
        },
        Theme::Blue => ThemePalette {
            primary: Color::Rgb(112, 166, 255),
            secondary: Color::Rgb(104, 205, 225),
            tertiary: Color::Rgb(145, 132, 238),
            success: Color::Rgb(104, 218, 185),
            background: Color::Rgb(9, 15, 26),
            surface: Color::Rgb(16, 28, 46),
            border: Color::Rgb(42, 67, 100),
            text: Color::Rgb(218, 230, 247),
            muted: Color::Rgb(100, 122, 151),
        },
        Theme::Red => ThemePalette {
            primary: Color::Rgb(235, 101, 101),
            secondary: Color::Rgb(241, 157, 91),
            tertiary: Color::Rgb(218, 114, 137),
            success: Color::Rgb(126, 207, 145),
            background: Color::Rgb(22, 11, 11),
            surface: Color::Rgb(39, 20, 19),
            border: Color::Rgb(91, 45, 43),
            text: Color::Rgb(244, 225, 218),
            muted: Color::Rgb(150, 105, 96),
        },
        Theme::Purple => ThemePalette {
            primary: Color::Rgb(180, 132, 255),
            secondary: Color::Rgb(232, 132, 211),
            tertiary: Color::Rgb(113, 165, 246),
            success: Color::Rgb(111, 218, 193),
            background: Color::Rgb(15, 10, 25),
            surface: Color::Rgb(28, 19, 45),
            border: Color::Rgb(66, 47, 96),
            text: Color::Rgb(233, 224, 247),
            muted: Color::Rgb(126, 107, 153),
        },
        Theme::Custom => custom.map_or_else(
            || theme_palette(Theme::Pink, None),
            |theme| ThemePalette {
                primary: hex_color(&theme.primary),
                secondary: hex_color(&theme.secondary),
                tertiary: hex_color(&theme.tertiary),
                success: hex_color(&theme.success),
                background: hex_color(&theme.background),
                surface: hex_color(&theme.surface),
                border: hex_color(&theme.border),
                text: hex_color(&theme.text),
                muted: hex_color(&theme.muted),
            },
        ),
    }
}

fn theme_color(theme: Theme) -> Color {
    theme_palette(theme, None).primary
}

fn hex_color(value: &str) -> Color {
    let value = value.trim_start_matches('#');
    let component = |range| {
        value
            .get(range)
            .and_then(|part| u8::from_str_radix(part, 16).ok())
            .unwrap_or(255)
    };
    Color::Rgb(component(0..2), component(2..4), component(4..6))
}

fn render_agent_confirmation(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(70);
    let height = area.height.min(15);
    let modal = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .title(Line::styled(
            " INSTALL LOCAL AGENT ",
            Style::default().fg(AMBER).bold(),
        ))
        .title_bottom(Line::from(" ←→ select  ·  Enter confirm  ·  Esc cancel ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(AMBER));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("ONE-TIME SETUP", Style::default().fg(DIM).bold()),
            Line::from(""),
            Line::styled(
                "This action will install the nekoHub agent on your computer.",
                Style::default().fg(Color::Gray),
            ),
            Line::styled("Do you want to proceed?", Style::default().fg(AMBER).bold()),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true }),
        rows[0],
    );
    let buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(rows[1]);
    render_welcome_button(frame, buttons[1], "Yes", app.agent_confirm_selected == 0);
    render_welcome_button(frame, buttons[2], "No", app.agent_confirm_selected == 1);
    frame.render_widget(
        Paragraph::new("The agent runs read-only and without root privileges.")
            .style(Style::default().fg(DIM))
            .alignment(Alignment::Center),
        rows[2],
    );
}

fn render_home(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let content = render_app_chrome(
        frame,
        area,
        app,
        0,
        "Tab header/cards  ·  ←→ browse  ·  Enter open  ·  m machines  ·  s settings  ·  q quit",
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(content);
    let ready = app
        .remote_hosts
        .iter()
        .filter(|machine| machine.tags.iter().any(|tag| tag == INSTALLED_TAG))
        .count()
        + 1;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("RECENT MACHINES", Style::default().fg(CYAN).bold()),
            Span::styled(
                format!("    {} registered", app.remote_hosts.len() + 1),
                Style::default().fg(DIM),
            ),
            Span::styled(format!("    ● {ready} ready"), Style::default().fg(GREEN)),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(LINE)),
        ),
        animated_area(rows[0], app, 0, 3),
    );
    render_home_machine_shelf(frame, rows[1], app);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("GROUPS", Style::default().fg(AMBER).bold()),
            Span::styled(
                format!("    {} spaces", app.machine_groups.len()),
                Style::default().fg(DIM),
            ),
            Span::styled("    organize your fleet", Style::default().fg(ORANGE)),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(LINE)),
        ),
        rows[2],
    );
    render_group_grid(frame, rows[3], app);
    if let Some(notice) = app.home_notice.as_deref() {
        frame.render_widget(
            Paragraph::new(notice)
                .style(Style::default().fg(ORANGE))
                .alignment(Alignment::Center),
            rows[4],
        );
    }
}

fn render_home_machine_shelf(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let machine_order = app.home_machine_order();
    let visible = usize::from(area.width / 24).max(1).min(machine_order.len());
    let first = app
        .home_machine_selected
        .saturating_sub(visible / 2)
        .min(machine_order.len().saturating_sub(visible));
    let last = first + visible;
    let cards = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Length(23); visible])
        .split(area);
    for (card_index, index) in (first..last).enumerate() {
        let machine_index = machine_order[index];
        let (name, local, installed) = if machine_index == 0 {
            (&app.local_name, true, true)
        } else {
            let host = &app.remote_hosts[machine_index - 1];
            (
                &host.display_name,
                false,
                host.tags.iter().any(|tag| tag == INSTALLED_TAG),
            )
        };
        let selected = app.home_focus == HomeFocus::Machines && app.home_machine_selected == index;
        let title = match (
            card_index == 0 && first > 0,
            card_index + 1 == visible && last < machine_order.len(),
        ) {
            (true, true) => " ‹ MACHINES › ",
            (true, false) => " ‹ MACHINES ",
            (false, true) => " MACHINES › ",
            (false, false) if index == 0 => " MACHINES ",
            _ => " ",
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(clipped(name, 19), Style::default().fg(TEXT).bold()),
                Line::styled(
                    if local {
                        "local machine"
                    } else if installed {
                        "agent installed"
                    } else {
                        "SSH inventory"
                    },
                    Style::default().fg(if installed { GREEN } else { DIM }),
                ),
            ])
            .style(card_background(app, selected))
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(if selected {
                        AMBER
                    } else if installed {
                        GREEN
                    } else {
                        LINE
                    })),
            ),
            cards[card_index],
        );
    }
}

fn render_app_chrome(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    active_nav: usize,
    footer: &str,
) -> Rect {
    let outer = Block::default()
        .title_bottom(
            Line::from(format!(" {footer} "))
                .style(Style::default().fg(DIM))
                .centered(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(LINE))
        .style(canvas_style(app));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(8),
        ])
        .split(inner);
    render_home_header(frame, rows[0], app, active_nav);
    render_machine_strip(frame, rows[1], app);
    rows[3]
}

fn render_home_header(frame: &mut Frame<'_>, area: Rect, app: &App, active_nav: usize) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(43), Constraint::Min(45)])
        .split(area);
    let logo = if columns[0].width >= 43 {
        vec![
            Line::from(vec![
                Span::styled(" /\\", Style::default().fg(AMBER).bold()),
                Span::styled(
                    "        __        __ __     __",
                    Style::default().fg(TEXT).bold(),
                ),
                Span::styled(" /\\", Style::default().fg(AMBER).bold()),
            ]),
            Line::styled(
                "  ___  ___ / /_____  / // /_ __/ /",
                Style::default().fg(TEXT).bold(),
            ),
            Line::styled(
                " / _ \\/ -_)  '_/ _ \\/ _  / // / _ \\",
                Style::default().fg(Color::Gray),
            ),
            Line::styled(
                "/_//_/\\__/_/\\_\\\\___/_//_/\\_,_/_.__/",
                Style::default().fg(AMBER),
            ),
        ]
    } else {
        vec![Line::from(vec![
            Span::styled("/\\ ", Style::default().fg(AMBER).bold()),
            Span::styled("nekoHub", Style::default().fg(TEXT).bold()),
            Span::styled(" /\\", Style::default().fg(AMBER).bold()),
        ])]
    };
    frame.render_widget(Paragraph::new(logo), columns[0]);
    let nav_area = Rect::new(
        columns[1].x,
        columns[1].y.saturating_add(1),
        columns[1].width,
        columns[1].height.saturating_sub(1).min(3),
    );
    let buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(nav_area);
    for (index, (label, shortcut)) in [("Home", "1"), ("Machines", "2"), ("Settings", "3")]
        .into_iter()
        .enumerate()
    {
        let focused = app.view == View::Home
            && app.home_focus == HomeFocus::Navigation
            && app.home_nav_selected == index;
        let active = index == active_nav;
        let color = if focused || active { AMBER } else { DIM };
        let style = if focused {
            Style::default().fg(INK).bg(AMBER).bold()
        } else {
            Style::default().fg(color).bold()
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!("[{shortcut}] "), Style::default().fg(DIM)),
                Span::styled(label, style),
            ]))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::BOTTOM)
                    .border_style(Style::default().fg(LINE)),
            ),
            buttons[index],
        );
    }
    let (indicator_from, indicator_to, frame_index, total_frames) = app
        .navigation_motion
        .map_or((active_nav, active_nav, 1, 1), |motion| {
            (motion.from, motion.to, motion.frame, motion.total_frames)
        });
    let from = buttons[indicator_from.min(2)];
    let to = buttons[indicator_to.min(2)];
    let progress = i32::from(frame_index.min(total_frames));
    let total = i32::from(total_frames.max(1));
    let from_x = i32::from(from.x.saturating_add(2));
    let to_x = i32::from(to.x.saturating_add(2));
    let indicator_x = from_x + (to_x - from_x) * progress / total;
    let indicator_width = to.width.saturating_sub(4).max(1);
    frame.render_widget(
        Paragraph::new("━".repeat(usize::from(indicator_width))).style(Style::default().fg(AMBER)),
        Rect::new(
            u16::try_from(indicator_x.max(0)).unwrap_or_default(),
            nav_area.bottom().saturating_sub(1),
            indicator_width,
            1,
        ),
    );
}

fn render_machine_strip(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut spans = vec![
        Span::styled(" ● ", Style::default().fg(GREEN).bold()),
        Span::styled(app.local_name.as_str(), Style::default().fg(TEXT).bold()),
        Span::styled("  LOCAL AGENT", Style::default().fg(DIM)),
    ];
    for host in &app.remote_hosts {
        spans.push(Span::styled("     ○ ", Style::default().fg(DIM)));
        spans.push(Span::styled(
            host.display_name.clone(),
            Style::default().fg(Color::Gray),
        ));
    }
    if app.remote_hosts.is_empty() {
        spans.push(Span::styled("     0 REMOTE", Style::default().fg(DIM)));
    }
    spans.push(Span::styled("     0 ALERTS", Style::default().fg(GREEN)));
    frame.render_widget(
        Paragraph::new(Line::from(spans)).block(
            Block::default()
                .title(Line::styled(
                    format!(" FLEET PULSE  {} ", app.remote_hosts.len() + 1),
                    Style::default().fg(CYAN).bold(),
                ))
                .borders(Borders::TOP | Borders::BOTTOM)
                .border_style(Style::default().fg(LINE)),
        ),
        area,
    );
}

fn render_group_grid(frame: &mut Frame<'_>, area: Rect, app: &App) {
    const CARD_WIDTH: u16 = 30;
    const CARD_HEIGHT: u16 = 8;
    const COLUMN_GAP: u16 = 2;
    const ROW_GAP: u16 = 1;
    let columns = usize::from(((area.width + COLUMN_GAP) / (CARD_WIDTH + COLUMN_GAP)).max(1));
    let visible_rows = usize::from(((area.height + ROW_GAP) / (CARD_HEIGHT + ROW_GAP)).max(1));
    let capacity = columns * visible_rows;
    let selected_page = app.home_selected / capacity;
    let first = selected_page * capacity;
    let last = (first + capacity).min(app.home_item_count());
    for row in 0..visible_rows {
        for column in 0..columns {
            let index = first + row * columns + column;
            if index >= last {
                break;
            }
            let x = area.x + u16::try_from(column).unwrap_or_default() * (CARD_WIDTH + COLUMN_GAP);
            let y = area.y + u16::try_from(row).unwrap_or_default() * (CARD_HEIGHT + ROW_GAP);
            let card_area = Rect::new(
                x,
                y,
                CARD_WIDTH.min(area.right().saturating_sub(x)),
                CARD_HEIGHT.min(area.bottom().saturating_sub(y)),
            );
            let stage = u8::try_from(index.saturating_sub(first) + 1).unwrap_or(u8::MAX);
            render_group_card(frame, animated_area(card_area, app, stage, 4), app, index);
        }
    }
}

fn render_group_card(frame: &mut Frame<'_>, area: Rect, app: &App, index: usize) {
    let selected = app.home_focus == HomeFocus::Groups && app.home_selected == index;
    let is_add = index + 1 == app.home_item_count();
    if is_add {
        render_add_group_button(frame, area, app, selected);
        return;
    }
    let (title, count, state, accent) = {
        let group = &app.machine_groups[index];
        let count = group.host_ids.len();
        (
            group.name.clone(),
            format!("{count} machine{}", if count == 1 { "" } else { "s" }),
            if count == 0 {
                "○ empty group".to_owned()
            } else {
                "● group ready".to_owned()
            },
            [AMBER, CYAN, ORANGE][index % 3],
        )
    };
    let border = if selected { accent } else { LINE };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border))
        .style(card_background(app, selected));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let available = usize::from(inner.width.saturating_sub(1));
    let title = clipped(&title, available.saturating_sub(3));
    let heading = format!(" ◆ {title:<width$}", width = available.saturating_sub(3));
    let pulse = ["·", "•", "●", "•"][(app.animation_tick as usize / 3) % 4];
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                heading,
                if selected {
                    Style::default().fg(INK).bg(accent).bold()
                } else {
                    Style::default().fg(TEXT).bold()
                },
            ),
            Line::styled(format!("   {count}"), Style::default().fg(DIM)),
            Line::from(""),
            Line::from(vec![
                Span::styled(format!("  {pulse} "), Style::default().fg(accent)),
                Span::styled(
                    state.trim_start_matches(['●', '○', ' ']),
                    Style::default().fg(accent),
                ),
                Span::styled(
                    if selected { "   Enter ›" } else { "" },
                    Style::default().fg(TEXT).bold(),
                ),
            ]),
        ])
        .wrap(Wrap { trim: true }),
        inner,
    );
}

fn render_add_group_button(frame: &mut Frame<'_>, area: Rect, app: &App, selected: bool) {
    let color = if selected { ORANGE } else { DIM };
    frame.render_widget(Block::default().style(card_background(app, selected)), area);
    if area.width < 2 || area.height < 2 {
        return;
    }
    let horizontal = (0..area.width.saturating_sub(2))
        .map(|index| if index % 2 == 0 { '-' } else { ' ' })
        .collect::<String>();
    frame.render_widget(
        Paragraph::new(format!("+{horizontal}+")).style(Style::default().fg(color)),
        Rect::new(area.x, area.y, area.width, 1),
    );
    frame.render_widget(
        Paragraph::new(format!("+{horizontal}+")).style(Style::default().fg(color)),
        Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1),
    );
    for (offset, y) in (area.y.saturating_add(1)..area.bottom().saturating_sub(1)).enumerate() {
        if offset % 2 != 0 {
            continue;
        }
        frame.render_widget(
            Paragraph::new("|").style(Style::default().fg(color)),
            Rect::new(area.x, y, 1, 1),
        );
        frame.render_widget(
            Paragraph::new("|").style(Style::default().fg(color)),
            Rect::new(area.right().saturating_sub(1), y, 1, 1),
        );
    }
    let inner = Rect::new(
        area.x.saturating_add(1),
        area.y.saturating_add(1),
        area.width.saturating_sub(2),
        area.height.saturating_sub(2),
    );
    let content_height = inner.height.min(3);
    let content = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(content_height) / 2,
        inner.width,
        content_height,
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("+  ", Style::default().fg(ORANGE).bold()),
                Span::styled("New group", Style::default().fg(TEXT).bold()),
            ]),
            Line::styled("Create a machine group", Style::default().fg(DIM)),
            Line::styled("Enter to add", Style::default().fg(color)),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true }),
        content,
    );
}

fn canvas_style(app: &App) -> Style {
    if app.background_enabled {
        Style::default().bg(INK)
    } else {
        Style::default()
    }
}

fn card_background(app: &App, selected: bool) -> Style {
    if app.background_enabled {
        Style::default().bg(if selected { SURFACE } else { INK })
    } else {
        Style::default()
    }
}

fn animated_area(area: Rect, app: &App, stage: u8, max_offset: u16) -> Rect {
    let Some(motion) = app.navigation_motion else {
        return area;
    };
    let total = motion.total_frames.saturating_sub(stage).max(1);
    let frame = motion.frame.saturating_sub(stage).min(total);
    let remaining = u16::from(total.saturating_sub(frame));
    let offset = max_offset
        .saturating_mul(remaining)
        .checked_div(u16::from(total))
        .unwrap_or_default()
        .min(area.width.saturating_sub(1));
    Rect::new(
        area.x.saturating_add(offset),
        area.y,
        area.width.saturating_sub(offset),
        area.height,
    )
}

fn render_create_group(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(62);
    let height = area.height.min(17);
    let modal = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" (^._.^) ", Style::default().fg(ORANGE).bold()),
            Span::styled(" NEW MACHINE GROUP ", Style::default().fg(TEXT).bold()),
        ]))
        .title_bottom(Line::from(" Enter create  ·  Esc cancel ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(ORANGE));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(2),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                "Create a friendly home for machines that belong together.",
                Style::default().fg(TEXT),
            ),
            Line::from(""),
            Line::styled(
                "You can add and reorganize machines later.",
                Style::default().fg(DIM),
            ),
        ])
        .alignment(Alignment::Center),
        rows[0],
    );
    let cursor = if app.animation_tick % 10 < 5 {
        "_"
    } else {
        " "
    };
    frame.render_widget(
        Paragraph::new(format!(" {}{cursor}", app.group_draft)).block(
            Block::default()
                .title(" Group name ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(AMBER)),
        ),
        rows[1],
    );
    if let Some(error) = app.group_error.as_deref() {
        frame.render_widget(
            Paragraph::new(error)
                .style(Style::default().fg(RED))
                .alignment(Alignment::Center),
            rows[2],
        );
    }
}

fn render_group_detail(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some(group) = app.machine_groups.get(app.active_group) else {
        return;
    };
    let width = area.width.min(76);
    let height = area.height.min(22);
    let modal = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal);
    let selected = app
        .group_machine_selected
        .min(group.host_ids.len().saturating_sub(1));
    let capacity = usize::from(modal.height.saturating_sub(8)).max(1);
    let first = selected
        .saturating_sub(capacity / 2)
        .min(group.host_ids.len().saturating_sub(capacity));
    let last = (first + capacity).min(group.host_ids.len());
    let mut lines = vec![
        Line::styled(group.name.as_str(), Style::default().fg(AMBER).bold()),
        Line::styled(
            format!(
                "{} machines{}",
                group.host_ids.len(),
                if group.host_ids.is_empty() {
                    String::new()
                } else {
                    format!("    selected {}/{}", selected + 1, group.host_ids.len())
                }
            ),
            Style::default().fg(DIM),
        ),
        Line::from(""),
    ];
    if group.host_ids.is_empty() {
        lines.push(Line::styled(
            "This group is empty. Select a machine in Machines and press g.",
            Style::default().fg(Color::Gray),
        ));
    } else {
        if first > 0 {
            lines.push(Line::styled(
                "              ↑ more",
                Style::default().fg(DIM),
            ));
        }
        for (index, id) in group.host_ids[first..last].iter().enumerate() {
            let absolute_index = first + index;
            lines.push(group_machine_line(app, id, absolute_index == selected));
        }
        if last < group.host_ids.len() {
            lines.push(Line::styled(
                "              ↓ more",
                Style::default().fg(DIM),
            ));
        }
    }
    let footer = if group.host_ids.is_empty() {
        " Esc close  ·  manage membership in Machines "
    } else {
        " ↑↓ select  ·  Enter open metrics  ·  Esc close "
    };
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" GROUP ")
                    .title_bottom(Line::from(footer).centered())
                    .borders(Borders::ALL)
                    .border_type(BorderType::Thick)
                    .border_style(Style::default().fg(AMBER))
                    .style(canvas_style(app)),
            )
            .wrap(Wrap { trim: true }),
        modal,
    );
}

fn group_machine_line(app: &App, id: &str, selected: bool) -> Line<'static> {
    let (name, status, status_color) = if id == "local" {
        (app.local_name.clone(), "local agent", GREEN)
    } else {
        app.remote_hosts
            .iter()
            .find(|machine| machine.id == id)
            .map_or((id.to_owned(), "unavailable", RED), |machine| {
                let installed = machine.tags.iter().any(|tag| tag == INSTALLED_TAG);
                (
                    machine.display_name.clone(),
                    if installed {
                        "agent installed"
                    } else {
                        "SSH inventory"
                    },
                    if installed { GREEN } else { DIM },
                )
            })
    };
    Line::from(vec![
        Span::styled(
            if selected { " › " } else { "   " },
            Style::default().fg(AMBER).bold(),
        ),
        Span::styled(
            format!("{:<28}", clipped(&name, 28)),
            if selected {
                Style::default().fg(INK).bg(AMBER).bold()
            } else {
                Style::default().fg(TEXT).bold()
            },
        ),
        Span::styled(format!("  {status}"), Style::default().fg(status_color)),
    ])
}

fn render_group_assignment(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(66);
    let height = (app.machine_groups.len() as u16 + 8)
        .min(area.height)
        .max(12);
    let modal = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal);
    let machine_id = if app.remote_selected == 0 {
        "local"
    } else {
        app.remote_hosts[app.remote_selected - 1].id.as_str()
    };
    let rows = app.machine_groups.iter().enumerate().map(|(index, group)| {
        let member = group.host_ids.iter().any(|id| id == machine_id);
        Line::from(vec![
            Span::styled(
                if index == app.group_assign_selected {
                    " › "
                } else {
                    "   "
                },
                Style::default().fg(AMBER),
            ),
            Span::styled(
                if member { "[x] " } else { "[ ] " },
                Style::default().fg(if member { GREEN } else { DIM }),
            ),
            Span::styled(group.name.as_str(), Style::default().fg(TEXT).bold()),
        ])
    });
    frame.render_widget(
        Paragraph::new(
            std::iter::once(Line::styled(
                "Choose a group for this machine",
                Style::default().fg(TEXT).bold(),
            ))
            .chain(std::iter::once(Line::from("")))
            .chain(rows)
            .collect::<Vec<_>>(),
        )
        .block(
            Block::default()
                .title(" GROUP MEMBERSHIP ")
                .title_bottom(Line::from(" ↑↓ choose  ·  Enter toggle  ·  Esc cancel ").centered())
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(Style::default().fg(AMBER))
                .style(canvas_style(app)),
        ),
        modal,
    );
}

fn render_settings(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let inner = render_app_chrome(
        frame,
        area,
        app,
        2,
        "Tab switch pane  ·  ↑↓ navigate  ·  Enter select  ·  ← back  ·  Esc home",
    );
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(30),
            Constraint::Length(2),
            Constraint::Min(30),
        ])
        .split(inner);
    let sections = [
        ("Appearance", "Background and motion preferences", ORANGE),
        ("Themes", "Palette and community themes", CYAN),
        (
            "Passwords & keys",
            "SSH credentials managed per machine",
            AMBER,
        ),
        ("Agents", "Local service and remote pairing", GREEN),
        (
            "Machine groups",
            "Create, rename and organize machine groups",
            AMBER,
        ),
        (
            "Data & integrations",
            "Prometheus, history and retention",
            Color::Rgb(164, 143, 214),
        ),
    ];
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(3); sections.len()])
        .split(columns[0]);
    for (index, (title, _, color)) in sections.iter().enumerate() {
        let selected = app.settings_selected == index;
        let focused = selected && app.settings_focus == SettingsFocus::Sidebar;
        frame.render_widget(
            Paragraph::new(format!("{} {title}", if selected { "›" } else { " " }))
                .style(if focused {
                    Style::default().fg(INK).bg(*color).bold()
                } else {
                    Style::default().fg(Color::Gray)
                })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(if focused { *color } else { DIM })),
                ),
            sidebar[index],
        );
    }
    let (title, detail, color) = sections[app.settings_selected];
    if app.settings_selected == 0 {
        render_appearance_settings(frame, columns[2], app, color);
        return;
    }
    if app.settings_selected == 1 {
        render_theme_settings(frame, columns[2], app, color);
        return;
    }
    if app.settings_selected == 2 {
        render_credentials_settings(frame, columns[2], app, color);
        return;
    }
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(title, Style::default().fg(color).bold()),
            Line::from(""),
            Line::styled(detail, Style::default().fg(Color::Gray)),
            Line::from(""),
            Line::styled(
                "Configuration options are coming next.",
                Style::default().fg(DIM),
            ),
        ])
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(" SETTINGS ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(
                    if app.settings_focus == SettingsFocus::Content
                        && app.settings_item_selected == 0
                    {
                        AMBER
                    } else {
                        color
                    },
                )),
        ),
        columns[2],
    );
}

#[allow(clippy::too_many_lines)]
fn render_appearance_settings(frame: &mut Frame<'_>, area: Rect, app: &App, color: Color) {
    let block = Block::default()
        .title(" APPEARANCE ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Length(7),
            Constraint::Min(2),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("Appearance", Style::default().fg(color).bold()),
            Line::styled(
                "Choose whether nekoHub paints its own canvas.",
                Style::default().fg(Color::Gray),
            ),
        ]),
        rows[0],
    );
    let checkbox = if app.background_enabled { "[x]" } else { "[ ]" };
    let state = if app.background_enabled {
        "Solid nekoHub background"
    } else {
        "Terminal background · transparency and blur visible"
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(format!(" {checkbox} "), Style::default().fg(color).bold()),
                Span::styled(
                    "Draw application background",
                    Style::default().fg(TEXT).bold(),
                ),
            ]),
            Line::from(""),
            Line::styled(format!("     {state}"), Style::default().fg(Color::Gray)),
            Line::styled("     Enter or Space to toggle", Style::default().fg(DIM)),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(
                    if app.settings_focus == SettingsFocus::Content
                        && app.settings_item_selected == 0
                    {
                        AMBER
                    } else {
                        color
                    },
                )),
        ),
        rows[1],
    );
    let font_options = FontProfile::ALL.map(|profile| {
        let selected = app.font_profile == profile;
        Span::styled(
            format!(" {} ", profile.label()),
            if selected {
                Style::default().fg(INK).bg(AMBER).bold()
            } else {
                Style::default().fg(Color::Gray)
            },
        )
    });
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![Span::styled(
                " Interface lettering ",
                Style::default().fg(TEXT).bold(),
            )]),
            Line::from(""),
            Line::from(font_options.to_vec()),
            Line::from(""),
            Line::styled(
                " Enter cycles symbols and visual density · terminal controls the real font",
                Style::default().fg(DIM),
            ),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(
                    if app.settings_focus == SettingsFocus::Content
                        && app.settings_item_selected == 1
                    {
                        AMBER
                    } else {
                        color
                    },
                )),
        ),
        rows[2],
    );
    if let Some(notice) = app.settings_notice.as_deref() {
        frame.render_widget(
            Paragraph::new(notice).style(Style::default().fg(color)),
            rows[3],
        );
    }
}

#[allow(clippy::too_many_lines)]
fn render_theme_settings(frame: &mut Frame<'_>, area: Rect, app: &App, color: Color) {
    let block = Block::default()
        .title(" THEMES ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(8),
            Constraint::Length(4),
            Constraint::Min(2),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("Theme color", Style::default().fg(color).bold()),
            Line::styled(
                "One focused color, carried through the whole interface.",
                Style::default().fg(Color::Gray),
            ),
        ]),
        rows[0],
    );
    let cards = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 4); 4])
        .split(rows[1]);
    for (index, theme) in Theme::ALL.into_iter().enumerate() {
        let active = app.theme == theme;
        let selected =
            app.settings_focus == SettingsFocus::Content && app.settings_item_selected == index;
        let swatch = theme_color(theme);
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("██████", Style::default().fg(swatch)),
                Line::from(""),
                Line::styled(
                    theme.label(),
                    if active {
                        Style::default().fg(INK).bg(swatch).bold()
                    } else {
                        Style::default().fg(TEXT)
                    },
                ),
                Line::styled(
                    if active { "● active" } else { "○ select" },
                    Style::default().fg(swatch),
                ),
            ])
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(if selected {
                        AMBER
                    } else if active {
                        swatch
                    } else {
                        LINE
                    })),
            ),
            cards[index],
        );
    }
    let import_selected =
        app.settings_focus == SettingsFocus::Content && app.settings_item_selected == 4;
    let custom_label = app
        .custom_theme
        .as_ref()
        .map_or("Load a community palette from a JSON file", |theme| {
            theme.name.as_str()
        });
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " + Import / upload theme  ",
                Style::default().fg(TEXT).bold(),
            ),
            Span::styled(custom_label, Style::default().fg(DIM)),
            Span::styled(
                if app.theme == Theme::Custom {
                    "  ● active"
                } else {
                    ""
                },
                Style::default().fg(AMBER),
            ),
        ]))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if import_selected { AMBER } else { LINE })),
        ),
        rows[2],
    );
    let message = app
        .settings_notice
        .as_deref()
        .unwrap_or("Use ↑↓ and Enter. Changes are saved immediately.");
    frame.render_widget(
        Paragraph::new(message).style(Style::default().fg(color)),
        rows[3],
    );
}

fn render_credentials_settings(frame: &mut Frame<'_>, area: Rect, app: &App, color: Color) {
    let block = Block::default()
        .title(" PASSWORDS & KEYS ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(color));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("Credentials by machine", Style::default().fg(color).bold()),
            Line::styled(
                "Passwords are hidden and stored with owner-only file permissions.",
                Style::default().fg(Color::Gray),
            ),
            Line::styled(
                "Saved keys are passed to OpenSSH only for the selected machine.",
                Style::default().fg(DIM),
            ),
        ]),
        rows[0],
    );
    let machines = app.credential_machines();
    let lines = if machines.is_empty() {
        vec![Line::styled(
            "No SSH machines found. Add one in ~/.ssh/config first.",
            Style::default().fg(DIM),
        )]
    } else {
        machines
            .iter()
            .enumerate()
            .map(|(index, (alias, display_name))| {
                let credential = app.credentials.get(alias);
                let password = if credential.and_then(|item| item.password.as_ref()).is_some() {
                    "password saved"
                } else {
                    "no password"
                };
                let key = credential
                    .and_then(|item| item.identity_file.as_ref())
                    .map_or_else(
                        || "SSH config/default key".into(),
                        |path| path.display().to_string(),
                    );
                let selected = app.settings_focus == SettingsFocus::Content
                    && app.settings_item_selected == index;
                Line::from(vec![
                    Span::styled(
                        if selected { " › " } else { "   " },
                        Style::default().fg(color).bold(),
                    ),
                    Span::styled(display_name, Style::default().fg(TEXT).bold()),
                    Span::styled(format!("  {alias}  "), Style::default().fg(DIM)),
                    Span::styled(
                        password,
                        Style::default().fg(
                            if credential.and_then(|item| item.password.as_ref()).is_some() {
                                GREEN
                            } else {
                                Color::Gray
                            },
                        ),
                    ),
                    Span::styled(format!("  key: {key}"), Style::default().fg(CYAN)),
                ])
            })
            .collect()
    };
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), rows[1]);
    let message = app
        .settings_notice
        .as_deref()
        .unwrap_or("Enter edit key path  ·  p remove password  ·  x remove managed key");
    frame.render_widget(
        Paragraph::new(message)
            .style(Style::default().fg(color))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::TOP)
                    .border_style(Style::default().fg(LINE)),
            ),
        rows[2],
    );
}

fn render_credential_key_edit(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(78);
    let height = area.height.min(15);
    let modal = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal);
    let alias = app.selected_credential_alias().unwrap_or_default();
    let block = Block::default()
        .title(format!(" SSH KEY · {alias} "))
        .title_bottom(Line::from(" Enter save  ·  Esc cancel ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(AMBER))
        .style(canvas_style(app));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(2),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("Private SSH key path", Style::default().fg(TEXT).bold()),
            Line::from(""),
            Line::styled(
                "The file must already exist. Paths beginning with ~/ are supported.",
                Style::default().fg(Color::Gray),
            ),
        ])
        .alignment(Alignment::Center),
        rows[0],
    );
    let cursor = if app.animation_tick % 10 < 5 {
        "_"
    } else {
        " "
    };
    frame.render_widget(
        Paragraph::new(format!(" {}{cursor}", app.credential_key_draft)).block(
            Block::default()
                .title(" Key file ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(AMBER)),
        ),
        rows[1],
    );
    if let Some(error) = app.credential_key_error.as_deref() {
        frame.render_widget(
            Paragraph::new(error)
                .style(Style::default().fg(RED))
                .alignment(Alignment::Center),
            rows[2],
        );
    }
}

fn render_theme_import(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(82);
    let height = area.height.min(20);
    let modal = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .title(" IMPORT COMMUNITY THEME ")
        .title_bottom(Line::from(" Enter import  ·  Esc cancel ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(AMBER))
        .style(canvas_style(app));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Min(3),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                "Choose a nekoHub theme JSON file.",
                Style::default().fg(TEXT).bold(),
            ),
            Line::from(""),
            Line::styled(
                "Required: name, primary, secondary, tertiary, success, background,",
                Style::default().fg(Color::Gray),
            ),
            Line::styled(
                "surface, border, text and muted.",
                Style::default().fg(Color::Gray),
            ),
            Line::styled("Colors use #RRGGBB.", Style::default().fg(DIM)),
        ])
        .alignment(Alignment::Center),
        rows[0],
    );
    let cursor = if app.animation_tick % 10 < 5 {
        "_"
    } else {
        " "
    };
    frame.render_widget(
        Paragraph::new(format!(" {}{cursor}", app.theme_import_draft)).block(
            Block::default()
                .title(" Theme file path ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(AMBER)),
        ),
        rows[1],
    );
    if let Some(error) = app.theme_import_error.as_deref() {
        frame.render_widget(
            Paragraph::new(error)
                .style(Style::default().fg(RED))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            rows[2],
        );
    }
}

#[allow(clippy::too_many_lines)]
fn render_agent_setup(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let failed = app.setup_error.is_some();
    let accent = if failed { RED } else { AMBER };
    let activity = ["·", "•", "●", "•"][(app.animation_tick as usize / 3) % 4];
    let footer = if failed {
        " Enter retry  ·  Esc back  ·  q quit "
    } else {
        " Setting up a read-only local service  ·  q quit "
    };
    let outer = Block::default()
        .title(Line::from(vec![
            Span::styled(format!(" {activity} "), Style::default().fg(accent)),
            Span::styled("nekoHub", Style::default().fg(AMBER).bold()),
            Span::styled(" / AGENT SETUP ", Style::default().fg(DIM)),
        ]))
        .title_bottom(Line::from(footer).centered())
        .borders(Borders::ALL)
        .border_style(Style::default().fg(accent));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let width = inner.width.min(70);
    let height = inner.height.min(if failed { 18 } else { 15 });
    let panel = Rect::new(
        inner.x + inner.width.saturating_sub(width) / 2,
        inner.y + inner.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let block = Block::default()
        .title(if failed {
            " Agent needs attention "
        } else {
            " Installing nekoHub agent "
        })
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent));
    let content = block.inner(panel);
    frame.render_widget(block, panel);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(3),
            Constraint::Min(4),
        ])
        .split(content);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                if failed {
                    "SETUP PAUSED"
                } else {
                    "PREPARING THIS MACHINE"
                },
                Style::default().fg(accent).bold(),
            ),
            Line::from(""),
            Line::styled(app.setup_message.as_str(), Style::default().fg(Color::Gray)),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true }),
        rows[0],
    );

    let gauge_area = Rect::new(
        rows[1].x + 2,
        rows[1].y + 1,
        rows[1].width.saturating_sub(4),
        1,
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(
                Style::default()
                    .fg(if failed { RED } else { AMBER })
                    .bg(DIM),
            )
            .ratio(f64::from(app.setup_visible_progress) / 100.0)
            .label(format!("{:>3}%", app.setup_visible_progress)),
        gauge_area,
    );

    if let Some(error) = &app.setup_error {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(error.as_str(), Style::default().fg(RED)),
                Line::from(""),
                Line::styled(
                    "sudo apt install nekohub-agent",
                    Style::default().fg(AMBER).bold(),
                ),
                Line::styled(
                    "sudo systemctl enable --now nekohub-agent",
                    Style::default().fg(AMBER).bold(),
                ),
            ])
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
            rows[2],
        );
    } else {
        let dots = ".".repeat((app.animation_tick as usize / 3) % 4);
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    format!("Verifying native Linux metrics{dots}"),
                    Style::default().fg(DIM),
                ),
                Line::from(""),
                Line::styled(
                    "No shell commands · no SSH polling · no root access",
                    Style::default().fg(DIM),
                ),
            ])
            .alignment(Alignment::Center),
            rows[2],
        );
    }
}

fn render_welcome(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let outer = Block::default()
        .title(Line::from(vec![
            Span::styled(" nekoHub ", Style::default().fg(INK).bg(AMBER).bold()),
            Span::styled(" / INITIAL SETUP ", Style::default().fg(DIM)),
        ]))
        .title_bottom(
            Line::from(" ↑↓ select  ·  Enter continue  ·  q quit ")
                .style(Style::default().fg(DIM))
                .centered(),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let content_height = if app.welcome_notice.is_some() { 14 } else { 12 };
    let vertical_margin = inner.height.saturating_sub(content_height) / 2;
    let centered = Rect::new(
        inner.x,
        inner.y + vertical_margin,
        inner.width,
        content_height.min(inner.height),
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(2),
        ])
        .split(centered);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                "Start with one machine. Build your fleet over time.",
                Style::default().fg(TEXT).bold(),
            ),
            Line::styled(
                "Choose where nekoHub should begin.",
                Style::default().fg(DIM),
            ),
        ])
        .alignment(Alignment::Center),
        rows[0],
    );
    render_welcome_button(
        frame,
        centered_button(rows[1], 42),
        "Monitor this machine",
        app.welcome_selected == 0,
    );
    render_welcome_button(
        frame,
        centered_button(rows[3], 42),
        "Connect to a remote machine",
        app.welcome_selected == 1,
    );
    if let Some(notice) = &app.welcome_notice {
        frame.render_widget(
            Paragraph::new(notice.as_str())
                .style(Style::default().fg(ORANGE))
                .alignment(Alignment::Center),
            rows[4],
        );
    }
}

fn centered_button(area: Rect, max_width: u16) -> Rect {
    let width = area.width.min(max_width);
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y,
        width,
        area.height,
    )
}

fn render_welcome_button(frame: &mut Frame<'_>, area: Rect, label: &str, selected: bool) {
    let border = if selected { AMBER } else { DIM };
    let style = if selected {
        Style::default().fg(INK).bg(AMBER).bold()
    } else {
        Style::default().fg(Color::Gray)
    };
    frame.render_widget(
        Paragraph::new(format!("{} {label}", if selected { "›" } else { " " }))
            .style(style)
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(if selected {
                        BorderType::Thick
                    } else {
                        BorderType::Plain
                    })
                    .border_style(Style::default().fg(border)),
            ),
        area,
    );
}

#[allow(clippy::too_many_lines)]
fn render_remote_picker(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let inner = render_app_chrome(
        frame,
        area,
        app,
        1,
        "↑↓ select  ·  Enter connect  ·  a alias  ·  g group  ·  u uninstall  ·  Esc home",
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(7),
            Constraint::Length(4),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("Machines", Style::default().fg(TEXT).bold()),
            Line::styled(
                "All discovered machines live here before you organize them into groups.",
                Style::default().fg(DIM),
            ),
        ]),
        rows[0],
    );
    let panel = rows[1];
    if let Some(notice) = &app.remote_notice {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("MACHINE STATUS", Style::default().fg(AMBER).bold()),
                Line::from(""),
                Line::from(notice.as_str()),
                Line::from(""),
                Line::styled(
                    "SSH is used only for setup and administration, never for metric polling.",
                    Style::default().fg(DIM),
                ),
            ])
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(" Remote monitoring ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(AMBER)),
            ),
            panel,
        );
        return;
    }
    let local_selected = app.remote_selected == 0;
    let local_group = app
        .machine_groups
        .iter()
        .find(|group| group.host_ids.iter().any(|id| id == "local"))
        .map_or("Unassigned", |group| group.name.as_str());
    let local = Row::new([
        if local_selected { "›" } else { " " },
        app.local_name.as_str(),
        "localhost",
        "Local agent",
        local_group,
        "● ready",
    ])
    .style(if local_selected {
        Style::default().fg(INK).bg(AMBER).bold()
    } else {
        Style::default().fg(Color::Gray)
    });
    let remote_rows = app.remote_hosts.iter().enumerate().map(|(index, host)| {
        let selected = index + 1 == app.remote_selected;
        let installed = host.tags.iter().any(|tag| tag == INSTALLED_TAG);
        let group = app
            .machine_groups
            .iter()
            .find(|group| group.host_ids.contains(&host.id))
            .map_or("Unassigned", |group| group.name.as_str());
        Row::new([
            if selected { "›" } else { " " },
            host.display_name.as_str(),
            host.alias.as_str(),
            if installed {
                "nekoHub agent"
            } else {
                "SSH inventory"
            },
            group,
            if installed {
                "● agent installed"
            } else {
                "○ agent needed"
            },
        ])
        .style(if selected {
            Style::default().fg(INK).bg(AMBER).bold()
        } else {
            Style::default().fg(Color::Gray)
        })
    });
    let table = Table::new(
        std::iter::once(local).chain(remote_rows),
        [
            Constraint::Length(2),
            Constraint::Min(14),
            Constraint::Min(14),
            Constraint::Length(14),
            Constraint::Length(14),
            Constraint::Length(17),
        ],
    )
    .header(
        Row::new(["", "Alias", "Connection", "Source", "Group", "Status"])
            .style(Style::default().fg(DIM).bold())
            .bottom_margin(1),
    )
    .block(
        Block::default()
            .title(format!(
                " registered machines  {} ",
                app.remote_hosts.len() + 1
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)),
    );
    frame.render_widget(table, panel);

    let install_selected = app.remote_install_selected();
    let can_uninstall = app.selected_installed_remote().is_some();
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    if install_selected { " › + " } else { "   + " },
                    Style::default().fg(AMBER).bold(),
                ),
                Span::styled(
                    "Install agent over SSH",
                    if install_selected {
                        Style::default().fg(INK).bg(AMBER).bold()
                    } else {
                        Style::default().fg(TEXT).bold()
                    },
                ),
                Span::styled("    Connect with user@machine", Style::default().fg(DIM)),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled(
                    "   [u] ",
                    Style::default()
                        .fg(if can_uninstall { RED } else { DIM })
                        .bold(),
                ),
                Span::styled(
                    "Uninstall selected agent",
                    Style::default()
                        .fg(if can_uninstall { TEXT } else { DIM })
                        .bold(),
                ),
                Span::styled(
                    "    Removes it from the fleet  ·  [g] group  ·  [a] alias",
                    Style::default().fg(DIM),
                ),
            ]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if install_selected { AMBER } else { LINE })),
        ),
        rows[2],
    );
}

fn render_machine_alias(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(62);
    let height = area.height.min(13);
    let modal = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal);
    let identity = if app.remote_selected == 0 {
        "localhost"
    } else {
        app.remote_hosts
            .get(app.remote_selected - 1)
            .map_or("machine", |machine| machine.alias.as_str())
    };
    let cursor = if app.animation_tick % 10 < 5 {
        "_"
    } else {
        " "
    };
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" /\\ ", Style::default().fg(AMBER).bold()),
            Span::styled(" NAME THIS MACHINE ", Style::default().fg(TEXT).bold()),
        ]))
        .title_bottom(Line::from(" Enter save  ·  Ctrl+U reset  ·  Esc cancel ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(AMBER))
        .style(canvas_style(app));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(2),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("Give it a friendly name", Style::default().fg(TEXT).bold()),
            Line::styled(
                format!("Connection identity stays {identity}"),
                Style::default().fg(DIM),
            ),
        ])
        .alignment(Alignment::Center),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(format!(" {}{cursor}", app.machine_alias_draft)).block(
            Block::default()
                .title(" alias · up to 32 characters ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(AMBER)),
        ),
        rows[1],
    );
    let hint = app
        .machine_alias_error
        .as_deref()
        .unwrap_or("Clear the field and save to restore the original machine name.");
    frame.render_widget(
        Paragraph::new(hint)
            .style(Style::default().fg(if app.machine_alias_error.is_some() {
                RED
            } else {
                DIM
            }))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true }),
        rows[2],
    );
}

fn render_remote_uninstall_confirmation(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(72);
    let height = area.height.min(14);
    let modal = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("UNINSTALL REMOTE AGENT", Style::default().fg(RED).bold()),
            Line::from(""),
            Line::from(vec![
                Span::styled("Machine  ", Style::default().fg(DIM)),
                Span::styled(
                    app.remote_install_draft.as_str(),
                    Style::default().fg(TEXT).bold(),
                ),
            ]),
            Line::from(""),
            Line::styled(
                "This removes nekohub-agent from the machine and unregisters it from monitoring.",
                Style::default().fg(Color::Gray),
            ),
            Line::from(""),
            Line::styled(
                "SSH / sudo password (leave empty when your key works)",
                Style::default().fg(DIM),
            ),
            Line::styled(
                format!(
                    "  {}{}",
                    "•".repeat(app.remote_password_draft.chars().count()),
                    if app.animation_tick % 10 < 5 {
                        "_"
                    } else {
                        " "
                    }
                ),
                Style::default().fg(TEXT),
            ),
            Line::from(""),
            Line::styled(
                "Press Enter to uninstall, or Esc to cancel.",
                Style::default().fg(RED),
            ),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(" CONFIRM REMOVAL ")
                .title_bottom(Line::from(" Enter uninstall  ·  Esc cancel ").centered())
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(Style::default().fg(RED))
                .style(canvas_style(app)),
        ),
        modal,
    );
}

#[allow(clippy::too_many_lines)]
fn render_remote_install(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(76);
    let height = area.height.min(22);
    let modal = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal);
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" /\\ ", Style::default().fg(AMBER).bold()),
            Span::styled(" INSTALL REMOTE AGENT ", Style::default().fg(TEXT).bold()),
        ]))
        .title_bottom(
            Line::from(" Tab field  ·  Enter connect & install  ·  Esc cancel ").centered(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(AMBER))
        .style(canvas_style(app));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Length(4),
            Constraint::Min(2),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                "Connect to a Debian or Ubuntu amd64 machine over SSH.",
                Style::default().fg(TEXT).bold(),
            ),
            Line::from(""),
            Line::styled(
                "nekoHub installs the agent, pairs its metric channel, and verifies a live sample.",
                Style::default().fg(Color::Gray),
            ),
            Line::styled(
                "Use a key, or enter the SSH password below. The password is never saved.",
                Style::default().fg(DIM),
            ),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true }),
        rows[0],
    );
    let cursor = if app.animation_tick % 10 < 5 {
        "_"
    } else {
        " "
    };
    frame.render_widget(
        Paragraph::new(format!(
            " {}{}",
            app.remote_install_draft,
            if app.remote_install_field == 0 {
                cursor
            } else {
                " "
            }
        ))
        .block(
            Block::default()
                .title(" SSH destination · user@machine ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(AMBER)),
        ),
        rows[1],
    );
    let masked = "•".repeat(app.remote_password_draft.chars().count());
    frame.render_widget(
        Paragraph::new(format!(
            " {masked}{}",
            if app.remote_install_field == 1 {
                cursor
            } else {
                " "
            }
        ))
        .block(
            Block::default()
                .title(" SSH / sudo password · optional when a key works ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if app.remote_install_field == 1 {
                    AMBER
                } else {
                    LINE
                })),
        ),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("What happens next", Style::default().fg(AMBER).bold()),
            Line::styled(
                "Setup stays inside nekoHub and opens the dashboard only when everything is ready.",
                Style::default().fg(Color::Gray),
            ),
        ])
        .wrap(Wrap { trim: true }),
        rows[3],
    );
    if let Some(error) = app.remote_install_error.as_deref() {
        frame.render_widget(
            Paragraph::new(error)
                .style(Style::default().fg(RED))
                .alignment(Alignment::Center),
            rows[4],
        );
    }
}

fn render_remote_connect(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(72);
    let height = area.height.min(17);
    let modal = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal);
    let masked = "•".repeat(app.remote_password_draft.chars().count());
    let cursor = if app.animation_tick % 10 < 5 {
        "_"
    } else {
        " "
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("OPEN LIVE METRICS", Style::default().fg(CYAN).bold()),
            Line::from(""),
            Line::from(vec![
                Span::styled("Machine  ", Style::default().fg(DIM)),
                Span::styled(
                    app.remote_install_draft.as_str(),
                    Style::default().fg(TEXT).bold(),
                ),
            ]),
            Line::from(""),
            Line::styled(
                "Metrics come from nekohub-agent through an encrypted SSH tunnel.",
                Style::default().fg(Color::Gray),
            ),
            Line::from(""),
            Line::styled(
                "SSH password (leave empty when your key works)",
                Style::default().fg(DIM),
            ),
            Line::styled(format!("  {masked}{cursor}"), Style::default().fg(TEXT)),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title(" CONNECT AGENT ")
                .title_bottom(Line::from(" Enter connect  ·  Esc cancel ").centered())
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(Style::default().fg(CYAN))
                .style(canvas_style(app)),
        ),
        modal,
    );
}

#[allow(clippy::too_many_lines)]
fn render_remote_install_progress(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(86);
    let height = area.height.min(26);
    let modal = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, modal);
    let failed = app.remote_install_error.is_some();
    let accent = if failed {
        RED
    } else if app.remote_install_complete {
        GREEN
    } else {
        AMBER
    };
    let footer = if app.remote_install_complete && !app.remote_uninstalling {
        " Enter open live metrics "
    } else if app.remote_install_complete || failed {
        " Enter return to Machines "
    } else {
        " Installing over SSH · nekoHub stays open "
    };
    let block = Block::default()
        .title(if app.remote_uninstalling {
            " REMOTE AGENT REMOVAL "
        } else {
            " REMOTE AGENT SETUP "
        })
        .title_bottom(Line::from(footer).centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(accent))
        .style(canvas_style(app));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(7),
            Constraint::Min(6),
            Constraint::Length(3),
        ])
        .split(inner);
    let activity = ["·", "•", "●", "•"][(app.animation_tick as usize / 3) % 4];
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                if failed {
                    "INSTALLATION STOPPED"
                } else if app.remote_uninstalling && app.remote_install_complete {
                    "AGENT REMOVED"
                } else if app.remote_uninstalling {
                    "UNINSTALLING NEKOHUB AGENT"
                } else if app.remote_install_complete {
                    "AGENT + METRICS READY"
                } else {
                    "INSTALLING NEKOHUB AGENT"
                },
                Style::default().fg(accent).bold(),
            ),
            Line::from(""),
            Line::from(vec![
                Span::styled(format!("{activity} "), Style::default().fg(accent)),
                Span::styled(
                    app.remote_install_message.as_str(),
                    Style::default().fg(TEXT),
                ),
            ]),
        ])
        .alignment(Alignment::Center),
        rows[0],
    );
    render_neko_delivery(frame, rows[1], app, accent);
    let logs = app
        .remote_install_logs
        .iter()
        .rev()
        .take(rows[2].height.saturating_sub(2) as usize)
        .rev()
        .map(|line| Line::styled(format!("  {line}"), Style::default().fg(Color::Gray)))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(logs)
            .block(
                Block::default()
                    .title(" live output ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(LINE)),
            )
            .wrap(Wrap { trim: true }),
        rows[2],
    );
    if let Some(error) = app.remote_install_error.as_deref() {
        frame.render_widget(
            Paragraph::new(error)
                .style(Style::default().fg(RED))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            rows[3],
        );
    } else if app.remote_install_complete {
        frame.render_widget(
            Paragraph::new(if app.remote_uninstalling {
                "Agent removed from the monitored fleet."
            } else {
                "Machine registered, paired, and already sending live metrics."
            })
            .style(Style::default().fg(GREEN).bold())
            .alignment(Alignment::Center),
            rows[3],
        );
    }
}

fn render_agent_update_auth(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(72);
    let height = area.height.min(17);
    let modal = centered_rect(area, width, height);
    frame.render_widget(Clear, modal);
    let masked = "•".repeat(app.remote_password_draft.chars().count());
    let cursor = if app.animation_tick % 10 < 5 {
        "_"
    } else {
        " "
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("AGENT UPDATE", Style::default().fg(AMBER).bold()),
            Line::from(""),
            Line::from(vec![
                Span::styled("Machine  ", Style::default().fg(DIM)),
                Span::styled(&app.remote_install_draft, Style::default().fg(TEXT).bold()),
            ]),
            Line::from(""),
            Line::styled(
                "Enter the sudo password, or leave empty when sudo does not require one.",
                Style::default().fg(Color::Gray),
            ),
            Line::from(""),
            Line::styled("SSH / sudo password", Style::default().fg(DIM)),
            Line::styled(format!("  {masked}{cursor}"), Style::default().fg(TEXT)),
        ])
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .title(" UPDATE AUTHENTICATION ")
                .title_bottom(Line::from(" Enter update  ·  Esc cancel ").centered())
                .borders(Borders::ALL)
                .border_type(BorderType::Thick)
                .border_style(Style::default().fg(AMBER))
                .style(canvas_style(app)),
        ),
        modal,
    );
}

fn render_agent_update_progress(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(82);
    let height = area.height.min(24);
    let modal = centered_rect(area, width, height);
    frame.render_widget(Clear, modal);
    let failed = app.remote_install_error.is_some();
    let complete = app.remote_install_complete;
    let color = if failed {
        RED
    } else if complete {
        GREEN
    } else {
        AMBER
    };
    let activity = ["◐", "◓", "◑", "◒"][(app.animation_tick as usize / 3) % 4];
    let block = Block::default()
        .title(" AGENT UPDATE ")
        .title_bottom(
            Line::from(if failed || complete {
                " Enter close "
            } else {
                " Updating securely over SSH "
            })
            .centered(),
        )
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(color))
        .style(canvas_style(app));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                if complete {
                    "AGENT IS UP TO DATE"
                } else if failed {
                    "UPDATE STOPPED"
                } else {
                    "UPDATING REMOTE AGENT"
                },
                Style::default().fg(color).bold(),
            ),
            Line::from(""),
            Line::from(vec![
                Span::styled(format!("{activity} "), Style::default().fg(color)),
                Span::styled(&app.remote_install_message, Style::default().fg(TEXT)),
            ]),
        ])
        .alignment(Alignment::Center),
        rows[0],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(color).bg(SURFACE).bold())
            .ratio(f64::from(app.remote_install_progress) / 100.0)
            .label(format!("{}%", app.remote_install_progress)),
        Rect::new(
            rows[1].x + 3,
            rows[1].y + 1,
            rows[1].width.saturating_sub(6),
            1,
        ),
    );
    let logs = app
        .remote_install_logs
        .iter()
        .rev()
        .take(rows[2].height.saturating_sub(2) as usize)
        .rev()
        .map(|line| Line::styled(format!("  {line}"), Style::default().fg(Color::Gray)))
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(logs).block(
            Block::default()
                .title(" update output ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(LINE)),
        ),
        rows[2],
    );
    if let Some(error) = app.remote_install_error.as_deref() {
        frame.render_widget(
            Paragraph::new(error)
                .style(Style::default().fg(RED))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            rows[3],
        );
    }
}

fn centered_rect(area: Rect, width: u16, height: u16) -> Rect {
    Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

fn render_neko_delivery(frame: &mut Frame<'_>, area: Rect, app: &App, accent: Color) {
    if area.width < 46 || area.height < 6 {
        frame.render_widget(
            Gauge::default()
                .gauge_style(Style::default().fg(accent).bg(DIM))
                .ratio(f64::from(app.remote_install_progress) / 100.0)
                .label(format!("{}%", app.remote_install_progress)),
            Rect::new(area.x + 2, area.y + 2, area.width.saturating_sub(4), 1),
        );
        return;
    }

    let depot_width = area.width.clamp(20, 28);
    let depot = Rect::new(
        area.right().saturating_sub(depot_width),
        area.y,
        depot_width,
        area.height,
    );
    let route_width = depot.x.saturating_sub(area.x).saturating_sub(10);
    let travel = route_width.saturating_sub(2).max(1);
    let courier_offset = if app.remote_install_complete {
        travel
    } else {
        u16::try_from((app.animation_tick / 2) % u64::from(travel.saturating_add(5)))
            .unwrap_or_default()
            .min(travel)
    };
    let paws = if (app.animation_tick / 3) & 1 == 0 {
        " /| |\\"
    } else {
        "  /|\\ "
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(" /\\_/\\", Style::default().fg(ORANGE).bold()),
            Line::from(vec![
                Span::styled("( o.o )", Style::default().fg(TEXT).bold()),
                Span::styled("■", Style::default().fg(accent).bold()),
            ]),
            Line::styled(paws, Style::default().fg(ORANGE)),
        ]),
        Rect::new(
            area.x.saturating_add(courier_offset),
            area.y.saturating_add(1),
            9,
            3,
        ),
    );
    let route = "· ".repeat(usize::from(route_width / 2));
    frame.render_widget(
        Paragraph::new(route).style(Style::default().fg(LINE)),
        Rect::new(area.x, area.y.saturating_add(5), route_width, 1),
    );

    let depot_block = Block::default()
        .title(if app.remote_uninstalling {
            " NEKO PICKUP "
        } else {
            " METRICS DEPOT "
        })
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(accent));
    let depot_inner = depot_block.inner(depot);
    frame.render_widget(depot_block, depot);
    let slots = usize::from(depot_inner.width.saturating_sub(2)).max(1);
    let progress = if app.remote_uninstalling {
        100_u16.saturating_sub(app.remote_install_progress)
    } else {
        app.remote_install_progress
    };
    let filled = usize::from(progress) * slots / 100;
    let cargo = format!("{}{}", "■".repeat(filled), "·".repeat(slots - filled));
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(""),
            Line::styled(cargo, Style::default().fg(accent).bold()),
            Line::from(""),
            Line::styled(
                format!("{:>3}%  paired delivery", app.remote_install_progress),
                Style::default().fg(TEXT),
            ),
        ])
        .alignment(Alignment::Center),
        depot_inner,
    );
}

fn render_monitoring_bar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let host_name = app
        .selected()
        .map_or("Machine", |host| host.target.display_name.as_str());
    let live = app.online_count() == app.hosts.len() && !app.hosts.is_empty();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(host_name, Style::default().fg(TEXT).bold()),
            Span::styled("    ", Style::default()),
            Span::styled(
                " Overview ",
                if app.view == View::Overview {
                    Style::default().fg(INK).bg(AMBER).bold()
                } else {
                    Style::default().fg(DIM)
                },
            ),
            Span::styled("  Details ", Style::default().fg(DIM)),
            Span::styled(
                if live {
                    "    ● live"
                } else {
                    "    ○ connecting"
                },
                Style::default()
                    .fg(if live { GREEN } else { ORANGE })
                    .bold(),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(Color::Rgb(43, 64, 82))),
        ),
        area,
    );
}

fn render_overview(frame: &mut Frame<'_>, area: Rect, app: &App) {
    if app.hosts.len() == 1 {
        render_neko_dashboard(frame, area, app);
        return;
    }
    let summary_height = if area.height >= 24 { 8 } else { 6 };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(summary_height), Constraint::Min(8)])
        .split(area);
    render_summary(frame, rows[0], app);
    render_host_grid(frame, rows[1], app);
}

#[allow(clippy::too_many_lines)]
fn render_neko_dashboard(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(29),
            Constraint::Length(2),
            Constraint::Min(40),
        ])
        .split(area);
    render_monitor_sidebar(frame, columns[0], app);
    let Some(host) = app.selected() else {
        return;
    };
    if app.monitor_selected == 7 {
        render_terminal_panel(frame, animated_area(columns[2], app, 2, 4), app, host);
        return;
    }
    if app.monitor_selected == 1 {
        render_processes(frame, animated_area(columns[2], app, 2, 4), host);
        return;
    }
    if app.monitor_selected == 5 {
        render_containers(frame, animated_area(columns[2], app, 2, 4), host);
        return;
    }
    if app.monitor_selected == 3 {
        render_storage_view(frame, animated_area(columns[2], app, 2, 4), app, host);
        return;
    }
    if app.monitor_selected != 0 {
        render_monitor_placeholder(frame, animated_area(columns[2], app, 2, 4), app, host);
        return;
    }
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Min(14),
        ])
        .split(columns[2]);
    let snapshot = host.snapshot.as_ref();
    let state_color = if host.last_error.is_some() {
        RED
    } else if snapshot.is_some() {
        GREEN
    } else {
        ORANGE
    };
    let identity = snapshot.map_or_else(
        || "Waiting for the first sample".to_owned(),
        |sample| format!("{} · {}", sample.os, sample.kernel),
    );
    let hostname = snapshot.map_or(host.target.display_name.as_str(), |sample| {
        sample.hostname.as_str()
    });
    let agent_version = app.selected_agent_version();
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("NOW MONITORING  ", Style::default().fg(DIM).bold()),
                Span::styled(hostname, Style::default().fg(TEXT).bold()),
                Span::styled(
                    if host.last_error.is_some() {
                        "    ○ OFFLINE"
                    } else if snapshot.is_some() {
                        "    ● LIVE"
                    } else {
                        "    ◌ COLLECTING"
                    },
                    Style::default().fg(state_color).bold(),
                ),
            ]),
            Line::styled(
                snapshot.map_or_else(
                    || {
                        agent_version.map_or_else(
                            || "checking agent version...".to_owned(),
                            |version| format!("collecting metrics    agent v{version}"),
                        )
                    },
                    |sample| {
                        format!(
                            "uptime {}    latency {}ms    agent v{}",
                            human_duration(Duration::from_secs(sample.uptime_secs)),
                            sample.latency_ms,
                            agent_version.unwrap_or("legacy")
                        )
                    },
                ),
                Style::default().fg(Color::Gray),
            ),
            if app.selected_agent_needs_update() {
                Line::styled(
                    if app.agent_update_is_armed() {
                        "UPDATE ARMED · press \" once more to start"
                    } else {
                        "AGENT UPDATE AVAILABLE · press \" twice to update"
                    },
                    Style::default().fg(ORANGE).bold(),
                )
            } else {
                Line::from(vec![
                    Span::styled(identity, Style::default().fg(DIM)),
                    Span::styled("    agent is up to date", Style::default().fg(GREEN)),
                ])
            },
        ])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(LINE)),
        ),
        animated_area(rows[0], app, 0, 4),
    );
    let dashboard = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(64),
            Constraint::Length(2),
            Constraint::Percentage(36),
        ])
        .split(rows[2]);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(58),
            Constraint::Length(1),
            Constraint::Min(7),
        ])
        .split(dashboard[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(1),
            Constraint::Length(7),
            Constraint::Length(1),
            Constraint::Min(6),
        ])
        .split(dashboard[2]);
    render_cpu_history(frame, animated_area(left[0], app, 1, 4), host);
    render_network_history(frame, animated_area(left[2], app, 3, 4), host);
    render_resource_panel(
        frame,
        animated_area(right[0], app, 2, 4),
        "MEMORY",
        snapshot.and_then(|sample| sample.memory.percent()),
        snapshot.map_or("waiting for data".into(), |sample| {
            format!(
                "{} / {}",
                bytes(sample.memory.used),
                bytes(sample.memory.total)
            )
        }),
        CYAN,
    );
    render_resource_panel(
        frame,
        animated_area(right[2], app, 3, 4),
        "ROOT DISK",
        snapshot.and_then(|sample| sample.root_disk.percent()),
        snapshot.map_or("waiting for data".into(), |sample| {
            format!(
                "{} / {}",
                bytes(sample.root_disk.used),
                bytes(sample.root_disk.total)
            )
        }),
        GREEN,
    );
    render_system_pulse(frame, animated_area(right[4], app, 4, 4), host);
}

fn render_processes(frame: &mut Frame<'_>, area: Rect, host: &HostState) {
    let processes = host
        .snapshot
        .as_ref()
        .map_or(&[][..], |snapshot| snapshot.processes.as_slice());
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" PROCESSES  ", Style::default().fg(CYAN).bold()),
            Span::styled(
                format!("{} tracked", processes.len()),
                Style::default().fg(DIM),
            ),
        ]))
        .title_bottom(Line::styled(
            " sorted by live CPU, then memory ",
            Style::default().fg(DIM),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(CYAN));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if processes.is_empty() {
        render_monitor_empty(
            frame,
            inner,
            "No process samples yet",
            "Waiting for the agent. Processes appear on the first collection.",
            CYAN,
        );
        return;
    }
    let visible = usize::from(inner.height.saturating_sub(2));
    if inner.width < 78 {
        let header = Row::new(["PID", "PROCESS", "CPU", "MEMORY"])
            .style(Style::default().fg(CYAN).bold())
            .bottom_margin(1);
        let rows = processes.iter().take(visible).map(|process| {
            Row::new([
                process.pid.to_string(),
                clipped(&process.name, 22),
                format!("{:>5.1}%", process.cpu_percent),
                bytes(process.memory_bytes),
            ])
            .style(process_style(&process.state))
        });
        frame.render_widget(
            Table::new(
                rows,
                [
                    Constraint::Length(7),
                    Constraint::Min(10),
                    Constraint::Length(8),
                    Constraint::Length(11),
                ],
            )
            .header(header)
            .column_spacing(1),
            inner,
        );
    } else {
        let header = Row::new(["PID", "S", "PROCESS", "CPU", "MEMORY", "COMMAND"])
            .style(Style::default().fg(CYAN).bold())
            .bottom_margin(1);
        let rows = processes.iter().take(visible).map(|process| {
            Row::new([
                process.pid.to_string(),
                process.state.clone(),
                clipped(&process.name, 20),
                format!("{:>5.1}%", process.cpu_percent),
                bytes(process.memory_bytes),
                clipped(&process.command, 48),
            ])
            .style(process_style(&process.state))
        });
        frame.render_widget(
            Table::new(
                rows,
                [
                    Constraint::Length(8),
                    Constraint::Length(3),
                    Constraint::Length(20),
                    Constraint::Length(8),
                    Constraint::Length(11),
                    Constraint::Min(12),
                ],
            )
            .header(header)
            .column_spacing(1),
            inner,
        );
    }
}

fn process_style(state: &str) -> Style {
    Style::default().fg(if state == "R" { GREEN } else { Color::Gray })
}

#[allow(clippy::too_many_lines)]
fn render_containers(frame: &mut Frame<'_>, area: Rect, host: &HostState) {
    let containers = host
        .snapshot
        .as_ref()
        .map_or(&[][..], |snapshot| snapshot.containers.as_slice());
    let running = containers
        .iter()
        .filter(|container| container.state == "running")
        .count();
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" CONTAINERS  ", Style::default().fg(CYAN).bold()),
            Span::styled(
                format!("{running} running  ·  {} discovered", containers.len()),
                Style::default().fg(DIM),
            ),
        ]))
        .title_bottom(Line::styled(
            " Docker and Podman · refreshed with the machine sample ",
            Style::default().fg(DIM),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(CYAN));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if containers.is_empty() {
        render_monitor_empty(
            frame,
            inner,
            "No active containers found",
            "Start a container or grant the agent access to the Docker/Podman runtime.",
            CYAN,
        );
        return;
    }
    let visible = usize::from(inner.height.saturating_sub(2));
    if inner.width < 100 {
        let header = Row::new(["ENGINE", "CONTAINER", "CPU", "MEMORY", "PIDS"])
            .style(Style::default().fg(CYAN).bold())
            .bottom_margin(1);
        let rows = containers.iter().take(visible).map(|container| {
            Row::new([
                container.engine.clone(),
                clipped(&container.name, 20),
                format!("{:.1}%", container.cpu_percent),
                container_memory(container),
                container.pids.to_string(),
            ])
            .style(container_style(&container.state))
        });
        frame.render_widget(
            Table::new(
                rows,
                [
                    Constraint::Length(8),
                    Constraint::Min(10),
                    Constraint::Length(8),
                    Constraint::Length(18),
                    Constraint::Length(6),
                ],
            )
            .header(header)
            .column_spacing(1),
            inner,
        );
    } else {
        let header = Row::new([
            "ENGINE",
            "CONTAINER",
            "STATE",
            "CPU",
            "MEMORY",
            "NETWORK",
            "BLOCK I/O",
            "PIDS",
        ])
        .style(Style::default().fg(CYAN).bold())
        .bottom_margin(1);
        let rows = containers.iter().take(visible).map(|container| {
            Row::new([
                container.engine.clone(),
                clipped(&container.name, 18),
                container.state.clone(),
                format!("{:.1}%", container.cpu_percent),
                container_memory(container),
                format!(
                    "↓{} ↑{}",
                    bytes(container.network_rx_bytes),
                    bytes(container.network_tx_bytes)
                ),
                format!(
                    "↓{} ↑{}",
                    bytes(container.block_read_bytes),
                    bytes(container.block_write_bytes)
                ),
                container.pids.to_string(),
            ])
            .style(container_style(&container.state))
        });
        frame.render_widget(
            Table::new(
                rows,
                [
                    Constraint::Length(8),
                    Constraint::Length(18),
                    Constraint::Length(9),
                    Constraint::Length(8),
                    Constraint::Length(20),
                    Constraint::Length(20),
                    Constraint::Length(20),
                    Constraint::Length(6),
                ],
            )
            .header(header)
            .column_spacing(1),
            inner,
        );
    }
}

fn container_memory(container: &nekohub_core::ContainerSnapshot) -> String {
    if container.memory_limit_bytes == 0 {
        bytes(container.memory_used_bytes)
    } else {
        format!(
            "{} / {}",
            bytes(container.memory_used_bytes),
            bytes(container.memory_limit_bytes)
        )
    }
}

fn container_style(state: &str) -> Style {
    Style::default().fg(if state == "running" { GREEN } else { ORANGE })
}

fn render_monitor_empty(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    detail: &str,
    color: Color,
) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(title, Style::default().fg(color).bold()),
            Line::from(""),
            Line::styled(detail, Style::default().fg(Color::Gray)),
        ])
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true }),
        vertically_centered(area, 3),
    );
}

#[allow(clippy::too_many_lines)]
fn render_storage_view(frame: &mut Frame<'_>, area: Rect, app: &App, host: &HostState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Min(12),
        ])
        .split(area);
    let hostname = host
        .snapshot
        .as_ref()
        .map_or(host.target.display_name.as_str(), |snapshot| {
            snapshot.hostname.as_str()
        });
    let summary = app.storage_snapshot.as_ref().map_or_else(
        || format!("Scanning {}", app.storage_path),
        |storage| {
            format!(
                "{} in {}  ·  {} files  ·  disk {} / {}  ·  {}ms",
                bytes(storage.scanned_bytes),
                storage.root,
                storage.file_count,
                bytes(storage.used_bytes),
                bytes(storage.total_bytes),
                storage.elapsed_ms
            )
        },
    );
    let ratio = app
        .storage_snapshot
        .as_ref()
        .filter(|storage| storage.total_bytes > 0)
        .map_or(0.0, |storage| {
            storage.used_bytes as f64 / storage.total_bytes as f64
        })
        .clamp(0.0, 1.0);
    let header = Block::default()
        .title(Line::from(vec![
            Span::styled(" STORAGE MAP  ", Style::default().fg(ORANGE).bold()),
            Span::styled(hostname, Style::default().fg(TEXT).bold()),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ORANGE));
    let header_inner = header.inner(rows[0]);
    frame.render_widget(header, rows[0]);
    let header_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(header_inner);
    frame.render_widget(
        Paragraph::new(summary).style(Style::default().fg(Color::Gray)),
        header_rows[0],
    );
    frame.render_widget(
        Gauge::default()
            .ratio(ratio)
            .label(format!("{:.0}%", ratio * 100.0))
            .gauge_style(Style::default().fg(ORANGE).bg(SURFACE)),
        header_rows[1],
    );

    if app.storage_loading
        && app
            .storage_snapshot
            .as_ref()
            .is_none_or(|snapshot| snapshot.root != app.storage_path)
    {
        let dots = ".".repeat(usize::try_from(app.animation_tick / 5 % 4).unwrap_or_default());
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(" /\\_/\\", Style::default().fg(ORANGE).bold()),
                Line::styled(
                    "( o.o )  walking the filesystem",
                    Style::default().fg(TEXT).bold(),
                ),
                Line::styled(format!(" > ^ <{dots}"), Style::default().fg(DIM)),
                Line::from(""),
                Line::styled(
                    format!("Opening {} · read-only scan", app.storage_path),
                    Style::default().fg(DIM),
                ),
            ])
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .title(" BUILDING MAP ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(ORANGE)),
            ),
            rows[2],
        );
        return;
    }
    if let Some(error) = app.storage_error.as_deref() {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(
                    "The storage map could not be built.",
                    Style::default().fg(RED).bold(),
                ),
                Line::from(""),
                Line::styled(error, Style::default().fg(Color::Gray)),
                Line::from(""),
                Line::styled("Press r to scan again.", Style::default().fg(ORANGE)),
            ])
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .title(" STORAGE SCAN ")
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(RED)),
            ),
            rows[2],
        );
        return;
    }
    let Some(storage) = app.storage_snapshot.as_ref() else {
        return;
    };
    let columns = storage_columns(rows[2]);
    let map_block = Block::default()
        .title(Line::styled(
            " WHAT IS USING SPACE ",
            Style::default().fg(DIM).bold(),
        ))
        .title_bottom(Line::styled(
            if app.storage_loading {
                " rescanning… "
            } else {
                " hover inspect · click open · Backspace/right-click up · r rescan "
            },
            Style::default().fg(if app.storage_loading { ORANGE } else { DIM }),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(LINE));
    let map_area = map_block.inner(columns[0]);
    frame.render_widget(map_block, columns[0]);
    let palette = [
        ORANGE,
        CYAN,
        GREEN,
        Color::Rgb(164, 143, 214),
        RED,
        AMBER,
        Color::Rgb(194, 139, 177),
    ];
    for (position, rectangle) in storage_treemap(&storage.entries, map_area) {
        let entry = &storage.entries[position];
        let hovered = app.storage_hovered == Some(position);
        let color = palette[position % palette.len()];
        let mut lines = Vec::new();
        if rectangle.width >= 9 && rectangle.height >= 3 {
            lines.push(Line::styled(
                clipped(&entry.name, usize::from(rectangle.width.saturating_sub(2))),
                Style::default().fg(INK).bold(),
            ));
        }
        if rectangle.width >= 12 && rectangle.height >= 4 {
            lines.push(Line::styled(
                bytes(entry.allocated_bytes),
                Style::default().fg(INK),
            ));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .style(Style::default().bg(color))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(if hovered {
                            BorderType::Double
                        } else {
                            BorderType::Plain
                        })
                        .border_style(Style::default().fg(if hovered { TEXT } else { SURFACE })),
                ),
            rectangle,
        );
    }
    render_storage_details(frame, columns[2], app, storage);
}

fn render_storage_details(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    storage: &nekohub_core::StorageSnapshot,
) {
    let entry = app
        .storage_hovered
        .and_then(|index| storage.entries.get(index))
        .or_else(|| storage.entries.first());
    let mut lines = entry.map_or_else(
        || {
            vec![Line::styled(
                "No readable entries",
                Style::default().fg(DIM),
            )]
        },
        |entry| {
            let percent = if storage.scanned_bytes == 0 {
                0.0
            } else {
                entry.allocated_bytes as f64 / storage.scanned_bytes as f64 * 100.0
            };
            vec![
                Line::styled(entry.name.as_str(), Style::default().fg(TEXT).bold()),
                Line::from(""),
                Line::styled("FULL PATH", Style::default().fg(DIM).bold()),
                Line::styled(entry.path.as_str(), Style::default().fg(Color::Gray)),
                Line::from(""),
                storage_detail_line("SIZE", bytes(entry.allocated_bytes), ORANGE),
                storage_detail_line("SHARE", format!("{percent:.1}%"), CYAN),
                storage_detail_line("FILES", entry.file_count.to_string(), TEXT),
                storage_detail_line(
                    "TYPE",
                    if entry.is_directory {
                        "directory"
                    } else {
                        "file"
                    }
                    .into(),
                    GREEN,
                ),
            ]
        },
    );
    if entry.is_some_and(|entry| entry.is_directory) {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            "Click to open this directory",
            Style::default().fg(ORANGE).bold(),
        ));
    }
    if storage.unreadable_entries > 0 || storage.truncated {
        lines.push(Line::from(""));
        lines.push(Line::styled(
            format!(
                "{} unreadable{}",
                storage.unreadable_entries,
                if storage.truncated {
                    " · scan limit reached"
                } else {
                    ""
                }
            ),
            Style::default().fg(ORANGE),
        ));
    }
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .title(" INSPECTOR ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(ORANGE)),
        ),
        area,
    );
}

fn storage_detail_line(label: &'static str, value: String, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<7}"), Style::default().fg(DIM)),
        Span::styled(value, Style::default().fg(color).bold()),
    ])
}

fn storage_columns(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(72),
            Constraint::Length(1),
            Constraint::Percentage(28),
        ])
        .split(area)
}

fn storage_treemap(entries: &[nekohub_core::StorageEntry], area: Rect) -> Vec<(usize, Rect)> {
    let items = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| entry.allocated_bytes > 0)
        .map(|(index, entry)| (index, entry.allocated_bytes))
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    split_treemap(&items, area, &mut output);
    output
}

fn split_treemap(items: &[(usize, u64)], area: Rect, output: &mut Vec<(usize, Rect)>) {
    if items.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }
    if items.len() == 1 || area.width < 4 || area.height < 3 {
        output.push((items[0].0, area));
        return;
    }
    let total = items.iter().map(|(_, size)| size).sum::<u64>().max(1);
    let mut first_total = 0_u64;
    let split = items
        .iter()
        .take(items.len() - 1)
        .position(|(_, size)| {
            first_total = first_total.saturating_add(*size);
            first_total >= total / 2
        })
        .map_or(items.len() / 2, |position| position + 1)
        .max(1);
    first_total = items[..split].iter().map(|(_, size)| size).sum();
    if area.width.saturating_mul(2) >= area.height {
        let extent = proportional_extent(area.width, first_total, total);
        split_treemap(
            &items[..split],
            Rect::new(area.x, area.y, extent, area.height),
            output,
        );
        split_treemap(
            &items[split..],
            Rect::new(
                area.x + extent,
                area.y,
                area.width.saturating_sub(extent),
                area.height,
            ),
            output,
        );
    } else {
        let extent = proportional_extent(area.height, first_total, total);
        split_treemap(
            &items[..split],
            Rect::new(area.x, area.y, area.width, extent),
            output,
        );
        split_treemap(
            &items[split..],
            Rect::new(
                area.x,
                area.y + extent,
                area.width,
                area.height.saturating_sub(extent),
            ),
            output,
        );
    }
}

fn proportional_extent(extent: u16, part: u64, total: u64) -> u16 {
    let value = (u64::from(extent) * part / total)
        .try_into()
        .unwrap_or(extent);
    value.clamp(1, extent.saturating_sub(1).max(1))
}

fn storage_entry_at(area: Rect, app: &App, x: u16, y: u16) -> Option<usize> {
    let storage = app.storage_snapshot.as_ref()?;
    let content = chrome_content(area);
    let body = Rect::new(
        content.x + 31,
        content.y + 3,
        content.width.saturating_sub(31),
        content.height.saturating_sub(3),
    );
    let storage_area = animated_area(body, app, 2, 4);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Min(12),
        ])
        .split(storage_area);
    let map_column = storage_columns(rows[2])[0];
    let map_area = Block::default().borders(Borders::ALL).inner(map_column);
    storage_treemap(&storage.entries, map_area)
        .into_iter()
        .find_map(|(index, rectangle)| {
            (x >= rectangle.x
                && x < rectangle.right()
                && y >= rectangle.y
                && y < rectangle.bottom())
            .then_some(index)
        })
}

fn render_monitor_sidebar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let sections = [
        ("Overview", AMBER),
        ("Processes", CYAN),
        ("Network", GREEN),
        ("Storage", ORANGE),
        ("Services", Color::Rgb(164, 143, 214)),
        ("Containers", CYAN),
        ("Logs", Color::Gray),
        ("Terminal", AMBER),
    ];
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            std::iter::once(Constraint::Length(5))
                .chain(std::iter::repeat_n(Constraint::Length(3), sections.len()))
                .collect::<Vec<_>>(),
        )
        .split(area);
    let host = app.selected();
    let hostname = host
        .and_then(|host| {
            host.snapshot
                .as_ref()
                .map(|sample| sample.hostname.as_str())
        })
        .or_else(|| host.map(|host| host.target.display_name.as_str()))
        .unwrap_or(app.local_name.as_str());
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("MACHINE", Style::default().fg(DIM).bold()),
            Line::styled(clipped(hostname, 24), Style::default().fg(TEXT).bold()),
            Line::styled(
                if host.is_some_and(HostState::is_online) {
                    "● agent connected"
                } else {
                    "◌ collecting metrics"
                },
                Style::default().fg(if host.is_some_and(HostState::is_online) {
                    GREEN
                } else {
                    ORANGE
                }),
            ),
        ])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(LINE)),
        ),
        rows[0],
    );
    for (index, (label, color)) in sections.into_iter().enumerate() {
        let selected = app.monitor_selected == index;
        frame.render_widget(
            Paragraph::new(format!("{} {label}", if selected { "›" } else { " " }))
                .style(if selected {
                    Style::default().fg(INK).bg(color).bold()
                } else {
                    Style::default().fg(Color::Gray)
                })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(if selected { color } else { DIM })),
                ),
            rows[index + 1],
        );
    }
}

fn render_cpu_history(frame: &mut Frame<'_>, area: Rect, host: &HostState) {
    let snapshot = host.snapshot.as_ref();
    let value = snapshot.and_then(|sample| sample.cpu_percent);
    let display = value.map_or_else(|| "--".into(), |value| format!("{value:.0}%"));
    let detail = snapshot.map_or_else(
        || "building a live baseline".into(),
        |sample| {
            format!(
                "load  {:.2}  {:.2}  {:.2}",
                sample.load[0], sample.load[1], sample.load[2]
            )
        },
    );
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" CPU  ", Style::default().fg(DIM).bold()),
            Span::styled(display, Style::default().fg(AMBER).bold()),
        ]))
        .title_bottom(Line::styled(
            format!(" {detail} "),
            Style::default().fg(DIM),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(AMBER));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(7), Constraint::Min(1)])
        .split(inner);
    let span = columns[0].height.saturating_sub(1);
    let cpu_labels = [
        (0, "100%".to_owned()),
        (span / 4, "75%".to_owned()),
        (span / 2, "50%".to_owned()),
        (span * 3 / 4, "25%".to_owned()),
        (span, "0%".to_owned()),
    ];
    render_y_axis(frame, columns[0], &cpu_labels, AMBER);
    let data: Vec<_> = host.cpu_history.iter().copied().collect();
    if data.is_empty() {
        frame.render_widget(
            Paragraph::new("Waiting for the first CPU samples")
                .style(Style::default().fg(DIM))
                .alignment(Alignment::Center),
            columns[1],
        );
    } else {
        frame.render_widget(
            Sparkline::default()
                .data(&data)
                .max(100)
                .style(Style::default().fg(AMBER)),
            columns[1],
        );
    }
}

fn render_network_history(frame: &mut Frame<'_>, area: Rect, host: &HostState) {
    let (receive, transmit) = host.snapshot.as_ref().map_or_else(
        || ("--".into(), "--".into()),
        |sample| {
            (
                format!("{}/s", bytes(sample.network.read_per_sec as u64)),
                format!("{}/s", bytes(sample.network.write_per_sec as u64)),
            )
        },
    );
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" NETWORK  ", Style::default().fg(DIM).bold()),
            Span::styled(format!("↓ {receive}"), Style::default().fg(CYAN).bold()),
            Span::styled("    ", Style::default()),
            Span::styled(format!("↑ {transmit}"), Style::default().fg(GREEN).bold()),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(LINE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
        .split(inner);
    let rx: Vec<_> = host.network_rx_history.iter().copied().collect();
    let tx: Vec<_> = host.network_tx_history.iter().copied().collect();
    if rx.is_empty() {
        frame.render_widget(
            Paragraph::new("Waiting for network activity")
                .style(Style::default().fg(DIM))
                .alignment(Alignment::Center),
            inner,
        );
        return;
    }
    render_network_lane(frame, rows[0], &rx, "↓", CYAN);
    render_network_lane(frame, rows[1], &tx, "↑", GREEN);
}

fn render_network_lane(
    frame: &mut Frame<'_>,
    area: Rect,
    data: &[u64],
    direction: &str,
    color: Color,
) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(11), Constraint::Min(1)])
        .split(area);
    let maximum = chart_ceiling(data.iter().copied().max().unwrap_or_default());
    let span = columns[0].height.saturating_sub(1);
    let labels = [
        (0, format!("{direction}{}", bytes(maximum))),
        (span, "0".to_owned()),
    ];
    render_y_axis(frame, columns[0], &labels, color);
    frame.render_widget(
        Sparkline::default()
            .data(data)
            .max(maximum)
            .style(Style::default().fg(color)),
        columns[1],
    );
}

fn render_y_axis(frame: &mut Frame<'_>, area: Rect, labels: &[(u16, String)], color: Color) {
    if area.width < 2 || area.height == 0 {
        return;
    }
    let guide = format!("{}│", " ".repeat(usize::from(area.width.saturating_sub(1))));
    frame.render_widget(
        Paragraph::new(
            (0..area.height)
                .map(|_| Line::styled(guide.clone(), Style::default().fg(LINE)))
                .collect::<Vec<_>>(),
        ),
        area,
    );
    let label_width = usize::from(area.width.saturating_sub(2));
    for (offset, label) in labels {
        frame.render_widget(
            Paragraph::new(format!("{label:>label_width$} ┤")).style(Style::default().fg(color)),
            Rect::new(
                area.x,
                area.y + (*offset).min(area.height - 1),
                area.width,
                1,
            ),
        );
    }
}

fn chart_ceiling(value: u64) -> u64 {
    value.max(1).checked_next_power_of_two().unwrap_or(u64::MAX)
}

fn render_resource_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    value: Option<f64>,
    detail: String,
    color: Color,
) {
    let percent = value.unwrap_or_default().clamp(0.0, 100.0);
    let display = value.map_or_else(|| "--".into(), |value| format!("{value:.0}%"));
    let block = Block::default()
        .title(Line::styled(
            format!(" {label} "),
            Style::default().fg(DIM).bold(),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(LINE));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(display).style(Style::default().fg(color).bold()),
        rows[0],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(color).bg(SURFACE))
            .ratio(percent / 100.0)
            .label(""),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(detail).style(Style::default().fg(Color::Gray)),
        rows[2],
    );
}

fn render_system_pulse(frame: &mut Frame<'_>, area: Rect, host: &HostState) {
    let lines = host.snapshot.as_ref().map_or_else(
        || {
            vec![
                Line::styled("Collecting system identity", Style::default().fg(ORANGE)),
                Line::styled(
                    "The first sample will appear here.",
                    Style::default().fg(DIM),
                ),
            ]
        },
        |sample| {
            vec![
                Line::from(vec![
                    Span::styled("uptime   ", Style::default().fg(DIM)),
                    Span::styled(
                        human_duration(Duration::from_secs(sample.uptime_secs)),
                        Style::default().fg(TEXT).bold(),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("kernel   ", Style::default().fg(DIM)),
                    Span::styled(
                        clipped(&sample.kernel, 20),
                        Style::default().fg(Color::Gray),
                    ),
                ]),
                Line::from(vec![
                    Span::styled("agent    ", Style::default().fg(DIM)),
                    Span::styled("● healthy", Style::default().fg(GREEN)),
                ]),
            ]
        },
    );
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .title(Line::styled(
                    " SYSTEM PULSE ",
                    Style::default().fg(DIM).bold(),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(LINE)),
        ),
        area,
    );
}

fn render_monitor_placeholder(frame: &mut Frame<'_>, area: Rect, app: &App, host: &HostState) {
    let sections = [
        ("Overview", "Live health and resource history", AMBER),
        (
            "Processes",
            "Inspect, sort and act on running processes",
            CYAN,
        ),
        (
            "Network",
            "Interfaces, throughput and active connections",
            GREEN,
        ),
        ("Storage", "Filesystems, devices and disk activity", ORANGE),
        (
            "Services",
            "systemd and OpenRC services for this machine",
            Color::Rgb(164, 143, 214),
        ),
        ("Containers", "Docker and Podman workloads", CYAN),
        ("Logs", "Search and follow machine logs", Color::Gray),
        (
            "Terminal",
            "Open an interactive SSH shell inside nekoHub",
            AMBER,
        ),
    ];
    let (title, detail, color) = sections[app.monitor_selected.min(sections.len() - 1)];
    let hostname = host
        .snapshot
        .as_ref()
        .map_or(host.target.display_name.as_str(), |sample| {
            sample.hostname.as_str()
        });
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(title, Style::default().fg(color).bold()),
                Span::styled(format!("    {hostname}"), Style::default().fg(DIM)),
            ]),
            Line::from(""),
            Line::styled(detail, Style::default().fg(Color::Gray)),
            Line::from(""),
            Line::styled(
                "This section is already part of the navigation model and will receive live data next.",
                Style::default().fg(DIM),
            ),
        ])
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .title(" MACHINE VIEW ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(color)),
        ),
        area,
    );
}

fn render_terminal_panel(frame: &mut Frame<'_>, area: Rect, app: &App, host: &HostState) {
    let block = Block::default()
        .title(Line::from(vec![
            Span::styled(" TERMINAL  ", Style::default().fg(AMBER).bold()),
            Span::styled(host.target.display_name.as_str(), Style::default().fg(DIM)),
            Span::styled(" ", Style::default()),
        ]))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(AMBER));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    match &app.terminal.phase {
        TerminalPhase::Idle => {
            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::styled("Interactive SSH terminal", Style::default().fg(TEXT).bold()),
                    Line::from(""),
                    Line::styled(
                        format!("Connect securely to {}", host.target.alias),
                        Style::default().fg(Color::Gray),
                    ),
                    Line::styled(
                        "The password stays in memory only while the connection starts.",
                        Style::default().fg(DIM),
                    ),
                    Line::from(""),
                    Line::styled("Press Enter to continue", Style::default().fg(AMBER).bold()),
                ])
                .alignment(Alignment::Center),
                vertically_centered(inner, 7),
            );
        }
        TerminalPhase::Password => render_terminal_password(frame, inner, app),
        TerminalPhase::Booting => render_terminal_boot(frame, inner, app),
        TerminalPhase::Connected => render_terminal_screen(frame, inner, app),
        TerminalPhase::Closed(status) => {
            render_terminal_screen(frame, inner, app);
            render_terminal_status(frame, inner, status, GREEN);
        }
        TerminalPhase::Error(message) => {
            render_terminal_screen(frame, inner, app);
            render_terminal_status(frame, inner, message, RED);
        }
    }
}

fn render_terminal_password(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.saturating_sub(4).clamp(20, 66).min(area.width);
    let height = 9_u16.min(area.height);
    let prompt = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, prompt);
    let prompt_block = Block::default()
        .title(" SSH AUTHENTICATION ")
        .title_bottom(Line::from(" Enter connect  ·  Esc cancel ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(AMBER));
    let prompt_inner = prompt_block.inner(prompt);
    frame.render_widget(prompt_block, prompt);
    let masked = if app.terminal.password.is_empty() {
        " ".to_owned()
    } else {
        "•".repeat(app.terminal.password.chars().count())
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                format!("Connect to {}", app.terminal.target),
                Style::default().fg(TEXT).bold(),
            ),
            Line::styled(
                if app.terminal.password_is_saved() {
                    "Saved password loaded. Backspace to replace it."
                } else {
                    "Leave empty when your SSH key already works."
                },
                Style::default().fg(DIM),
            ),
            Line::from(""),
            Line::styled("SSH password", Style::default().fg(Color::Gray)),
            Line::styled(masked, Style::default().fg(AMBER).bold()),
        ]),
        prompt_inner,
    );
}

fn render_terminal_password_save(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.saturating_sub(4).clamp(30, 66).min(area.width);
    let height = 11_u16.min(area.height);
    let prompt = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, prompt);
    let block = Block::default()
        .title(" SAVE SSH PASSWORD? ")
        .title_bottom(Line::from(" ←→ choose  ·  Enter confirm  ·  Esc back ").centered())
        .borders(Borders::ALL)
        .border_type(BorderType::Thick)
        .border_style(Style::default().fg(AMBER))
        .style(canvas_style(app));
    let inner = block.inner(prompt);
    frame.render_widget(block, prompt);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(3),
            Constraint::Min(1),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                format!("Remember the password for {}?", app.terminal.target),
                Style::default().fg(TEXT).bold(),
            ),
            Line::from(""),
            Line::styled(
                "It will be reused automatically on future terminal connections.",
                Style::default().fg(Color::Gray),
            ),
            Line::styled(
                "You can remove it later in Settings > Passwords & keys.",
                Style::default().fg(DIM),
            ),
        ])
        .alignment(Alignment::Center),
        rows[0],
    );
    let buttons = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);
    for (index, label) in ["Yes, save", "No, only this time"].iter().enumerate() {
        let selected = app.terminal_save_selected == index;
        frame.render_widget(
            Paragraph::new(*label)
                .alignment(Alignment::Center)
                .style(if selected {
                    Style::default().fg(INK).bg(AMBER).bold()
                } else {
                    Style::default().fg(Color::Gray)
                })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(if selected { AMBER } else { LINE })),
                ),
            buttons[index],
        );
    }
}

fn render_terminal_boot(frame: &mut Frame<'_>, area: Rect, app: &App) {
    const STEPS: [&str; 4] = [
        "initializing secure console",
        "opening pseudo-terminal",
        "negotiating SSH session",
        "attaching remote shell",
    ];
    let step = app.terminal.boot_step(app.animation_tick);
    let mut lines = vec![
        Line::styled(
            " /\\        __        __ __     __ /\\",
            Style::default().fg(AMBER).bold(),
        ),
        Line::styled(
            "  ___  ___ / /_____  / // /_ __/ /",
            Style::default().fg(TEXT).bold(),
        ),
        Line::styled(
            " / _ \\/ -_)  '_/ _ \\/ _  / // / _ \\",
            Style::default().fg(Color::Gray),
        ),
        Line::styled(
            "/_//_/\\__/_/\\_\\\\___/_//_/\\_,_/_.__/",
            Style::default().fg(AMBER),
        ),
        Line::from(""),
    ];
    for (index, label) in STEPS.into_iter().enumerate() {
        let (marker, color) = match index.cmp(&step) {
            std::cmp::Ordering::Less => ("●", GREEN),
            std::cmp::Ordering::Equal => ("◆", AMBER),
            std::cmp::Ordering::Greater => ("·", DIM),
        };
        lines.push(Line::from(vec![
            Span::styled(format!("{marker} "), Style::default().fg(color).bold()),
            Span::styled(label, Style::default().fg(color)),
        ]));
    }
    frame.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center),
        vertically_centered(area, 9),
    );
}

fn render_terminal_screen(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let screen = app.terminal.screen();
    let (rows, cols) = screen.size();
    let visible_rows = rows.min(area.height);
    let visible_cols = cols.min(area.width);
    for row in 0..visible_rows {
        for col in 0..visible_cols {
            let Some(source) = screen.cell(row, col) else {
                continue;
            };
            if source.is_wide_continuation() {
                continue;
            }
            let foreground = terminal_color(source.fgcolor(), TEXT);
            let background = terminal_color(source.bgcolor(), SURFACE);
            let mut style = Style::default().fg(foreground).bg(background);
            if source.bold() {
                style = style.add_modifier(Modifier::BOLD);
            }
            if source.dim() {
                style = style.add_modifier(Modifier::DIM);
            }
            if source.italic() {
                style = style.add_modifier(Modifier::ITALIC);
            }
            if source.underline() {
                style = style.add_modifier(Modifier::UNDERLINED);
            }
            if source.inverse() {
                style = Style::default().fg(background).bg(foreground);
            }
            let symbol = if source.has_contents() {
                source.contents()
            } else {
                " "
            };
            if let Some(cell) = frame.buffer_mut().cell_mut((area.x + col, area.y + row)) {
                cell.set_symbol(symbol).set_style(style);
            }
        }
    }
    if !screen.hide_cursor() {
        let (row, col) = screen.cursor_position();
        if row < area.height && col < area.width {
            frame.set_cursor_position((area.x + col, area.y + row));
        }
    }
}

fn render_terminal_status(frame: &mut Frame<'_>, area: Rect, message: &str, color: Color) {
    let status = Rect::new(
        area.x,
        area.bottom().saturating_sub(2),
        area.width,
        2.min(area.height),
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(format!(" {message} "), Style::default().fg(color).bold()),
            Line::styled(
                " F10 return to machine metrics  ·  Ctrl+] alternative ",
                Style::default().fg(DIM),
            ),
        ])
        .style(Style::default().bg(SURFACE)),
        status,
    );
}

fn vertically_centered(area: Rect, height: u16) -> Rect {
    let height = height.min(area.height);
    Rect::new(
        area.x,
        area.y + area.height.saturating_sub(height) / 2,
        area.width,
        height,
    )
}

fn terminal_color(color: vt100::Color, default: Color) -> Color {
    match color {
        vt100::Color::Default => default,
        vt100::Color::Idx(index) => Color::Indexed(index),
        vt100::Color::Rgb(red, green, blue) => Color::Rgb(red, green, blue),
    }
}

fn clipped(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

#[allow(dead_code)]
fn render_soft_metric(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    value: Option<f64>,
    detail: String,
    color: Color,
) {
    let percent = value.unwrap_or_default().clamp(0.0, 100.0);
    let display = value.map_or_else(|| "--".into(), |value| format!("{value:.0}%"));
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .split(area);
    frame.render_widget(
        Paragraph::new(label).style(Style::default().fg(DIM)),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(display).style(Style::default().fg(TEXT).bold()),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(detail).style(Style::default().fg(Color::Gray)),
        rows[2],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(color).bg(SURFACE))
            .ratio(percent / 100.0)
            .label(""),
        rows[3],
    );
}

#[allow(dead_code)]
#[allow(clippy::too_many_lines)]
fn render_local_dashboard(frame: &mut Frame<'_>, area: Rect, host: Option<&HostState>) {
    let Some(host) = host else {
        frame.render_widget(
            Paragraph::new("No local machine selected.")
                .style(Style::default().fg(DIM))
                .alignment(Alignment::Center),
            area,
        );
        return;
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(8),
            Constraint::Min(7),
        ])
        .split(area);
    let state_color = if host.last_error.is_some() {
        RED
    } else if host.snapshot.is_some() {
        GREEN
    } else {
        AMBER
    };
    let hero = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM));
    let hero_inner = hero.inner(rows[0]);
    frame.render_widget(hero, rows[0]);
    let identity = host.snapshot.as_ref().map_or_else(
        || "Waiting for the first sample".to_owned(),
        |snapshot| format!("{}  ·  {}", snapshot.os, snapshot.kernel),
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    "  THIS MACHINE  ",
                    Style::default().fg(INK).bg(AMBER).bold(),
                ),
                Span::raw("  "),
                Span::styled(
                    host.snapshot
                        .as_ref()
                        .map_or(host.target.display_name.as_str(), |snapshot| {
                            snapshot.hostname.as_str()
                        }),
                    Style::default().fg(TEXT).bold(),
                ),
                Span::raw("  "),
                Span::styled(
                    if host.last_error.is_some() {
                        "○ OFFLINE"
                    } else if host.snapshot.is_some() {
                        "● HEALTHY"
                    } else {
                        "◌ CONNECTING"
                    },
                    Style::default().fg(state_color).bold(),
                ),
            ]),
            Line::from(""),
            Line::styled(format!("  {identity}"), Style::default().fg(DIM)),
        ]),
        hero_inner,
    );

    let metric_columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(34),
            Constraint::Percentage(33),
            Constraint::Percentage(33),
        ])
        .split(rows[1]);
    let snapshot = host.snapshot.as_ref();
    render_dashboard_metric(
        frame,
        metric_columns[0],
        "CPU",
        snapshot.and_then(|sample| sample.cpu_percent),
        snapshot.map_or("warming up".into(), |sample| {
            format!(
                "load {:.2} · {:.2} · {:.2}",
                sample.load[0], sample.load[1], sample.load[2]
            )
        }),
        AMBER,
    );
    render_dashboard_metric(
        frame,
        metric_columns[1],
        "MEMORY",
        snapshot.and_then(|sample| sample.memory.percent()),
        snapshot.map_or("waiting for data".into(), |sample| {
            format!(
                "{} / {}",
                bytes(sample.memory.used),
                bytes(sample.memory.total)
            )
        }),
        ORANGE,
    );
    render_dashboard_metric(
        frame,
        metric_columns[2],
        "ROOT DISK",
        snapshot.and_then(|sample| sample.root_disk.percent()),
        snapshot.map_or("waiting for data".into(), |sample| {
            format!(
                "{} / {}",
                bytes(sample.root_disk.used),
                bytes(sample.root_disk.total)
            )
        }),
        GREEN,
    );

    let lower = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[2]);
    let (receive, transmit, uptime) = snapshot.map_or_else(
        || ("--".into(), "--".into(), "--".into()),
        |sample| {
            (
                format!("{}/s", bytes(sample.network.read_per_sec as u64)),
                format!("{}/s", bytes(sample.network.write_per_sec as u64)),
                human_duration(Duration::from_secs(sample.uptime_secs)),
            )
        },
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("NETWORK", Style::default().fg(CYAN).bold()),
            Line::from(""),
            Line::from(vec![
                Span::styled("↓  ", Style::default().fg(DIM)),
                Span::styled(receive, Style::default().fg(TEXT).bold()),
                Span::styled("     ↑  ", Style::default().fg(DIM)),
                Span::styled(transmit, Style::default().fg(TEXT).bold()),
            ]),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(DIM)),
        ),
        lower[0],
    );
    let history_width = usize::from(lower[1].width.saturating_sub(6)).clamp(8, 50);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("ACTIVITY", Style::default().fg(ORANGE).bold()),
                Span::styled(format!("   uptime {uptime}"), Style::default().fg(DIM)),
            ]),
            Line::from(""),
            Line::styled(
                sparkline_text(&host.cpu_history, history_width),
                Style::default().fg(AMBER),
            ),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(DIM)),
        ),
        lower[1],
    );
}

fn render_dashboard_metric(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    value: Option<f64>,
    detail: String,
    color: Color,
) {
    let percent = value.unwrap_or_default().clamp(0.0, 100.0);
    let display = value.map_or_else(|| "--".into(), |value| format!("{value:.1}%"));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(DIM));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(label.to_owned()).style(Style::default().fg(DIM).bold()),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(display).style(Style::default().fg(color).bold()),
        rows[1],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(color).bg(INK))
            .ratio(percent / 100.0)
            .label(""),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(detail)
            .style(Style::default().fg(Color::Gray))
            .wrap(Wrap { trim: true }),
        rows[3],
    );
}

fn render_summary(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let snapshots: Vec<_> = app
        .hosts
        .iter()
        .filter_map(|host| host.snapshot.as_ref())
        .collect();
    let cpu_values: Vec<_> = snapshots
        .iter()
        .filter_map(|snapshot| snapshot.cpu_percent)
        .collect();
    let cpu_average = if cpu_values.is_empty() {
        None
    } else {
        Some(cpu_values.iter().sum::<f64>() / cpu_values.len() as f64)
    };
    let offline: Vec<_> = app
        .hosts
        .iter()
        .filter(|host| host.last_error.is_some())
        .map(|host| host.target.display_name.as_str())
        .collect();
    let risk = highest_risk(app);

    let mut lines = vec![
        summary_line(
            "systems",
            format!("{} of {} responding", app.online_count(), app.hosts.len()),
            if app.online_count() == app.hosts.len() {
                GREEN
            } else {
                ORANGE
            },
        ),
        summary_line(
            "load",
            cpu_average.map_or_else(
                || "warming up history".into(),
                |value| format!("{value:.1}% average"),
            ),
            AMBER,
        ),
    ];
    if offline.is_empty() {
        lines.push(summary_line("reach", "no unavailable hosts".into(), GREEN));
    } else {
        lines.push(summary_line("reach", offline.join(", "), RED));
    }
    if let Some((host, metric, value)) = risk {
        lines.push(summary_line(
            "risk",
            format!("{} · {} {:.0}%", host.target.display_name, metric, value),
            metric_color(value),
        ));
    } else {
        lines.push(summary_line("risk", "collecting baseline".into(), DIM));
    }
    if area.height >= 8 {
        lines.push(summary_line(
            "next",
            "select an outlier and press Enter".into(),
            Color::Gray,
        ));
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .title(" FLEET OVERVIEW ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(DIM)),
            )
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn summary_line(label: &'static str, value: String, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<10}"), Style::default().fg(DIM)),
        Span::styled(value, Style::default().fg(color).bold()),
    ])
}

fn highest_risk(app: &App) -> Option<(&HostState, &'static str, f64)> {
    app.hosts
        .iter()
        .filter_map(|host| {
            let snapshot = host.snapshot.as_ref()?;
            let metrics = [
                ("cpu", snapshot.cpu_percent.unwrap_or_default()),
                ("mem", snapshot.memory.percent().unwrap_or_default()),
                ("root", snapshot.root_disk.percent().unwrap_or_default()),
            ];
            let (name, value) = metrics
                .into_iter()
                .max_by(|left, right| left.1.total_cmp(&right.1))?;
            Some((host, name, value))
        })
        .max_by(|left, right| left.2.total_cmp(&right.2))
}

fn render_host_grid(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let columns = if area.width >= 120 {
        3
    } else if area.width >= 76 {
        2
    } else {
        1
    };
    let card_height = 8u16;
    let visible_rows = usize::from((area.height / card_height).max(1));
    let capacity = columns * visible_rows;
    let page = app.selected / capacity;
    let start = page * capacity;
    let visible = app.hosts.iter().enumerate().skip(start).take(capacity);

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Ratio(1, visible_rows as u32);
            visible_rows
        ])
        .split(area);
    for (offset, (index, host)) in visible.enumerate() {
        let row = offset / columns;
        let column = offset % columns;
        let horizontal = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Ratio(1, columns as u32); columns])
            .split(vertical[row]);
        render_host_card(frame, horizontal[column], host, index == app.selected);
    }
}

fn render_host_card(frame: &mut Frame<'_>, area: Rect, host: &HostState, selected: bool) {
    let (state, state_color) = if host.last_error.is_some() {
        ("OFFLINE", RED)
    } else if host.snapshot.is_some() {
        ("NORMAL", GREEN)
    } else {
        ("COLETANDO", DIM)
    };
    let title = Line::from(vec![
        Span::styled(
            format!(
                " {}{} ",
                if selected { "▸ " } else { "" },
                host.target.display_name
            ),
            Style::default().fg(AMBER).bold(),
        ),
        Span::styled(
            format!("· {state} "),
            Style::default().fg(state_color).bold(),
        ),
    ]);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(if selected {
            BorderType::Thick
        } else {
            BorderType::Plain
        })
        .border_style(Style::default().fg(state_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(snapshot) = &host.snapshot else {
        let message = host
            .last_error
            .as_deref()
            .unwrap_or("waiting for the first sample");
        frame.render_widget(
            Paragraph::new(message).style(Style::default().fg(if host.last_error.is_some() {
                RED
            } else {
                DIM
            })),
            inner,
        );
        return;
    };
    let width = usize::from(inner.width.saturating_sub(12)).clamp(6, 20);
    let lines = vec![
        metric_line("CPU", snapshot.cpu_percent, width),
        metric_line("MEM", snapshot.memory.percent(), width),
        metric_line("DSK", snapshot.root_disk.percent(), width),
        Line::from(vec![
            Span::styled("load  ", Style::default().fg(DIM)),
            Span::styled(
                format!(
                    "{:.2} {:.2} {:.2}",
                    snapshot.load[0], snapshot.load[1], snapshot.load[2]
                ),
                Style::default().fg(AMBER),
            ),
        ]),
        Line::from(vec![
            Span::styled("net   ", Style::default().fg(DIM)),
            Span::styled(
                format!(
                    "↓{}/s  ↑{}/s",
                    bytes(snapshot.network.read_per_sec as u64),
                    bytes(snapshot.network.write_per_sec as u64)
                ),
                Style::default().fg(CYAN),
            ),
        ]),
        Line::from(vec![
            Span::styled("up    ", Style::default().fg(DIM)),
            Span::styled(
                human_duration(Duration::from_secs(snapshot.uptime_secs)),
                Style::default().fg(AMBER),
            ),
            Span::styled(
                format!("   {}ms", snapshot.latency_ms),
                Style::default().fg(DIM),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines), inner);
}

fn metric_line(label: &'static str, value: Option<f64>, width: usize) -> Line<'static> {
    let value = value.unwrap_or_default().clamp(0.0, 100.0);
    let filled = ((value / 100.0) * width as f64).round() as usize;
    Line::from(vec![
        Span::styled(format!("{label:<5} "), Style::default().fg(DIM)),
        Span::styled("█".repeat(filled), Style::default().fg(metric_color(value))),
        Span::styled(
            "░".repeat(width.saturating_sub(filled)),
            Style::default().fg(DIM),
        ),
        Span::styled(
            format!(" {value:>4.0}%"),
            Style::default().fg(metric_color(value)),
        ),
    ])
}

fn render_detail(frame: &mut Frame<'_>, area: Rect, host: Option<&HostState>) {
    let Some(host) = host else {
        frame.render_widget(Paragraph::new("No hosts discovered."), area);
        return;
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(13), Constraint::Min(7)])
        .split(area);
    render_detail_panel(frame, rows[0], host);
    render_metric_table(frame, rows[1], host);
}

fn render_detail_panel(frame: &mut Frame<'_>, area: Rect, host: &HostState) {
    let state_color = if host.last_error.is_some() {
        RED
    } else if host.snapshot.is_some() {
        GREEN
    } else {
        DIM
    };
    let title = Line::from(vec![
        Span::styled(
            format!(" {} ", host.target.display_name),
            Style::default().fg(AMBER).bold(),
        ),
        Span::styled(
            if host.last_error.is_some() {
                "· OFFLINE "
            } else {
                "· HOST DETAIL "
            },
            Style::default().fg(state_color).bold(),
        ),
    ]);
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(state_color));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(snapshot) = &host.snapshot else {
        frame.render_widget(
            Paragraph::new(host.last_error.as_deref().unwrap_or("collecting"))
                .style(Style::default().fg(state_color)),
            inner,
        );
        return;
    };
    let bar_width = usize::from(inner.width.saturating_sub(18)).clamp(10, 42);
    let history = sparkline_text(&host.cpu_history, bar_width);
    let lines = vec![
        Line::from(vec![
            Span::styled("system  ", Style::default().fg(DIM)),
            Span::styled(
                format!("{} · {}", snapshot.os, snapshot.kernel),
                Style::default().fg(Color::Gray),
            ),
        ]),
        Line::from(""),
        metric_line("CPU", snapshot.cpu_percent, bar_width),
        metric_line("MEM", snapshot.memory.percent(), bar_width),
        metric_line("ROOT", snapshot.root_disk.percent(), bar_width),
        Line::from(vec![
            Span::styled("cpu    ", Style::default().fg(DIM)),
            Span::styled(history, Style::default().fg(AMBER)),
        ]),
        Line::from(vec![
            Span::styled("load   ", Style::default().fg(DIM)),
            Span::styled(
                format!(
                    "{:.2}  {:.2}  {:.2}",
                    snapshot.load[0], snapshot.load[1], snapshot.load[2]
                ),
                Style::default().fg(AMBER),
            ),
        ]),
        Line::from(vec![
            Span::styled("up     ", Style::default().fg(DIM)),
            Span::styled(
                human_duration(Duration::from_secs(snapshot.uptime_secs)),
                Style::default().fg(AMBER),
            ),
            Span::styled(
                format!("   latency {}ms", snapshot.latency_ms),
                Style::default().fg(CYAN),
            ),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
}

fn render_metric_table(frame: &mut Frame<'_>, area: Rect, host: &HostState) {
    let rows = if let Some(snapshot) = host.snapshot.as_ref() {
        vec![
            Row::new(vec![
                Cell::from("memory"),
                Cell::from(bytes(snapshot.memory.used)),
                Cell::from(bytes(snapshot.memory.total)),
                Cell::from(format!(
                    "{:.1}%",
                    snapshot.memory.percent().unwrap_or_default()
                )),
            ]),
            Row::new(vec![
                Cell::from("root filesystem"),
                Cell::from(bytes(snapshot.root_disk.used)),
                Cell::from(bytes(snapshot.root_disk.total)),
                Cell::from(format!(
                    "{:.1}%",
                    snapshot.root_disk.percent().unwrap_or_default()
                )),
            ]),
            Row::new(vec![
                Cell::from("network receive"),
                Cell::from(format!("{}/s", bytes(snapshot.network.read_per_sec as u64))),
                Cell::from(""),
                Cell::from(""),
            ]),
            Row::new(vec![
                Cell::from("network transmit"),
                Cell::from(format!(
                    "{}/s",
                    bytes(snapshot.network.write_per_sec as u64)
                )),
                Cell::from(""),
                Cell::from(""),
            ]),
        ]
    } else {
        Vec::new()
    };
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(35),
            Constraint::Percentage(22),
            Constraint::Percentage(22),
            Constraint::Percentage(21),
        ],
    )
    .header(
        Row::new(["METRIC", "USED / RATE", "TOTAL", "USE"])
            .style(Style::default().fg(AMBER).bold()),
    )
    .column_spacing(2)
    .style(Style::default().fg(Color::Gray))
    .block(
        Block::default()
            .title(" CURRENT SAMPLE ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)),
    );
    frame.render_widget(table, area);
}

fn sparkline_text(values: &std::collections::VecDeque<u64>, width: usize) -> String {
    const BLOCKS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let mut output = "░".repeat(width.saturating_sub(values.len().min(width)));
    for value in values
        .iter()
        .rev()
        .take(width)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        let level = (((*value).min(100) as f64 / 100.0).sqrt() * 8.0).round() as usize;
        output.push(BLOCKS[level.max(1)]);
    }
    output
}

fn metric_color(percent: f64) -> Color {
    if percent >= 90.0 {
        ORANGE
    } else if percent >= 75.0 {
        AMBER
    } else {
        GREEN
    }
}

fn bytes(value: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = value as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0}{}", UNITS[unit])
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

fn human_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds >= 86_400 {
        format!("{}d {:02}h", seconds / 86_400, (seconds % 86_400) / 3_600)
    } else if seconds >= 3_600 {
        format!("{}h", seconds / 3_600)
    } else if seconds >= 60 {
        format!("{}m", seconds / 60)
    } else {
        format!("{seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, MouseEvent};

    #[test]
    fn formats_sizes() {
        assert_eq!(bytes(1024), "1.0KiB");
        assert_eq!(bytes(0), "0B");
        assert_eq!(chart_ceiling(0), 1);
        assert_eq!(chart_ceiling(65), 128);
        assert_eq!(chart_ceiling(1024), 1024);
    }

    #[test]
    fn mouse_clicks_select_navigation_and_monitor_sections() {
        let area = Rect::new(0, 0, 106, 40);
        let mut app = App::monitoring(vec![nekohub_core::HostTarget::from_alias("local")]);
        let nav = mouse_key(
            &mut app,
            area,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 70,
                row: 3,
                modifiers: KeyModifiers::NONE,
            },
        )
        .unwrap();
        assert_eq!(nav.code, KeyCode::Char('2'));
        assert_eq!(nav.kind, KeyEventKind::Press);

        let section = mouse_key(
            &mut app,
            area,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: 21,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(section.is_none());
        assert_eq!(app.monitor_selected, 1);

        let overview = mouse_key(
            &mut app,
            area,
            MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 5,
                row: 19,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert!(overview.is_none());
        assert_eq!(app.monitor_selected, 0);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn render_smoke_test_at_compact_and_wide_sizes() {
        use ratatui::{Terminal, backend::TestBackend};

        for (width, height) in [(80, 24), (106, 40), (160, 45)] {
            let mut app = App::new(
                (0..8)
                    .map(|index| nekohub_core::HostTarget::from_alias(format!("host-{index}")))
                    .collect(),
                vec![
                    crate::groups::MachineGroup::empty("Production".into()),
                    crate::groups::MachineGroup::empty("Home lab".into()),
                ],
            );
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.open_agent_confirmation();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.start_agent_setup();
            app.update_agent_setup(72, "Validating CPU, memory, disk and network".into());
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.fail_agent_setup("Could not reach the local agent".into());
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.open_home();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.home_selected = 0;
            app.open_group();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.close_group();
            app.begin_group_creation();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.cancel_group_creation();
            app.open_settings();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.settings_selected = 1;
            app.enter_settings_content();
            app.settings_item_selected = 4;
            app.theme = Theme::Purple;
            app.font_profile = FontProfile::Ascii;
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.begin_theme_import();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.cancel_theme_import();
            app.open_settings();
            app.settings_selected = 2;
            app.enter_settings_content();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.begin_credential_key_edit();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.cancel_credential_key_edit();
            app.open_remote_picker();
            app.remote_selected = 0;
            app.begin_machine_alias();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.cancel_machine_alias();
            app.begin_group_assignment();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.cancel_group_assignment();
            app.remote_selected = app.remote_item_count() - 1;
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.begin_remote_install();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.start_remote_install_progress();
            app.update_remote_install(55, "Repository configured".into());
            app.push_remote_install_log("Installing nekohub-agent".into());
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.complete_remote_install("ops@server");
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.close_remote_install_progress();
            app.remote_selected = app
                .remote_hosts
                .iter()
                .position(|host| host.alias == "ops@server")
                .unwrap()
                + 1;
            app.begin_remote_connect();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.cancel_remote_connect();
            app.begin_remote_uninstall();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.cancel_remote_uninstall();
            app.start_monitoring(nekohub_core::HostTarget::from_alias("local"));
            app.apply_snapshot(nekohub_core::HostSnapshot {
                host_id: "local".into(),
                agent_version: "0.11.0".into(),
                collected_at: std::time::SystemTime::now(),
                latency_ms: 2,
                hostname: "demo".into(),
                os: "Linux".into(),
                kernel: "6.x".into(),
                uptime_secs: 300,
                cpu_percent: Some(30.0),
                memory: nekohub_core::Usage {
                    used: 40,
                    total: 100,
                },
                root_disk: nekohub_core::Usage {
                    used: 25,
                    total: 100,
                },
                load: [0.2, 0.3, 0.4],
                network: nekohub_core::Throughput::default(),
                processes: vec![nekohub_core::ProcessSnapshot {
                    pid: 42,
                    name: "worker".into(),
                    command: "worker --serve".into(),
                    state: "R".into(),
                    cpu_percent: 12.5,
                    memory_bytes: 64 * 1024 * 1024,
                }],
                containers: vec![nekohub_core::ContainerSnapshot {
                    id: "abc123".into(),
                    name: "api".into(),
                    engine: "docker".into(),
                    state: "running".into(),
                    cpu_percent: 8.0,
                    memory_used_bytes: 128 * 1024 * 1024,
                    memory_limit_bytes: 512 * 1024 * 1024,
                    network_rx_bytes: 1024,
                    network_tx_bytes: 2048,
                    block_read_bytes: 4096,
                    block_write_bytes: 8192,
                    pids: 7,
                }],
            });
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.monitor_selected = 1;
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.monitor_selected = 5;
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.monitor_selected = 0;
            assert!(!app.register_agent_update_quote());
            assert!(app.register_agent_update_quote());
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.start_agent_update_progress();
            app.update_remote_install(72, "Installing updated agent".into());
            app.push_remote_install_log("Package downloaded".into());
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.complete_agent_update();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.close_agent_update_progress();
            app.monitor_selected = 7;
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.begin_terminal_password();
            app.push_terminal_password_character('x');
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.begin_terminal_password_save();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            let request = app.start_terminal(
                height.saturating_sub(16).max(2),
                width.saturating_sub(35).max(10),
            );
            app.terminal
                .process_output(request.generation, b"\x1b[32mconnected\x1b[0m\r\nserver$ ");
            for _ in 0..42 {
                app.advance_animation();
            }
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.terminal.finish(request.generation, "Success".into());
            terminal.draw(|frame| render(frame, &app)).unwrap();
        }
    }
}
