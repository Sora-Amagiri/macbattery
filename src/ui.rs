//! Live ratatui dashboard.

use std::io;
use std::time::{Duration, Instant};

use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Gauge, Paragraph};
use ratatui::{DefaultTerminal, Frame};

use crate::battery::{self, BatteryInfo};

const REFRESH: Duration = Duration::from_secs(2);
const ACCENT: Color = Color::Cyan;

struct App {
    info: BatteryInfo,
    paused: bool,
    last_update: Instant,
}

pub fn run() -> io::Result<()> {
    let mut terminal = ratatui::try_init()?;
    let result = event_loop(&mut terminal);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut DefaultTerminal) -> io::Result<()> {
    let mut app = App {
        info: battery::read(),
        paused: false,
        last_update: Instant::now(),
    };

    loop {
        terminal.draw(|frame| draw(frame, &app))?;

        // Wake up often enough for snappy keys, refresh on a timer.
        if event::poll(Duration::from_millis(200))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            match key.code {
                // Raw mode disables SIGINT, so Ctrl+C must be handled as a key.
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    return Ok(());
                }
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('r') => {
                    app.info = battery::read();
                    app.last_update = Instant::now();
                }
                KeyCode::Char('p') => app.paused = !app.paused,
                _ => {}
            }
        }

        if !app.paused && app.last_update.elapsed() >= REFRESH {
            app.info = battery::read();
            app.last_update = Instant::now();
        }
    }
}

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    if area.width < 62 || area.height < 18 {
        frame.render_widget(
            Paragraph::new("Terminal too small — resize to at least 62x18.")
                .alignment(Alignment::Center),
            area,
        );
        return;
    }

    let rows = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(7),
        Constraint::Length(4),
        Constraint::Length(1),
    ])
    .split(area);

    if !app.info.present {
        // Desktops / VMs: keep the screen clean instead of zero-filled panels.
        let block = Block::bordered().title(Span::styled(
            " MacBook Battery ",
            Style::default().fg(ACCENT).bold(),
        ));
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from("  No battery detected on this machine."),
                Line::from("  (desktop Mac, VM, or battery service unavailable)"),
            ])
            .block(block),
            rows[0],
        );
        draw_footer(frame, app, rows[3]);
        return;
    }

    draw_header(frame, app, rows[0]);
    draw_panels(frame, &app.info, rows[1]);
    draw_details(frame, &app.info, rows[2]);
    draw_footer(frame, app, rows[3]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let info = &app.info;
    let block = Block::bordered().title(Span::styled(
        " MacBook Battery ",
        Style::default().fg(ACCENT).bold(),
    ));

    let color = charge_color(info.charge_percent);
    let label = format!(
        "{:.0}%  ·  {} / {} mAh  ·  {}",
        info.charge_percent,
        info.current_capacity,
        info.full_charge_capacity,
        info.state()
    );
    let gauge = Gauge::default()
        .block(block)
        .gauge_style(Style::default().fg(color).bg(Color::Rgb(40, 40, 40)))
        .ratio((info.charge_percent / 100.0).clamp(0.0, 1.0))
        .label(Span::styled(
            label,
            Style::default().fg(Color::White).bold(),
        ));

    let inner = Layout::vertical([Constraint::Length(3), Constraint::Length(1)]).split(area);
    frame.render_widget(gauge, inner[0]);

    let status = if let Some(min) = info.time_to_full_min {
        format!(
            " {} until full  ·  {}  ·  {}",
            battery::fmt_duration(min),
            if info.on_ac { "AC power" } else { "battery" },
            info.adapter_watts
                .map(|w| format!("{w} W adapter"))
                .unwrap_or_default()
        )
    } else if let Some(min) = info.time_to_empty_min {
        format!(
            " {} remaining  ·  on battery  ·  {:.1} W",
            battery::fmt_duration(min),
            info.watts
        )
    } else {
        format!(
            " Time estimate unavailable  ·  {}  ·  {:.1} W",
            if info.on_ac { "AC power" } else { "on battery" },
            info.watts
        )
    };
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            status,
            Style::default().fg(Color::Gray),
        ))),
        inner[1],
    );
}

fn draw_panels(frame: &mut Frame, info: &BatteryInfo, area: Rect) {
    let cols =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).split(area);
    draw_health(frame, info, cols[0]);
    draw_power(frame, info, cols[1]);
}

