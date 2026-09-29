//! Ratatui-based TUI kiosk: a giant block-digit clock filling the left
//! column, a prominent next-prayer countdown, six bordered prayer cards, a
//! night-thirds (Qiyam) card, and a slim footer with location/method + keys.
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
// Scaled block-digit glyphs: a 5x7 pixel font stretched to fill its area.
// Each glyph pixel becomes one cell wide and one half-block tall, so pixels
// stay square and the digits grow as large as the box allows.
// ---------------------------------------------------------------------------

const DIGITS: [[&str; 7]; 10] = [
    [" ███ ", "█   █", "█   █", "█   █", "█   █", "█   █", " ███ "],
    ["   █ ", "  ██ ", "   █ ", "   █ ", "   █ ", "   █ ", "   █ "],
    [" ███ ", "█   █", "    █", "  ██ ", " █   ", "█    ", "█████"],
    [" ███ ", "█   █", "    █", "  ██ ", "    █", "█   █", " ███ "],
    ["   ██", "  █ █", " █  █", "█   █", "█████", "    █", "    █"],
    ["█████", "█    ", "████ ", "    █", "    █", "█   █", " ███ "],
    [" ███ ", "█   █", "█    ", "████ ", "█   █", "█   █", " ███ "],
    ["█████", "    █", "   █ ", "  █  ", " █   ", " █   ", " █   "],
    [" ███ ", "█   █", "█   █", " ███ ", "█   █", "█   █", " ███ "],
    [" ███ ", "█   █", "█   █", " ████", "    █", "█   █", " ███ "],
];

const GLYPH_HEIGHT: u16 = 7;

fn glyph_width(ch: char) -> u16 {
    match ch {
        '0'..='9' => 5,
        _ => 3,
    }
}

/// Pixel (col, row) of the 7-row glyph grid for `ch`; colon pixels blink.
fn glyph_pixel(ch: char, colon_on: bool, col: u16, row: usize) -> bool {
    match ch {
        '0'..='9' => {
            let glyph = DIGITS[ch.to_digit(10).unwrap_or(0) as usize];
            glyph[row].chars().nth(col as usize) == Some('█')
        }
        ':' => colon_on && col == 1 && matches!(row, 1 | 2 | 4 | 5),
        _ => false,
    }
}

/// Render `text` as block digits scaled up to fill `area`; each glyph pixel
/// is drawn as an `s`x`s` block of half-row pixels so it stays square.
/// Returns false when the text cannot fit and the caller should fall back
/// to a plain text paragraph.
fn render_big_digits(
    frame: &mut Frame,
    area: Rect,
    text: &str,
    colon_on: bool,
    style: Style,
) -> bool {
    let chars: Vec<char> = text.chars().collect();
    if area.width == 0 || area.height == 0 || chars.is_empty() {
        return false;
    }

    // Grid column where each character starts, plus the total grid width.
    let mut starts: Vec<u16> = Vec::with_capacity(chars.len());
    let mut grid_w: u16 = 0;
    for (i, &ch) in chars.iter().enumerate() {
        if i > 0 {
            grid_w += 1; // 1-column gap between glyphs
        }
        starts.push(grid_w);
        grid_w += glyph_width(ch);
    }

    // One cell column is one pixel wide; one cell row is two pixel rows
    // (half-blocks), so the pixel canvas is width x 2*height.
    let canvas_h = u32::from(area.height) * 2;
    if u32::from(grid_w) > u32::from(area.width) || u32::from(GLYPH_HEIGHT) > canvas_h {
        return false;
    }
    let scale =
        (u32::from(area.width) / u32::from(grid_w)).min(canvas_h / u32::from(GLYPH_HEIGHT));
    let scaled_w = u32::from(grid_w) * scale;
    let scaled_h = u32::from(GLYPH_HEIGHT) * scale;
    let off_x = (u32::from(area.width) - scaled_w) / 2;
    // Keep the vertical offset even so cell rows align with pixel pairs.
    let off_y = ((canvas_h - scaled_h) / 2) & !1;

    let pixel_on = |px: u32, py: u32| -> bool {
        if px < off_x || py < off_y {
            return false;
        }
        let gx = ((px - off_x) / scale) as u16;
        let gy = ((py - off_y) / scale) as usize;
        if gx >= grid_w || gy >= GLYPH_HEIGHT as usize {
            return false;
        }
        let mut idx = 0;
        let mut rel = 0;
        for i in (0..chars.len()).rev() {
            if gx >= starts[i] {
                idx = i;
                rel = gx - starts[i];
                break;
            }
        }
        glyph_pixel(chars[idx], colon_on, rel, gy)
    };

    for r in 0..area.height {
        let py_top = u32::from(r) * 2;
        let row: String = (0..u32::from(area.width))
            .map(|px| {
                let (top, bottom) = (pixel_on(px, py_top), pixel_on(px, py_top + 1));
                half_char(top, bottom)
            })
            .collect();
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(row, style))),
            Rect::new(area.x, area.y.saturating_add(r), area.width, 1),
        );
    }
    true
}

