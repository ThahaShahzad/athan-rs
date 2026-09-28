//! Ratatui-based TUI kiosk: full-screen block-digit clock, prominent
//! next-prayer countdown, six bordered prayer cards, night-thirds (Qiyam)
//! cards, and a slim footer with location/method + key bindings.
//!
//! Port of `tui.js` from the Node athan-cli, upgraded to a kiosk layout.
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
use ratatui::Frame;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::audio::play_sound_in_background;
use crate::config::{Config, load_config, save_config};
use crate::display::{
    PRAYER_INFO, calculate_night_thirds, calculate_prayer_times, current_and_next,
    format_time, method_label, time_by_key,
};
use crate::error::AppError;
use crate::hijri::format_hijri;
use athan::prayer_times::PrayerTimes;
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

// ---------------------------------------------------------------------------
// Big block-digit glyphs (5 wide, 7 tall).
// ---------------------------------------------------------------------------

const DIGITS: [[&str; 7]; 10] = [
    [" ███ ", "█   █", "█   █", "█   █", "█   █", "█   █", " ███ "],
    ["   █ ", "  ██ ", "   █ ", "   █ ", "   █ ", "   █ ", " ███ "],
    [" ███ ", "█   █", "    █", "  ██ ", " █   ", "█    ", "█████"],
    [" ███ ", "█   █", "    █", "  ██ ", "    █", "█   █", " ███ "],
    ["   ██", "  █ █", " █  █", "█   █", "█████", "    █", "    █"],
    ["█████", "█    ", "████ ", "    █", "    █", "█   █", " ███ "],
    [" ███ ", "█   █", "█    ", "████ ", "█   █", "█   █", " ███ "],
    ["█████", "    █", "   █ ", "  █  ", " █   ", " █   ", " █   "],
    [" ███ ", "█   █", "█   █", " ███ ", "█   █", "█   █", " ███ "],
    [" ███ ", "█   █", "█   █", " ████", "    █", "█   █", " ███ "],
];

const COLON: [&str; 7] = ["   ", " █ ", " █ ", "   ", " █ ", " █ ", "   "];

const BLANK: [&str; 7] = ["   "; 7];

fn glyph_rows(ch: char, colon_on: bool) -> [&'static str; 7] {
    match ch {
        '0'..='9' => DIGITS[ch.to_digit(10).unwrap_or(0) as usize],
        ':' if colon_on => COLON,
        _ => BLANK,
    }
}

fn digit_style() -> Style {
    Style::default()
}

fn draw_clock(frame: &mut Frame, area: Rect, local_now: chrono::DateTime<Local>) {
    // Big block-digit clock fits when the top area is at least 7 tall and the
    // glyphs + AM/PM tag fit (5-glyph digits separated by spaces).
    let big = area.height >= 7 && area.width >= 48;

    if !big {
        let clock = Paragraph::new(Line::from(Span::styled(
            local_now.format("%-I:%M:%S %p").to_string(),
            digit_style(),
        )))
        .alignment(Alignment::Center);
        frame.render_widget(clock, area);
        return;
    }

    let time = local_now.format("%-I:%M:%S").to_string();
    let period = local_now.format("%p").to_string();
    let colon_on = local_now.timestamp() % 2 == 0;
    let digit_style = Style::default();

    // One 7-row composition: digits separated by single spaces, then a gap
    // with the AM/PM tag on the middle row. Each row is rendered into its own
    // 1-row rect so they stack instead of overwriting each other.
    let mut rows: Vec<Vec<Span>> = vec![Vec::new(); 7];
    for (i, ch) in time.chars().enumerate() {
        if i > 0 {
            for row in &mut rows {
                row.push(Span::raw(" "));
            }
        }
        let glyph = glyph_rows(ch, colon_on);
        for (r, seg) in glyph.iter().enumerate() {
            rows[r].push(Span::styled((*seg).to_string(), digit_style));
        }
    }
    for (r, row) in rows.iter_mut().enumerate() {
        row.push(Span::raw("   "));
        row.push(Span::styled(
            if r == 3 { period.clone() } else { "     " .to_string() },
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));
    }
    for (r, row) in rows.into_iter().enumerate() {
        frame.render_widget(
            Paragraph::new(Line::from(row)).alignment(Alignment::Center),
            Rect::new(area.x, area.y + r as u16, area.width, 1),
        );
    }
}

/// Formatted as "H:MM:SS" e.g. 3:04:09.
fn format_hms(ms: i64) -> String {
    let total = (ms / 1000).max(0);
    format!("{}:{:02}:{:02}", total / 3600, (total % 3600) / 60, total % 60)
}

