#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::time::Duration;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Cell, Clear, Gauge, Paragraph, Row, Table, Wrap},
};

use crate::app::{App, HostState, View};

const AMBER: Color = Color::Rgb(103, 199, 255);
const ORANGE: Color = Color::Rgb(181, 140, 255);
const DIM: Color = Color::Rgb(70, 91, 111);
const GREEN: Color = Color::Rgb(102, 226, 178);
const RED: Color = Color::Rgb(255, 74, 74);
const CYAN: Color = Color::Rgb(112, 220, 232);
const INK: Color = Color::Rgb(15, 24, 33);
const TEXT: Color = Color::Rgb(214, 226, 238);

pub fn render(frame: &mut Frame<'_>, app: &App) {
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
        View::RemotePicker => {
            render_remote_picker(frame, frame.area(), app);
            return;
        }
        View::Settings => {
            render_settings(frame, frame.area(), app);
            return;
        }
        View::Overview | View::Detail => {}
    }
    let shell = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(frame.area());
    render_header(frame, shell[0], app);
    match app.view {
        View::Overview => render_overview(frame, shell[1], app),
        View::Detail => render_detail(frame, shell[1], app.selected()),
        View::Welcome
        | View::AgentConfirm
        | View::AgentSetup
        | View::Home
        | View::RemotePicker
        | View::Settings => {
            unreachable!("setup views return before shell render")
        }
    }
    render_footer(frame, shell[2], app.view);
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
    let outer = Block::default()
        .title(Line::from(vec![
            Span::styled(" nekoHub ", Style::default().fg(INK).bg(AMBER).bold()),
            Span::styled(" HOME ", Style::default().fg(DIM)),
        ]))
        .title_bottom(
            Line::from(" ↑↓ choose  ·  Enter open  ·  q quit ")
                .style(Style::default().fg(DIM))
                .centered(),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    let width = inner.width.min(78);
    let height = inner.height.min(22);
    let content = Rect::new(
        inner.x + inner.width.saturating_sub(width) / 2,
        inner.y + inner.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(4),
            Constraint::Length(4),
            Constraint::Length(1),
            Constraint::Length(4),
            Constraint::Min(1),
        ])
        .split(content);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("Choose a machine space", Style::default().fg(TEXT).bold()),
            Line::from(""),
            Line::styled(
                "Start local, open a fleet, or adjust how nekoHub behaves.",
                Style::default().fg(DIM),
            ),
        ]),
        rows[0],
    );

    let live = if app.animation_tick % 16 < 8 {
        "● READY"
    } else {
        "• READY"
    };
    render_home_item(
        frame,
        rows[1],
        "This machine",
        "Local agent · 1 host",
        live,
        app.home_selected == 0,
        GREEN,
    );
    let remote_count = app.remote_hosts.len();
    render_home_item(
        frame,
        rows[2],
        "Remote fleet",
        &format!("SSH inventory · {remote_count} discovered"),
        if remote_count == 0 {
            "EMPTY"
        } else {
            "PAIRING NEEDED"
        },
        app.home_selected == 1,
        ORANGE,
    );
    frame.render_widget(
        Paragraph::new("SYSTEM").style(Style::default().fg(DIM).bold()),
        rows[3],
    );
    render_home_item(
        frame,
        rows[4],
        "Settings",
        "Appearance, agents, data and preferences",
        "OPEN",
        app.home_selected == 2,
        CYAN,
    );
}

#[allow(clippy::too_many_arguments)]
fn render_home_item(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    detail: &str,
    status: &str,
    selected: bool,
    status_color: Color,
) {
    let border = if selected { AMBER } else { DIM };
    let title_color = if selected { TEXT } else { Color::Gray };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    if selected { "  ›  " } else { "     " },
                    Style::default().fg(AMBER).bold(),
                ),
                Span::styled(title.to_owned(), Style::default().fg(title_color).bold()),
            ]),
            Line::from(vec![
                Span::raw("     "),
                Span::styled(detail.to_owned(), Style::default().fg(DIM)),
            ]),
        ]),
        inner,
    );
    let status_width = status.chars().count() as u16 + 2;
    if status_width < inner.width {
        let status_area = Rect::new(
            inner.right().saturating_sub(status_width),
            inner.y,
            status_width,
            1,
        );
        frame.render_widget(
            Paragraph::new(status.to_owned()).style(Style::default().fg(status_color).bold()),
            status_area,
        );
    }
}

fn render_settings(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let outer = Block::default()
        .title(Line::from(vec![
            Span::styled(" nekoHub ", Style::default().fg(INK).bg(AMBER).bold()),
            Span::styled(" SETTINGS ", Style::default().fg(DIM)),
        ]))
        .title_bottom(
            Line::from(" Esc return to menu  ·  q quit ")
                .style(Style::default().fg(DIM))
                .centered(),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(DIM));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(12)])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("Settings foundation", Style::default().fg(TEXT).bold()),
            Line::from(""),
            Line::styled(
                "These sections will become configurable as the product grows.",
                Style::default().fg(DIM),
            ),
        ])
        .alignment(Alignment::Center),
        rows[0],
    );
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(columns[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(columns[1]);
    render_settings_card(
        frame,
        left[0],
        "Appearance & themes",
        "Palette, motion and community themes",
        ORANGE,
    );
    render_settings_card(
        frame,
        left[1],
        "Agents",
        "Local service and future remote pairing",
        GREEN,
    );
    render_settings_card(
        frame,
        right[0],
        "Machine groups",
        &format!("{} remote hosts discovered", app.remote_hosts.len()),
        AMBER,
    );
    render_settings_card(
        frame,
        right[1],
        "Data & integrations",
        "Prometheus, history and retention",
        CYAN,
    );
}

fn render_settings_card(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    detail: &str,
    color: Color,
) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(title.to_owned(), Style::default().fg(color).bold()),
            Line::from(""),
            Line::styled(detail.to_owned(), Style::default().fg(Color::Gray)),
            Line::from(""),
            Line::styled("COMING NEXT", Style::default().fg(DIM).bold()),
        ])
        .wrap(Wrap { trim: true })
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(DIM)),
        ),
        area,
    );
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

