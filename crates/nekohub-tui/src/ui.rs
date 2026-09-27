#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]

use std::time::Duration;

use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Clear, Gauge, Paragraph, Row, Sparkline, Table, Widget,
        Wrap,
    },
};

use crate::{
    app::{App, HomeFocus, HostState, View},
    machines::INSTALLED_TAG,
    preferences::{FontProfile, Theme},
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

pub fn render(frame: &mut Frame<'_>, app: &App) {
    let area = frame.area();
    render_content(frame, app);
    frame.render_widget(
        ThemeOverlay {
            accent: theme_color(app.theme),
            font_profile: app.font_profile,
        },
        area,
    );
}

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
        View::RemotePicker => {
            render_remote_picker(frame, frame.area(), app);
            return;
        }
        View::RemoteInstall => {
            render_remote_picker(frame, frame.area(), app);
            render_remote_install(frame, frame.area(), app);
            return;
        }
        View::Settings => {
            render_settings(frame, frame.area(), app);
            return;
        }
        View::Overview | View::Detail => {}
    }
    let content = render_app_chrome(
        frame,
        frame.area(),
        app,
        1,
        "↑↓ sections  ·  Enter open  ·  n/p machine  ·  r refresh  ·  Esc home  ·  q quit",
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
        | View::RemoteInstall
        | View::Settings => {
            unreachable!("setup views return before shell render")
        }
    }
}

struct ThemeOverlay {
    accent: Color,
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
                if style.fg == Some(AMBER) {
                    cell.set_fg(self.accent);
                }
                if style.bg == Some(AMBER) {
                    cell.set_bg(self.accent);
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

const fn theme_color(theme: Theme) -> Color {
    match theme {
        Theme::Pink => Color::Rgb(239, 139, 181),
        Theme::Blue => Color::Rgb(112, 166, 255),
        Theme::Red => Color::Rgb(235, 101, 101),
        Theme::Purple => Color::Rgb(180, 132, 255),
    }
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
            Constraint::Min(8),
            Constraint::Length(1),
        ])
        .split(content);
    let ready = 1;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("GROUPS", Style::default().fg(AMBER).bold()),
            Span::styled(
                format!("    {} spaces", app.machine_groups.len() + 1),
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
    render_group_grid(frame, rows[1], app);
    if let Some(notice) = app.home_notice.as_deref() {
        frame.render_widget(
            Paragraph::new(notice)
                .style(Style::default().fg(ORANGE))
                .alignment(Alignment::Center),
            rows[2],
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
        "↑↓ section  ·  ←→ choose  ·  Enter apply  ·  Esc home  ·  q quit",
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
    if app.settings_selected == 0 {
        render_appearance_settings(frame, columns[2], app, color);
        return;
    }
    if app.settings_selected == 1 {
        render_theme_settings(frame, columns[2], app, color);
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
                .border_style(Style::default().fg(color)),
        ),
        columns[2],
    );
}

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
                .border_style(Style::default().fg(color)),
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
                " ←→ changes symbols and visual density · terminal controls the real font",
                Style::default().fg(DIM),
            ),
        ])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(color)),
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
        let selected = app.theme == theme;
        let swatch = theme_color(theme);
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled("██████", Style::default().fg(swatch)),
                Line::from(""),
                Line::styled(
                    theme.label(),
                    if selected {
                        Style::default().fg(INK).bg(swatch).bold()
                    } else {
                        Style::default().fg(TEXT)
                    },
                ),
                Line::styled(
                    if selected { "● active" } else { "○ select" },
                    Style::default().fg(swatch),
                ),
            ])
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(if selected { swatch } else { LINE })),
            ),
            cards[index],
        );
    }
    let message = app
        .settings_notice
        .as_deref()
        .unwrap_or("Use ←→ or Enter to preview. Changes are saved immediately.");
    frame.render_widget(
        Paragraph::new(message).style(Style::default().fg(color)),
        rows[2],
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

#[allow(clippy::too_many_lines)]
fn render_remote_picker(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let inner = render_app_chrome(
        frame,
        area,
        app,
        1,
        "↑↓ select  ·  Enter open  ·  + install agent  ·  Esc home  ·  q quit",
    );
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
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
        let installed = host.tags.iter().any(|tag| tag == INSTALLED_TAG);
        Row::new([
            if selected { "›" } else { " " },
            host.display_name.as_str(),
            if installed {
                "nekoHub agent"
            } else {
                "SSH inventory"
            },
            "Unassigned",
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

    let install_selected = app.remote_install_selected();
    frame.render_widget(
        Paragraph::new(Line::from(vec![
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
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(if install_selected { AMBER } else { LINE })),
        ),
        rows[2],
    );
}

fn render_remote_install(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let width = area.width.min(76);
    let height = area.height.min(18);
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
        .title_bottom(Line::from(" Enter connect & install  ·  Esc cancel ").centered())
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
            Constraint::Length(4),
            Constraint::Min(2),
        ])
        .split(inner);
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                "Connect to a Debian or Ubuntu machine over SSH.",
                Style::default().fg(TEXT).bold(),
            ),
            Line::from(""),
            Line::styled(
                "nekoHub will add its signed APT repository, install nekohub-agent and start it.",
                Style::default().fg(Color::Gray),
            ),
            Line::styled(
                "Passwords are handled by SSH and sudo. They are never read or saved by nekoHub.",
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
        Paragraph::new(format!(" {}{cursor}", app.remote_install_draft)).block(
            Block::default()
                .title(" SSH destination · user@machine ")
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(AMBER)),
        ),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled("What happens next", Style::default().fg(AMBER).bold()),
            Line::styled(
                "The interface briefly yields to the normal SSH session for host verification and password prompts.",
                Style::default().fg(Color::Gray),
            ),
        ])
        .wrap(Wrap { trim: true }),
        rows[2],
    );
    if let Some(error) = app.remote_install_error.as_deref() {
        frame.render_widget(
            Paragraph::new(error)
                .style(Style::default().fg(RED))
                .alignment(Alignment::Center),
            rows[3],
        );
    }
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
            Line::from(vec![
                Span::styled(identity, Style::default().fg(DIM)),
                Span::styled(
                    snapshot.map_or_else(String::new, |sample| {
                        format!(
                            "    uptime {}    latency {}ms",
                            human_duration(Duration::from_secs(sample.uptime_secs)),
                            sample.latency_ms
                        )
                    }),
                    Style::default().fg(Color::Gray),
                ),
            ]),
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

