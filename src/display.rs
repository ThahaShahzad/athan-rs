//! Shared display helpers: method labels, prayer info, time/countdown
//! formatting, and the prayer-times wrapper around the math core.
//!
//! Port of the display-side of `prayer-times.js` from the Node athan-cli.

use chrono::{DateTime, Local, NaiveDate, Utc};

use athan::prayer_times::{
    CalculationMethod, Coordinates, Madhab, Prayer, PrayerTimes,
};

/// A prayer row, in display order.
pub struct PrayerInfo {
    pub key: &'static str,
    pub en: &'static str,
}

pub const PRAYER_INFO: &[PrayerInfo] = &[
    PrayerInfo { key: "fajr", en: "Fajr" },
    PrayerInfo { key: "sunrise", en: "Sunrise" },
    PrayerInfo { key: "dhuhr", en: "Dhuhr" },
    PrayerInfo { key: "asr", en: "Asr" },
    PrayerInfo { key: "maghrib", en: "Maghrib" },
    PrayerInfo { key: "isha", en: "Isha" },
];

/// Prayers that get athan (exclude sunrise).
pub const ATHAN_PRAYERS: &[&str] = &["fajr", "dhuhr", "asr", "maghrib", "isha"];

pub fn prayer_name(key: &str) -> &str {
    PRAYER_INFO
        .iter()
        .find(|p| p.key == key)
        .map(|p| p.en)
        .unwrap_or(key)
}

pub fn method_label(method: &str) -> &str {
    match method {
        "NorthAmerica" => "ISNA (North America)",
        "MuslimWorldLeague" => "Muslim World League",
        "Egyptian" => "Egyptian General Authority",
        "UmmAlQura" => "Umm Al-Qura (Saudi)",
        "Karachi" => "University of Islamic Sciences, Karachi",
        "Dubai" => "Dubai",
        "Kuwait" => "Kuwait",
        "Qatar" => "Qatar",
        "Singapore" => "Singapore",
        "Tehran" => "Institute of Geophysics, Tehran",
        "Turkey" => "Diyanet (Turkey)",
        "MoonsightingCommittee" => "Moonsighting Committee",
        other => other,
    }
}

/// The 12 method keys accepted by `config set method ...`, in the same order
/// as the Node CLI's error message.
pub const METHOD_KEYS: &[&str] = &[
    "NorthAmerica",
    "MuslimWorldLeague",
    "Egyptian",
    "UmmAlQura",
    "Karachi",
    "Dubai",
    "Kuwait",
    "Qatar",
    "Singapore",
    "Tehran",
    "Turkey",
    "MoonsightingCommittee",
];

/// Calculate prayer times for a given location and local calendar date,
/// mirroring `calculatePrayerTimes` in the Node CLI (unknown methods fall
/// back to NorthAmerica).
pub fn calculate_prayer_times(
    lat: f64,
    lng: f64,
    date: NaiveDate,
    method: &str,
    madhab: &str,
) -> PrayerTimes {
    let method = CalculationMethod::from_name(method).unwrap_or(CalculationMethod::NorthAmerica);
    let mut params = method.parameters();
    params.madhab = if madhab == "Hanafi" {
        Madhab::Hanafi
    } else {
        Madhab::Shafi
    };
    PrayerTimes::new(Coordinates::new(lat, lng), date, params)
}

/// The six display times keyed like the JS object.
pub fn time_by_key(times: &PrayerTimes, key: &str) -> Option<DateTime<Utc>> {
    match key {
        "fajr" => times.fajr,
        "sunrise" => times.sunrise,
        "dhuhr" => times.dhuhr,
        "asr" => times.asr,
        "maghrib" => times.maghrib,
        "isha" => times.isha,
        _ => None,
    }
}

/// Current/next prayer as string keys ("none" like the JS Prayer.None).
pub fn current_and_next(times: &PrayerTimes, now: DateTime<Utc>) -> (String, String) {
    let to_key = |p: Prayer| match p {
        Prayer::Fajr => "fajr",
        Prayer::Sunrise => "sunrise",
        Prayer::Dhuhr => "dhuhr",
        Prayer::Asr => "asr",
        Prayer::Maghrib => "maghrib",
        Prayer::Isha => "isha",
        Prayer::None => "none",
    };
    (
        to_key(times.current_prayer(now)).to_string(),
        to_key(times.next_prayer(now)).to_string(),
    )
}