fn draw_date(frame: &mut Frame, area: Rect, local_now: chrono::DateTime<Local>) {
    let date = local_now.date_naive();
    let date_line = format!(
        "{}  •  {}",
        local_now.format("%A, %b %-d, %Y"),
        format_hijri(date)
    );
    let hijri = Paragraph::new(Line::from(Span::styled(
        date_line,
        Style::default().fg(Color::Cyan),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(hijri, area);
}

/// 3-row compact block font (each glyph 3 wide, two vertical pixel halves
/// per character row). Bits per row: bit2=left, bit1=mid, bit0=right.
const SMALL_DIGITS: [[u8; 6]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b101, 0b111],
    [0b010, 0b110, 0b010, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111, 0b111],
    [0b111, 0b001, 0b111, 0b001, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001, 0b001],
    [0b111, 0b100, 0b111, 0b001, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b101, 0b111],
    [0b111, 0b001, 0b001, 0b001, 0b001, 0b001],
    [0b111, 0b101, 0b111, 0b101, 0b101, 0b111],
    [0b111, 0b101, 0b111, 0b001, 0b001, 0b111],
];
const SMALL_COLON: [u8; 6] = [0, 0b010, 0, 0b010, 0, 0];

fn small_glyph(ch: char) -> [u8; 6] {
    match ch {
        '0'..='9' => SMALL_DIGITS[ch.to_digit(10).unwrap_or(0) as usize],
        ':' => SMALL_COLON,
        _ => [0; 6],
    }
}

fn half_char(top: bool, bottom: bool) -> char {
    match (top, bottom) {
        (true, true) => '█',
        (true, false) => '▀',
        (false, true) => '▄',
        _ => ' ',
    }
}

/// Render ASCII digits/colons as 3-row half-block digits, centered in `area`.
/// Returns false when they don't fit and the caller should fall back to text.
fn render_small_digits(frame: &mut Frame, area: Rect, text: &str, style: Style) -> bool {
    if area.height < 3 || text.is_empty() {
        return false;
    }
    let needed = (text.chars().count() * 4).saturating_sub(1);
    if area.width < u16::try_from(needed).unwrap_or(u16::MAX) {
        return false;
    }
    let mut rows: Vec<Vec<Span>> = vec![Vec::new(); 3];
    for (i, ch) in text.chars().enumerate() {
        if i > 0 {
            for row in &mut rows {
                row.push(Span::styled(" ", style));
            }
        }
        let m = small_glyph(ch);
        for (r, row) in rows.iter_mut().enumerate() {
            let mut seg = String::new();
            for c in 0..3 {
                let mask = 4 >> c;
                seg.push(half_char(m[r * 2] & mask != 0, m[r * 2 + 1] & mask != 0));
            }
            row.push(Span::styled(seg, style));
        }
    }
    for (r, row) in rows.into_iter().enumerate() {
        frame.render_widget(
            Paragraph::new(Line::from(row)).alignment(Alignment::Center),
            Rect::new(area.x, area.y + r as u16, area.width, 1),
        );
    }
    true
}

fn draw_countdown(frame: &mut Frame, label: Rect, digits: Rect, next_name: &str, until_ms: i64) {
    let text = Paragraph::new(Line::from(Span::styled(
        format!("Until {next_name}").to_uppercase(),
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(text, label);
    let style = Style::default().fg(Color::Green).add_modifier(Modifier::BOLD);
    if digits.height >= 3 && render_small_digits(frame, digits, &format_hms(until_ms), style) {
        return;
    }
    frame.render_widget(
        Paragraph::new(format_hms(until_ms)).style(style).alignment(Alignment::Center),
        digits,
    );
}

fn draw_prayer_cards(frame: &mut Frame, area: Rect, times: &PrayerTimes, next: &str) {
    let cells = Layout::vertical([Constraint::Ratio(1, 6); 6])
        .spacing(1)
        .split(area);
    for (p, chunk) in PRAYER_INFO.iter().zip(cells.iter()) {
        let is_next = p.key == next;
        let time = format_time(time_by_key(times, p.key));
        let title_style =
            Style::default().fg(if is_next { Color::Green } else { Color::Gray }).add_modifier(Modifier::BOLD);
        let time_style = if is_next {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let block = Block::bordered()
            .title(format!(" {} ", p.en))
            .title_alignment(Alignment::Center)
            .title_style(title_style)
            .border_style(Style::default().fg(if is_next { Color::Green } else { Color::DarkGray }));
        frame.render_widget(block, *chunk);
        let inner = Rect {
            x: chunk.x + 1,
            y: chunk.y + 1,
            width: chunk.width.saturating_sub(2),
            height: chunk.height.saturating_sub(2),
        };
        let row_y = inner.y + inner.height.saturating_sub(1) / 2;
        let body = Paragraph::new(Span::styled(time, time_style))
            .alignment(Alignment::Center);
        frame.render_widget(body, Rect::new(inner.x, row_y, inner.width, 1));
    }
}

fn draw_qiyam_card(frame: &mut Frame, area: Rect, thirds: &crate::display::NightThirds) {
    let active = thirds.active;
    let window = format!(
        "{} → {}",
        format_time(Some(thirds.third3.start)),
        format_time(Some(thirds.third3.end))
    );
    let (title_style, border_style, text_style) = if active.is_some() {
        (
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
            Style::default().fg(Color::Magenta),
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        )
    } else {
        (
            Style::default().fg(Color::Gray),
            Style::default().fg(Color::DarkGray),
            Style::default().fg(Color::White),
        )
    };
    let active_note = match active {
        Some(n) => format!(" — active (third {n} of 3)"),
        None => String::new(),
    };
    let body = Paragraph::new(vec![
        Line::from(Span::styled(window, text_style)),
        Line::from(Span::styled(
            format!("last third of night{active_note}"),
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .alignment(Alignment::Center)
    .block(
        Block::bordered()
            .title(" Qiyam ")
            .title_alignment(Alignment::Center)
            .title_style(title_style)
            .border_style(border_style),
    );
    frame.render_widget(body, area);
}

fn draw_footer(frame: &mut Frame, area: Rect, left: String, scheduler_on: bool) {
    let chunks = Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);
    let scheduler_state = if scheduler_on {
        Span::styled("on", Style::default().fg(Color::Green))
    } else {
        Span::styled("off", Style::default().fg(Color::Red))
    };
    let left = Paragraph::new(Line::from(vec![
        Span::raw(left),
        scheduler_state,
    ]));
    let right = Paragraph::new(Line::from("q quit   t test athan   d toggle scheduler"))
        .alignment(Alignment::Right);
    frame.render_widget(left, chunks[0]);
    frame.render_widget(right, chunks[1]);
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
    let times_tomorrow =
        calculate_prayer_times(lat, lng, date.succ_opt().unwrap_or(date), &cfg.method, &cfg.madhab);

    // Next prayer + countdown; after Isha count down to tomorrow's Fajr.
    let (next_name, until_ms): (String, i64) = if next != "none" {
        match time_by_key(&times, &next) {
            Some(t) => (
                crate::display::prayer_name(&next).to_string(),
                (t - now).num_milliseconds().max(0),
            ),
            None => ("Next prayer later".to_string(), 0),
        }
    } else if let Some(fajr) = times_tomorrow.fajr {
        (
            format!("{} (tomorrow)", crate::display::prayer_name("fajr")),
            (fajr - now).num_milliseconds().max(0),
        )
    } else {
        ("Next prayer tomorrow Fajr".to_string(), 0)
    };

    let area = frame.area();

    // Full-screen split: data column (clock, date, countdown, qiyam) on the
    // left, six stacked prayer cards on the right taking the full height;
    // footer sits under the left column only.
    let columns = Layout::horizontal([
        Constraint::Percentage(50), // data
        Constraint::Percentage(50), // prayers
    ])
    .split(area);

    let data = Layout::vertical([
        Constraint::Length(7), // big clock
        Constraint::Length(1), // hijri / date line
        Constraint::Length(1), // countdown label
        Constraint::Length(3), // countdown block digits
        Constraint::Min(1),    // spacer
        Constraint::Length(4), // qiyam card
        Constraint::Min(1),    // spacer
        Constraint::Length(1), // footer (left column only)
    ])
    .split(columns[0]);

    let prayers = Layout::vertical([
        Constraint::Length(1), // "Prayer times" heading
        Constraint::Min(6),    // six stacked cards, full remaining height
    ])
    .split(columns[1]);

    draw_clock(frame, data[0], local_now);
    draw_date(frame, data[1], local_now);
    draw_countdown(frame, data[2], data[3], &next_name, until_ms);

    match calculate_night_thirds(lat, lng, date, &cfg.method, &cfg.madhab, now) {
        Some(thirds) => draw_qiyam_card(frame, data[5], &thirds),
        None => {
            let label = Paragraph::new(Span::styled(
                "Qiyam — night thirds unavailable",
                Style::default().fg(Color::DarkGray),
            ))
            .alignment(Alignment::Center)
            .block(Block::bordered().border_style(Style::default().fg(Color::DarkGray)));
            frame.render_widget(label, data[5]);
        }
    }

    let heading = Paragraph::new(Line::from(Span::styled(
        "Prayer times — today",
        Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(heading, prayers[0]);
    draw_prayer_cards(frame, prayers[1], &times, &next);

    draw_footer(
        frame,
        data[7],
        format!(
            " lat {lat:.4} lng {lng:.4} • {} • {} madhab • scheduler: ",
            method_label(&cfg.method),
            cfg.madhab
        ),
        scheduler_on,
    );
}
