//! Approximate Hijri (Islamic) date conversion.
//!
//! There is no `Intl` in Rust, so this implements the tabular (arithmetic)
//! Islamic calendar with the civil epoch — the standard "Kuwaiti algorithm"
//! converting a Julian day number to Islamic year/month/day. It can differ
//! by ±1 day from the official Umm al-Qura calendar (which is based on actual
//! moon sighting), so the UI labels it as approximate.

use chrono::{Datelike, NaiveDate};

const MONTH_NAMES: [&str; 12] = [
    "Muharram",
    "Safar",
    "Rabiʻ I",
    "Rabiʻ II",
    "Jumada I",
    "Jumada II",
    "Rajab",
    "Shaʻban",
    "Ramadan",
    "Shawwal",
    "Dhuʻl-Qiʻdah",
    "Dhuʻl-Hijjah",
];

/// Days since the Unix epoch for a Gregorian calendar date
/// (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Convert a Gregorian date to a tabular Islamic (Hijri) date.
/// Returns (year, month 1-12, day).
pub fn hijri_from_gregorian(date: NaiveDate) -> (i64, i64, i64) {
    // Julian day number: 1970-01-01 is JDN 2440588.
    let jdn = days_from_civil(date.year(), date.month(), date.day()) + 2_440_588;

    // Kuwaiti algorithm (civil epoch, JDN 1948440 = 16 July 622 Julian).
    let mut jd = jdn - 1_948_440 + 10_632;
    let n = (jd - 1).div_euclid(10_631);
    jd = jd - 10_631 * n + 354;
    let j = ((10_985 - jd) / 5_316) * ((50 * jd) / 17_719) + (jd / 5_670) * ((43 * jd) / 15_238);
    jd = jd - ((30 - j) / 15) * ((17_719 * j) / 50) - (j / 16) * ((15_238 * j) / 43) + 29;
    let month = (24 * jd) / 709;
    let day = jd - (709 * month) / 24;
    let year = 30 * n + j - 30;

    (year, month, day)
}

/// Format like the Node TUI's `Intl.DateTimeFormat('en-u-ca-islamic-umalqura',
/// { day: 'numeric', month: 'long', year: 'numeric' })`, e.g.
/// "22 Safar 1448 AH" — with an "(approx.)" marker since this is the tabular
/// calendar, not Umm al-Qura.
pub fn format_hijri(date: NaiveDate) -> String {
    let (year, month, day) = hijri_from_gregorian(date);
    let month_name = MONTH_NAMES[(month.clamp(1, 12) - 1) as usize];
    format!("{day} {month_name} {year} AH (approx.)")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        // 1 Ramadan 1445 fell on 2024-03-11 in the tabular civil calendar.
        assert_eq!(
            hijri_from_gregorian(NaiveDate::from_ymd_opt(2024, 3, 11).unwrap()),
            (1445, 9, 1)
        );
        // Islamic epoch: 1 Muharram 1 = 622-07-19 Gregorian.
        assert_eq!(
            hijri_from_gregorian(NaiveDate::from_ymd_opt(622, 7, 19).unwrap()),
            (1, 1, 1)
        );
    }
}