fn render_monitor_sidebar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let sections = [
        ("Overview", AMBER),
        ("Processes", CYAN),
        ("Network", GREEN),
        ("Storage", ORANGE),
        ("Services", Color::Rgb(164, 143, 214)),
        ("Containers", CYAN),
        ("Logs", Color::Gray),
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
    let data: Vec<_> = host.cpu_history.iter().copied().collect();
    if data.is_empty() {
        frame.render_widget(
            Paragraph::new("Waiting for the first CPU samples")
                .style(Style::default().fg(DIM))
                .alignment(Alignment::Center),
            inner,
        );
    } else {
        frame.render_widget(
            Sparkline::default()
                .data(&data)
                .max(100)
                .style(Style::default().fg(AMBER)),
            inner,
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
    frame.render_widget(
        Sparkline::default()
            .data(&rx)
            .style(Style::default().fg(CYAN)),
        rows[0],
    );
    frame.render_widget(
        Sparkline::default()
            .data(&tx)
            .style(Style::default().fg(GREEN)),
        rows[1],
    );
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
            app.settings_selected = 1;
            app.theme = Theme::Purple;
            app.font_profile = FontProfile::Ascii;
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.open_remote_picker();
            app.remote_selected = app.remote_item_count() - 1;
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.begin_remote_install();
            terminal.draw(|frame| render(frame, &app)).unwrap();
            app.cancel_remote_install();
            app.start_monitoring(nekohub_core::HostTarget::from_alias("local"));
            terminal.draw(|frame| render(frame, &app)).unwrap();
        }
    }
}
