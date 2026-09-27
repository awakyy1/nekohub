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

use crate::app::{App, HomeFocus, HostState, View};

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
            render_tab_transition(frame, frame.area(), app);
            return;
        }
        View::CreateGroup => {
            render_home(frame, frame.area(), app);
            render_create_group(frame, frame.area(), app);
            return;
        }
        View::RemotePicker => {
            render_remote_picker(frame, frame.area(), app);
            render_tab_transition(frame, frame.area(), app);
            return;
        }
        View::Settings => {
            render_settings(frame, frame.area(), app);
            render_tab_transition(frame, frame.area(), app);
            return;
        }
        View::Overview | View::Detail => {}
    }
    let content = render_app_chrome(
        frame,
        frame.area(),
        app,
        0,
        "Esc home  ·  r refresh  ·  m machines  ·  s settings  ·  q quit",
    );
    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(10)])
        .split(content);
    render_monitoring_bar(frame, sections[0], app);
    match app.view {
        View::Overview => render_overview(frame, sections[1], app),
        View::Detail => render_detail(frame, sections[1], app.selected()),
        View::Welcome
        | View::AgentConfirm
        | View::AgentSetup
        | View::Home
        | View::CreateGroup
        | View::RemotePicker
        | View::Settings => {
            unreachable!("setup views return before shell render")
        }
    }
    render_tab_transition(frame, frame.area(), app);
}

fn render_tab_transition(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let Some((age, direction)) = app.tab_transition() else {
        return;
    };
    let revealed = area.width.saturating_mul(age as u16) / 5;
    let hidden = area.width.saturating_sub(revealed);
    if hidden == 0 {
        return;
    }
    let mask = if direction > 0 {
        Rect::new(area.x.saturating_add(revealed), area.y, hidden, area.height)
    } else {
        Rect::new(area.x, area.y, hidden, area.height)
    };
    frame.render_widget(Clear, mask);
    frame.render_widget(Block::default().style(Style::default().bg(INK)), mask);

    let edge_x = if direction > 0 {
        mask.x
    } else {
        mask.right().saturating_sub(2)
    };
    let edge = Rect::new(edge_x, area.y, 2.min(area.width), area.height);
    let edge_line = if direction > 0 {
        Line::from(vec![
            Span::styled("▓", Style::default().fg(AMBER)),
            Span::styled("░", Style::default().fg(DIM)),
        ])
    } else {
        Line::from(vec![
            Span::styled("░", Style::default().fg(DIM)),
            Span::styled("▓", Style::default().fg(AMBER)),
        ])
    };
    let scanline = std::iter::repeat_with(|| edge_line.clone())
        .take(usize::from(area.height))
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(scanline), edge);
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
        .constraints([Constraint::Min(8), Constraint::Length(1)])
        .split(content);
    render_group_grid(frame, rows[0], app);
    if let Some(notice) = app.home_notice.as_deref() {
        frame.render_widget(
            Paragraph::new(notice)
                .style(Style::default().fg(ORANGE))
                .alignment(Alignment::Center),
            rows[1],
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
        .style(Style::default().bg(INK));
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
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);
    let logo = if columns[0].width >= 40 {
        vec![
            Line::from(vec![
                Span::styled("  /\\_/\\", Style::default().fg(AMBER).bold()),
                Span::styled("    _       _  _      _", Style::default().fg(DIM)),
            ]),
            Line::styled(
                "  _ _  ___| |_____| || |_  _| |__",
                Style::default().fg(TEXT).bold(),
            ),
            Line::styled(
                " | ' \\/ -_) / / _ \\ __ | || | '_ \\",
                Style::default().fg(Color::Gray),
            ),
            Line::styled(
                " |_||_\\___|_\\_\\___/_||_|\\_,_|_.__/",
                Style::default().fg(AMBER),
            ),
        ]
    } else {
        vec![Line::from(vec![
            Span::styled(" /\\_/\\  ", Style::default().fg(AMBER).bold()),
            Span::styled("nekoHub", Style::default().fg(TEXT).bold()),
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
                    .border_style(Style::default().fg(color)),
            ),
            buttons[index],
        );
    }
}

fn render_machine_strip(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut spans = vec![
        Span::styled(" ● ", Style::default().fg(GREEN)),
        Span::styled(app.local_name.as_str(), Style::default().fg(TEXT).bold()),
    ];
    for host in &app.remote_hosts {
        spans.push(Span::styled("    ○ ", Style::default().fg(DIM)));
        spans.push(Span::styled(
            host.display_name.clone(),
            Style::default().fg(Color::Gray),
        ));
    }
    if app.remote_hosts.is_empty() {
        spans.push(Span::styled(
            "    No remote machines paired yet",
            Style::default().fg(DIM),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).block(
            Block::default()
                .title(Line::styled(
                    format!(" machines  {} ", app.remote_hosts.len() + 1),
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
            render_group_card(frame, card_area, app, index);
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
    let (title, count, state, accent) = if index == 0 {
        (
            app.local_name.clone(),
            "1 machine".to_owned(),
            "● local · ready".to_owned(),
            GREEN,
        )
    } else {
        let group = &app.machine_groups[index - 1];
        let count = group.host_ids.len();
        (
            group.name.clone(),
            format!("{count} machine{}", if count == 1 { "" } else { "s" }),
            if count == 0 {
                "○ empty group".to_owned()
            } else {
                "● group ready".to_owned()
            },
            [AMBER, CYAN, ORANGE][(index - 1) % 3],
        )
    };
    let border = if selected { accent } else { LINE };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(border))
        .style(Style::default().bg(if selected { SURFACE } else { INK }));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(" ◆  ", Style::default().fg(accent).bold()),
                Span::styled(title, Style::default().fg(TEXT).bold()),
            ]),
            Line::styled(format!("    {count}"), Style::default().fg(DIM)),
            Line::from(""),
            Line::styled(format!("    {state}"), Style::default().fg(accent)),
        ])
        .wrap(Wrap { trim: true }),
        inner,
    );
}

