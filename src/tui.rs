//! Ratatui-based TUI dashboard: clock + Hijri date, countdown to the next
//! prayer, 6 prayer rows (next highlighted), a night-thirds (Qiyam) row, and
//! a footer with location/method + key bindings.
//!
//! Port of `tui.js` from the Node athan-cli.
//!
//! Keys:
//!   q  quit
//!   t  test athan sound
//!   d  toggle scheduler (athan playback at prayer times)

use std::time::{Duration, Instant};

use chrono::{Local, Utc};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::audio::play_sound_in_background;
use crate::config::{Config, load_config, save_config};
use crate::display::{
    PRAYER_INFO, calculate_night_thirds, calculate_prayer_times, current_and_next,
    format_countdown, format_time, method_label, time_by_key, time_until_next,
};
use crate::error::AppError;
use crate::hijri::format_hijri;
use crate::scheduler::{SchedulerHandle, start_scheduler};

/// Restores the terminal on drop, however the TUI exits.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self, AppError> {
        enable_raw_mode()?;
        execute!(std::io::stdout(), EnterAlternateScreen)?;
        Ok(TerminalGuard)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    }
}

/// Run the TUI dashboard. Blocks until the user quits.
pub fn run_tui(_config: &Config) -> Result<(), AppError> {
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;

    // The scheduler is off when the TUI opens (matching the Node TUI);
    // pressing `d` starts/stops it.
    let mut scheduler: Option<SchedulerHandle> = None;
    let mut last_tick = Instant::now() - Duration::from_secs(1);

    loop {
        // 1 s refresh tick.
        if last_tick.elapsed() >= Duration::from_secs(1) {
            let scheduler_on = scheduler.as_ref().is_some_and(SchedulerHandle::is_running);
            terminal.draw(|frame| draw(frame, scheduler_on))?;
            last_tick = Instant::now();
        }

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char('t') => {
                        let cfg = load_config();
                        play_sound_in_background(cfg.athan_sound);
                    }
                    KeyCode::Char('d') => {
                        if scheduler.as_ref().is_some_and(SchedulerHandle::is_running) {
                            if let Some(mut handle) = scheduler.take() {
                                handle.stop();
                            }
                        } else {
                            scheduler = Some(start_scheduler(|_| {
                                // The 1 s render tick picks up fired athans on its own.
                            }));
                            // Persist the toggle so `athan start` and the TUI agree.
                            let cfg = load_config();
                            if let Err(err) = save_config(&Config {
                                athan_enabled: true,
                                ..cfg
                            }) {
                                eprintln!("Failed to save config: {err}");
                            }
                        }
                        last_tick = Instant::now() - Duration::from_secs(1);
                    }
                    _ => {}
                }
            }
    }

    if let Some(mut handle) = scheduler.take() {
        handle.stop();
    }
    Ok(())
}

fn draw(frame: &mut ratatui::Frame, scheduler_on: bool) {
    let cfg = load_config();
    let lat = cfg.latitude.unwrap_or(f64::NAN);
    let lng = cfg.longitude.unwrap_or(f64::NAN);
    let now = Utc::now();
    let local_now = now.with_timezone(&Local);
    let date = local_now.date_naive();

    let times = calculate_prayer_times(lat, lng, date, &cfg.method, &cfg.madhab);
    let (_, next) = current_and_next(&times, now);
    let until = time_until_next(&times, now);

    let chunks = Layout::vertical([
        Constraint::Length(3), // clock
        Constraint::Length(1), // hijri / date line
        Constraint::Length(2), // countdown
        Constraint::Length(1), // spacer
        Constraint::Length(8), // prayer table
        Constraint::Length(2), // qiyam
        Constraint::Min(0),    // filler
        Constraint::Length(2), // footer
    ])
    .split(frame.area());

    // Big clock.
    let clock = Paragraph::new(Line::from(Span::styled(
        local_now.format("%-I:%M:%S %p").to_string(),
        Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
    )))
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(clock, chunks[0]);

    // Gregorian + Hijri date.
    let date_line = format!(
        "{}  •  {}",
        local_now.format("%A, %b %-d, %Y"),
        format_hijri(date)
    );
    let hijri = Paragraph::new(Line::from(Span::styled(
        date_line,
        Style::default().fg(Color::Cyan),
    )))
    .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(hijri, chunks[1]);

    // Countdown to the next prayer.
    let countdown_line = if next != "none" && until.is_some() {
        Line::from(vec![
            Span::raw("Next: "),
            Span::styled(
                crate::display::prayer_name(&next),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" in {}", format_countdown(until.unwrap_or_default()))),
        ])
    } else {
        Line::from("Next: Fajr tomorrow")
    };
    let countdown = Paragraph::new(countdown_line)
        .style(Style::default().fg(Color::Green))
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(countdown, chunks[2]);

    // Prayer rows, next one highlighted.
    let rows: Vec<Line> = PRAYER_INFO
        .iter()
        .map(|p| {
            let label = format!("{:9} {}", p.en, format_time(time_by_key(&times, p.key)));
            if p.key == next {
                Line::from(Span::styled(
                    format!("▶ {label}"),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(format!("  {label}"))
            }
        })
        .collect();
    let table = Paragraph::new(rows).alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(table, chunks[4]);

    // Night-thirds (Qiyam) row.
    let qiyam_lines = match calculate_night_thirds(lat, lng, date, &cfg.method, &cfg.madhab, now) {
        Some(thirds) => {
            let active_label = thirds
                .active
                .map(|a| format!(" (third {a} of 3)"))
                .unwrap_or_default();
            vec![
                Line::from(format!("Qiyam — last third of night{active_label}")),
                Line::from(format!(
                    "  {} → {}",
                    format_time(Some(thirds.third3.start)),
                    format_time(Some(thirds.third3.end))
                )),
            ]
        }
        None => vec![
            Line::from("Qiyam — last third of night"),
            Line::from("  --:-- → --:--"),
        ],
    };
    let qiyam = Paragraph::new(qiyam_lines)
        .style(Style::default().fg(Color::Magenta))
        .alignment(ratatui::layout::Alignment::Center);
    frame.render_widget(qiyam, chunks[5]);

    // Footer: location/method + keys.
    let scheduler_state = if scheduler_on {
        Span::styled("on", Style::default().fg(Color::Green))
    } else {
        Span::styled("off", Style::default().fg(Color::Red))
    };
    let footer = Paragraph::new(vec![
        Line::from(vec![
            Span::raw(format!(
                " {lat}, {lng} • {} • {} • scheduler: ",
                method_label(&cfg.method),
                cfg.madhab
            )),
            scheduler_state,
        ]),
        Line::from(" [q] quit   [t] test athan   [d] toggle scheduler"),
    ])
    .style(Style::default().fg(Color::Gray));
    frame.render_widget(footer, chunks[7]);
}