fn draw_health(frame: &mut Frame, info: &BatteryInfo, area: Rect) {
    let block =
        Block::bordered().title(Span::styled(" Health ", Style::default().fg(ACCENT).bold()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            kv("Cycles"),
            Span::raw(format!("{} / {}", info.cycle_count, info.max_cycle_count)),
        ])),
        rows[0],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(Style::default().fg(Color::Green).bg(Color::Rgb(40, 40, 40)))
            .ratio((info.cycle_count as f64 / info.max_cycle_count.max(1) as f64).clamp(0.0, 1.0))
            .label(Span::styled(
                format!("{:.0}% of cycle budget used", cycle_pct(info)),
                Style::default().fg(Color::Gray),
            )),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            kv("Health"),
            Span::styled(
                format!("{:.1} %", info.health_percent),
                Style::default()
                    .fg(health_color(info.health_percent))
                    .bold(),
            ),
        ])),
        rows[2],
    );
    frame.render_widget(
        Gauge::default()
            .gauge_style(
                Style::default()
                    .fg(health_color(info.health_percent))
                    .bg(Color::Rgb(40, 40, 40)),
            )
            .ratio((info.health_percent / 100.0).clamp(0.0, 1.0))
            .label(Span::styled(
                format!(
                    "{} mAh of {} mAh design",
                    info.full_charge_capacity, info.design_capacity
                ),
                Style::default().fg(Color::Gray),
            )),
        rows[3],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            kv("Condition"),
            Span::styled(
                info.condition.as_str(),
                Style::default().fg(condition_color(info)).bold(),
            ),
        ])),
        rows[4],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            kv("Temperature"),
            Span::raw(format!("{:.1} °C", info.temperature_c)),
        ])),
        rows[5],
    );
}

fn draw_power(frame: &mut Frame, info: &BatteryInfo, area: Rect) {
    let block =
        Block::bordered().title(Span::styled(" Power ", Style::default().fg(ACCENT).bold()));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(inner);

    let flow = if info.is_charging {
        "charging"
    } else if info.on_ac {
        "on AC"
    } else {
        "discharging"
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            kv("Voltage"),
            Span::raw(format!("{:.3} V", info.voltage_mv as f64 / 1000.0)),
        ])),
        rows[0],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            kv("Current"),
            Span::styled(
                battery::fmt_amperage(info.amperage_ma),
                Style::default().fg(if info.amperage_ma >= 0 {
                    Color::Green
                } else {
                    Color::Yellow
                }),
            ),
        ])),
        rows[1],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            kv("Power"),
            Span::raw(format!("{:.1} W", info.watts)),
            Span::styled(format!("  ({flow})"), Style::default().fg(Color::DarkGray)),
        ])),
        rows[2],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            kv("Adapter"),
            Span::raw(
                info.adapter_watts
                    .map(|w| format!("{w} W"))
                    .unwrap_or_else(|| "—".into()),
            ),
        ])),
        rows[3],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            kv("Charge"),
            Span::raw(format!(
                "{} mAh  ({:.0}%)",
                info.current_capacity, info.charge_percent
            )),
        ])),
        rows[4],
    );
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            kv("Full charge"),
            Span::raw(format!("{} mAh", info.full_charge_capacity)),
        ])),
        rows[5],
    );
}

fn draw_details(frame: &mut Frame, info: &BatteryInfo, area: Rect) {
    let block = Block::bordered().title(Span::styled(
        " Details ",
        Style::default().fg(ACCENT).bold(),
    ));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let line1 = Line::from(vec![
        kv("Serial"),
        Span::raw(format!(
            "{:<20}",
            info.serial.clone().unwrap_or_else(|| "—".into())
        )),
        kv("Manufacturer"),
        Span::raw(info.manufacturer.clone().unwrap_or_else(|| "—".into())),
    ]);
    let line2 = Line::from(vec![
        kv("Model"),
        Span::raw(format!(
            "{:<20}",
            info.device_name.clone().unwrap_or_else(|| "—".into())
        )),
        kv("Manufactured"),
        Span::raw(info.manufacture_date.clone().unwrap_or_else(|| "—".into())),
    ]);
    frame.render_widget(Paragraph::new(vec![line1, line2]), inner);
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let status = if app.paused { " · paused" } else { "" };
    let footer = Line::from(vec![
        Span::styled(" q", Style::default().fg(Color::White).bold()),
        Span::styled(" quit ", Style::default().fg(Color::DarkGray)),
        Span::styled("r", Style::default().fg(Color::White).bold()),
        Span::styled(" refresh ", Style::default().fg(Color::DarkGray)),
        Span::styled("p", Style::default().fg(Color::White).bold()),
        Span::styled(" pause ", Style::default().fg(Color::DarkGray)),
        Span::styled(status, Style::default().fg(Color::Yellow)),
    ]);
    frame.render_widget(Paragraph::new(footer), area);
}

// ---------------------------------------------------------------------------

fn kv(key: &'static str) -> Span<'static> {
    Span::styled(format!("{key:<15}"), Style::default().fg(Color::DarkGray))
}

fn cycle_pct(info: &BatteryInfo) -> f64 {
    info.cycle_count as f64 / info.max_cycle_count.max(1) as f64 * 100.0
}

fn charge_color(pct: f64) -> Color {
    if pct <= 20.0 {
        Color::Red
    } else if pct <= 50.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn health_color(pct: f64) -> Color {
    if pct < 80.0 {
        Color::Red
    } else if pct < 90.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}

fn condition_color(info: &BatteryInfo) -> Color {
    match info.condition {
        battery::Condition::Normal => Color::Green,
        battery::Condition::ServiceRecommended => Color::Red,
    }
}

#[cfg(test)]
mod tests;