fn render_add_group_button(frame: &mut Frame<'_>, area: Rect, app: &App, selected: bool) {
    let width = area.width.min(16);
    let height = area.height.min(6);
    let button = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let color = if selected { ORANGE } else { DIM };
    let pulse = if selected && app.animation_tick % 6 < 3 {
        "✦"
    } else {
        "+"
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("  ┌┈┈┈┈┈┈┈┈┈┈┐", Style::default().fg(color)),
            Line::from(vec![
                Span::styled("  ┊    ", Style::default().fg(color)),
                Span::styled(pulse, Style::default().fg(ORANGE).bold()),
                Span::styled("     ┊", Style::default().fg(color)),
            ]),
            Line::styled("  └┈┈┈┈┈┈┈┈┈┈┘", Style::default().fg(color)),
            Line::styled(
                "    New group",
                Style::default()
                    .fg(if selected { TEXT } else { Color::Gray })
                    .bold(),
            ),
            Line::styled("    Enter to add", Style::default().fg(DIM)),
        ])
        .style(Style::default().bg(if selected { SURFACE } else { INK })),
        button,
    );
}

fn render_create_group(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(62);
    let height = area.height.min(14);
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
            Constraint::Length(4),
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

fn render_settings(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let inner = render_app_chrome(
        frame,
        area,
        app,
        2,
        "↑↓ select  ·  Esc home  ·  m machines  ·  q quit",
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
        (
            "Appearance & themes",
            "Palette, motion and community themes",
            ORANGE,
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
            CYAN,
        ),
    ];
    let sidebar = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(3); sections.len()])
        .split(columns[0]);
    for (index, (title, _, color)) in sections.iter().enumerate() {
        let selected = app.settings_selected == index;
        frame.render_widget(
            Paragraph::new(format!("{} {title}", if selected { "›" } else { " " }))
                .style(if selected {
                    Style::default().fg(INK).bg(*color).bold()
                } else {
                    Style::default().fg(Color::Gray)
                })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(if selected { *color } else { DIM })),
                ),
            sidebar[index],
        );
    }
    let (title, detail, color) = sections[app.settings_selected];
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
                .border_style(Style::default().fg(color)),
        ),
        columns[2],
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
    let inner = render_app_chrome(
        frame,
        area,
        app,
        1,
        "↑↓ select  ·  Enter inspect  ·  Esc home  ·  s settings  ·  q quit",
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(4), Constraint::Min(7)])
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
    let local_selected = app.remote_selected == 0;
    let local = Row::new([
        if local_selected { "›" } else { " " },
        app.local_name.as_str(),
        "Local agent",
        "Local",
        "● ready",
    ])
    .style(if local_selected {
        Style::default().fg(INK).bg(AMBER).bold()
    } else {
        Style::default().fg(Color::Gray)
    });
    let remote_rows = app.remote_hosts.iter().enumerate().map(|(index, host)| {
        let selected = index + 1 == app.remote_selected;
        Row::new([
            if selected { "›" } else { " " },
            host.display_name.as_str(),
            "SSH inventory",
            "Unassigned",
            "○ pairing needed",
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
            Constraint::Min(18),
            Constraint::Length(15),
            Constraint::Length(16),
            Constraint::Length(18),
        ],
    )
    .header(
        Row::new(["", "Machine", "Source", "Group", "Status"])
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
            Constraint::Length(24),
            Constraint::Length(2),
            Constraint::Min(40),
        ])
        .split(area);
    render_monitor_sidebar(frame, columns[0], app);
    let Some(host) = app.selected() else {
        return;
    };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Length(5),
            Constraint::Min(5),
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
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(hostname, Style::default().fg(TEXT).bold()),
                Span::styled(
                    if host.last_error.is_some() {
                        "    (-._.-) offline"
                    } else if snapshot.is_some() {
                        "    (^._.^) healthy"
                    } else {
                        "    (^o_o^) collecting"
                    },
                    Style::default().fg(state_color),
                ),
            ]),
            Line::styled(identity, Style::default().fg(DIM)),
        ])
        .block(
            Block::default()
                .borders(Borders::BOTTOM)
                .border_style(Style::default().fg(LINE)),
        ),
        rows[0],
    );

    let metrics = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .spacing(2)
        .split(rows[1]);
    render_soft_metric(
        frame,
        metrics[0],
        "Cpu",
        snapshot.and_then(|sample| sample.cpu_percent),
        snapshot.map_or("warming up".into(), |sample| {
            format!("load {:.2}", sample.load[0])
        }),
        AMBER,
    );
    render_soft_metric(
        frame,
        metrics[1],
        "Memory",
        snapshot.and_then(|sample| sample.memory.percent()),
        snapshot.map_or("waiting".into(), |sample| {
            format!(
                "{} / {}",
                bytes(sample.memory.used),
                bytes(sample.memory.total)
            )
        }),
        CYAN,
    );
    render_soft_metric(
        frame,
        metrics[2],
        "Root disk",
        snapshot.and_then(|sample| sample.root_disk.percent()),
        snapshot.map_or("waiting".into(), |sample| {
            format!(
                "{} / {}",
                bytes(sample.root_disk.used),
                bytes(sample.root_disk.total)
            )
        }),
        GREEN,
    );

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
    let history_width = usize::from(rows[2].width.saturating_sub(4)).clamp(12, 72);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Network · latest sample", Style::default().fg(DIM)),
                Span::styled(
                    format!("    ↓ {receive}   ↑ {transmit}"),
                    Style::default().fg(TEXT),
                ),
            ]),
            Line::from(""),
            Line::styled(
                sparkline_text(&host.cpu_history, history_width),
                Style::default().fg(CYAN),
            ),
            Line::styled(
                format!("cpu activity · uptime {uptime}"),
                Style::default().fg(DIM),
            ),
        ])
        .block(
            Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(LINE)),
        ),
        rows[2],
    );
}

fn render_monitor_sidebar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let mut lines = vec![
        Line::styled("Groups", Style::default().fg(DIM)),
        Line::styled(
            format!("◆ {:<14} 1", clipped(&app.local_name, 14)),
            Style::default().fg(AMBER).bold(),
        ),
    ];
    for group in app.machine_groups.iter().take(4) {
        lines.push(Line::styled(
            format!("◇ {:<14} {}", group.name, group.host_ids.len()),
            Style::default().fg(Color::Gray),
        ));
    }
    lines.extend([
        Line::from(""),
        Line::styled("Quick view", Style::default().fg(DIM)),
        Line::styled("○ Alerts          0", Style::default().fg(Color::Gray)),
        Line::styled("= Services        --", Style::default().fg(Color::Gray)),
    ]);
    frame.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::RIGHT)
                .border_style(Style::default().fg(LINE)),
        ),
        area,
    );
}

fn clipped(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

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
            app.begin_group_creation();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.cancel_group_creation();
            app.open_settings();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.start_monitoring(nekohub_core::HostTarget::from_alias("local"));
            terminal.draw(|frame| render(frame, &app)).unwrap();
        }
    }
}