/// Milliseconds until the next prayer, or None after Isha (port of
/// `getTimeUntilNext`).
pub fn time_until_next(times: &PrayerTimes, now: DateTime<Utc>) -> Option<i64> {
    let (_, next) = current_and_next(times, now);
    if next == "none" {
        return None;
    }
    time_by_key(times, &next).map(|t| (t - now).num_milliseconds())
}

/// Format a UTC instant as a local 12-hour time ("5:41 AM"), like the Node
/// `formatTime`. Uncomputable times render as `--:--`.
pub fn format_time(time: Option<DateTime<Utc>>) -> String {
    match time {
        Some(t) => t
            .with_timezone(&Local)
            .format("%-I:%M %p")
            .to_string(),
        None => "--:--".to_string(),
    }
}

/// Format milliseconds into a human-readable countdown (port of
/// `formatCountdown`).
pub fn format_countdown(ms: i64) -> String {
    if ms < 0 {
        return String::new();
    }
    let total_seconds = ms / 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

// ---------------------------------------------------------------------------
// Night thirds (Qiyam) — port of calculateNightThirds from the Node CLI.
// ---------------------------------------------------------------------------

pub struct NightThird {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

/// Full result of the night-thirds calculation. Only the Qiyam (third3)
/// window and `active` are currently displayed, but the full shape mirrors
/// the JS return value for future use.
#[allow(dead_code)]
pub struct NightThirds {
    pub night_start: DateTime<Utc>,
    pub night_end: DateTime<Utc>,
    pub third1: NightThird,
    pub third2: NightThird,
    /// The last third — the recommended time for Qiyam (Tahajjud).
    pub third3: NightThird,
    /// Active third (1, 2, 3) or None if outside the night window.
    pub active: Option<u8>,
    /// Progress through the active third (0..1), 0 if none.
    pub progress: f64,
}

/// Calculate the three thirds of the night (Maghrib → next Fajr) containing
/// `now`, or the upcoming one during daytime. Returns None when the relevant
/// Maghrib/Fajr are uncomputable (extreme latitudes).
pub fn calculate_night_thirds(
    lat: f64,
    lng: f64,
    date: NaiveDate,
    method: &str,
    madhab: &str,
    now: DateTime<Utc>,
) -> Option<NightThirds> {
    let yesterday = date - chrono::Duration::days(1);
    let tomorrow = date + chrono::Duration::days(1);

    let today_times = calculate_prayer_times(lat, lng, date, method, madhab);
    let yesterday_times = calculate_prayer_times(lat, lng, yesterday, method, madhab);
    let tomorrow_times = calculate_prayer_times(lat, lng, tomorrow, method, madhab);

    // Two possible night windows: yesterday-maghrib → today-fajr (early
    // morning) and today-maghrib → tomorrow-fajr (evening/night).
    let window_a = (yesterday_times.maghrib, today_times.fajr);
    let window_b = (today_times.maghrib, tomorrow_times.fajr);

    let contains = |win: &(Option<DateTime<Utc>>, Option<DateTime<Utc>>)| match win {
        (Some(start), Some(end)) => now >= *start && now < *end,
        _ => false,
    };

    // Pick the window containing `now`, otherwise the next-upcoming one.
    let (start, end) = if contains(&window_a) {
        (window_a.0?, window_a.1?)
    } else {
        (window_b.0?, window_b.1?)
    };

    let total_ms = (end - start).num_milliseconds() as f64;
    let third_ms = total_ms / 3.0;

    let at = |offset_ms: f64| start + chrono::Duration::milliseconds(offset_ms as i64);
    let third1_start = start;
    let third1_end = at(third_ms);
    let third2_start = third1_end;
    let third2_end = at(third_ms * 2.0);
    let third3_start = third2_end;
    let third3_end = end;

    let (active, progress) = if now >= third1_start && now < third2_start {
        (
            Some(1),
            (now - third1_start).num_milliseconds() as f64
                / (third2_start - third1_start).num_milliseconds() as f64,
        )
    } else if now >= third2_start && now < third3_start {
        (
            Some(2),
            (now - third2_start).num_milliseconds() as f64
                / (third3_start - third2_start).num_milliseconds() as f64,
        )
    } else if now >= third3_start && now < third3_end {
        (
            Some(3),
            (now - third3_start).num_milliseconds() as f64
                / (third3_end - third3_start).num_milliseconds() as f64,
        )
    } else {
        (None, 0.0)
    };

    Some(NightThirds {
        night_start: start,
        night_end: end,
        third1: NightThird { start: third1_start, end: third1_end },
        third2: NightThird { start: third2_start, end: third2_end },
        third3: NightThird { start: third3_start, end: third3_end },
        active,
        progress: progress.clamp(0.0, 1.0),
    })
}