fn render_remote_picker(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let outer = Block::default()
        .title(Line::styled(
            " REMOTE MACHINE ",
            Style::default().fg(AMBER).bold(),
        ))
        .title_bottom(
            Line::from(" ↑↓ select  ·  Enter agent setup  ·  Esc back  ·  q quit ").centered(),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(AMBER));
    let inner = outer.inner(area);
    frame.render_widget(outer, area);
    let width = inner.width.min(64);
    let height = (app.remote_hosts.len() as u16 + 4).min(inner.height).max(7);
    let panel = Rect::new(
        inner.x + inner.width.saturating_sub(width) / 2,
        inner.y + inner.height.saturating_sub(height) / 2,
        width,
        height,
    );
    if let Some(notice) = &app.remote_notice {
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("AGENT PAIRING", Style::default().fg(AMBER).bold()),
                Line::from(""),
                Line::from(notice.as_str()),
                Line::from(""),
                Line::styled(
                    "SSH remains available for discovery and administration.",
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
    let rows = app.remote_hosts.iter().enumerate().map(|(index, host)| {
        let selected = index == app.remote_selected;
        Row::new([if selected { "›" } else { " " }, host.alias.as_str()]).style(if selected {
            Style::default().fg(INK).bg(AMBER).bold()
        } else {
            Style::default().fg(Color::Gray)
        })
    });
    let table = Table::new(rows, [Constraint::Length(2), Constraint::Min(10)])
        .header(
            Row::new(["", "DISCOVERED HOST"])
                .style(Style::default().fg(DIM).bold())
                .bottom_margin(1),
        )
        .block(
            Block::default()
                .title(" Inventory from ~/.ssh/config ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(DIM)),
        );
    frame.render_widget(table, panel);
}

fn render_header(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut line = vec![
        Span::styled(" nekoHub ", Style::default().fg(INK).bg(AMBER).bold()),
        Span::styled("  /  ", Style::default().fg(DIM)),
        Span::styled(
            app.selected()
                .map_or("MACHINES", |host| host.target.display_name.as_str()),
            Style::default().fg(TEXT).bold(),
        ),
        Span::raw("   "),
    ];
    for (label, view) in [("OVERVIEW", View::Overview), ("DETAILS", View::Detail)] {
        let style = if app.view == view {
            Style::default().fg(AMBER).bold()
        } else {
            Style::default().fg(DIM)
        };
        line.push(Span::styled(format!(" {label} "), style));
        line.push(Span::raw(" "));
    }
    let header = Paragraph::new(Line::from(line)).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(DIM)),
    );
    frame.render_widget(header, area);

    let status = if app.online_count() == app.hosts.len() && !app.hosts.is_empty() {
        "● LIVE ".to_owned()
    } else {
        "○ CONNECTING ".to_owned()
    };
    let width = status.chars().count() as u16;
    let status_area = Rect::new(area.right().saturating_sub(width + 1), area.y + 1, width, 1);
    let color = if app.online_count() == app.hosts.len() && !app.hosts.is_empty() {
        GREEN
    } else {
        ORANGE
    };
    frame.render_widget(
        Paragraph::new(status).style(Style::default().fg(color).bold()),
        status_area,
    );
}

fn render_overview(frame: &mut Frame<'_>, area: Rect, app: &App) {
    if app.hosts.len() == 1 {
        render_local_dashboard(frame, area, app.selected());
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

fn render_footer(frame: &mut Frame<'_>, area: Rect, view: View) {
    let mut spans = Vec::new();
    if view == View::Overview {
        spans.extend([
            key("Enter"),
            Span::styled(" details   ", Style::default().fg(DIM)),
            key("Esc"),
            Span::styled(" menu   ", Style::default().fg(DIM)),
        ]);
    } else {
        spans.extend([
            key("Esc"),
            Span::styled(" overview   ", Style::default().fg(DIM)),
        ]);
    }
    spans.extend([
        key("r"),
        Span::styled(" refresh   ", Style::default().fg(DIM)),
        key("q"),
        Span::styled(" quit", Style::default().fg(DIM)),
    ]);
    frame.render_widget(
        Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
        area,
    );
}

fn key(label: &'static str) -> Span<'static> {
    Span::styled(format!(" {label} "), Style::default().fg(AMBER).bold())
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

    #[test]
    fn formats_sizes() {
        assert_eq!(bytes(1024), "1.0KiB");
        assert_eq!(bytes(0), "0B");
    }

    #[test]
    fn render_smoke_test_at_compact_and_wide_sizes() {
        use ratatui::{Terminal, backend::TestBackend};

        for (width, height) in [(80, 24), (106, 40), (160, 45)] {
            let mut app = App::new(
                (0..8)
                    .map(|index| nekohub_core::HostTarget::from_alias(format!("host-{index}")))
                    .collect(),
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
            app.open_settings();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.start_monitoring(nekohub_core::HostTarget::from_alias("local"));
            terminal.draw(|frame| render(frame, &app)).unwrap();
        }
    }
}