fn draw_clock(frame: &mut Frame, area: Rect, local_now: chrono::DateTime<Local>, colon_on: bool) {
    // Giant hours:minutes stretched to fill the whole clock area.
    let time = local_now.format("%-I:%M").to_string();
    if render_big_digits(frame, area, &time, colon_on, Style::default()) {
        return;
    }
    let clock = Paragraph::new(Line::from(Span::styled(
        local_now.format("%-I:%M:%S %p").to_string(),
        Style::default(),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(clock, area);
}

/// Seconds in block digits with the AM/PM tag beside them.
fn draw_seconds(frame: &mut Frame, area: Rect, local_now: chrono::DateTime<Local>) {
    let seconds = local_now.format("%S").to_string();
    if !render_big_digits(frame, area, &seconds, true, Style::default()) {
        frame.render_widget(
            Paragraph::new(local_now.format("%S %p").to_string()).alignment(Alignment::Center),
            area,
        );
        return;
    }
    if area.width >= 24 {
        let period = Paragraph::new(Line::from(Span::styled(
            local_now.format("%p").to_string(),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )))
        .alignment(Alignment::Right);
        frame.render_widget(
            period,
            Rect::new(area.x, area.y.saturating_add(area.height / 2), area.width, 1),
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

/// Half-block character for a pixel pair (top, bottom) within one cell.
fn half_char(top: bool, bottom: bool) -> char {
    match (top, bottom) {
        (true, true) => '█',
        (true, false) => '▀',
        (false, true) => '▄',
        _ => ' ',
    }
}

fn draw_countdown(
    frame: &mut Frame,
    label: Rect,
    digits: Rect,
    next_name: &str,
    until_ms: i64,
    colon_on: bool,
) {
    let text = Paragraph::new(Line::from(Span::styled(
        format!("Until {next_name}").to_uppercase(),
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
    )))
    .alignment(Alignment::Center);
    frame.render_widget(text, label);
    let style = Style::default().fg(Color::Green).add_modifier(Modifier::BOLD);
    if !render_big_digits(frame, digits, &format_hms(until_ms), colon_on, style) {
        frame.render_widget(
            Paragraph::new(format_hms(until_ms)).style(style).alignment(Alignment::Center),
            digits,
        );
    }
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
        if !render_big_digits(frame, inner, &time, true, time_style) {
            let row_y = inner.y + inner.height.saturating_sub(1) / 2;
            let body = Paragraph::new(Span::styled(time.clone(), time_style))
                .alignment(Alignment::Center);
            frame.render_widget(body, Rect::new(inner.x, row_y, inner.width, 1));
        }
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

    let colon_on = local_now.timestamp() % 2 == 0;
    let data = Layout::vertical([
        Constraint::Min(7),    // giant clock, fills the remaining height
        Constraint::Length(4), // seconds + AM/PM
        Constraint::Length(1), // hijri / date line
        Constraint::Length(1), // countdown label
        Constraint::Length(7), // countdown block digits
        Constraint::Length(1), // spacer
        Constraint::Length(4), // qiyam card
        Constraint::Length(1), // footer (left column only)
    ])
    .split(columns[0]);

    let prayers = Layout::vertical([
        Constraint::Length(1), // "Prayer times" heading
        Constraint::Min(6),    // six stacked cards, full remaining height
    ])
    .split(columns[1]);

    draw_clock(frame, data[0], local_now, colon_on);
    draw_seconds(frame, data[1], local_now);
    draw_date(frame, data[2], local_now);
    draw_countdown(frame, data[3], data[4], &next_name, until_ms, colon_on);

    match calculate_night_thirds(lat, lng, date, &cfg.method, &cfg.madhab, now) {
        Some(thirds) => draw_qiyam_card(frame, data[6], &thirds),
        None => {
            let label = Paragraph::new(Span::styled(
                "Qiyam — night thirds unavailable",
                Style::default().fg(Color::DarkGray),
            ))
            .alignment(Alignment::Center)
            .block(Block::bordered().border_style(Style::default().fg(Color::DarkGray)));
            frame.render_widget(label, data[6]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    #[test]
    fn big_digits_scale_to_fill_area() {
        let mut terminal = Terminal::new(TestBackend::new(60, 8)).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                assert!(render_big_digits(f, area, "12:34", true, Style::default()));
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        let blocks = buf.content.iter().filter(|c| c.symbol() == "█").count();
        assert!(blocks > 100, "expected large scaled glyphs, got {blocks} blocks");
    }

    #[test]
    fn big_digits_fall_back_when_too_small() {
        let mut terminal = Terminal::new(TestBackend::new(20, 3)).unwrap();
        terminal
            .draw(|f| {
                // A 4x2 area cannot hold the 5x7 glyph grid.
                assert!(!render_big_digits(
                    f,
                    Rect::new(0, 0, 4, 2),
                    "12:34",
                    true,
                    Style::default()
                ));
            })
            .unwrap();
    }

    #[test]
    fn big_digits_blink_colon_off() {
        let mut terminal = Terminal::new(TestBackend::new(40, 7)).unwrap();
        terminal
            .draw(|f| {
                let area = f.area();
                assert!(render_big_digits(f, area, "7:05", false, Style::default()));
            })
            .unwrap();
        let buf = terminal.backend().buffer().clone();
        // With the colon off its grid columns stay blank.
        let blank_col = (4..8).all(|x| {
            (0..7).all(|y| {
                buf.content[(y as usize) * 40 + x as usize].symbol() == " "
            })
        });
        assert!(blank_col, "colon column should be blank when blinked off");
    }
}
