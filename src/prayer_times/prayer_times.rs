//! The core prayer time calculation.
//!
//! Port of `PrayerTimes.js`, `DateUtils.js`, `TimeComponents.js`,
//! `Prayer.js` and `PolarCircleResolution.js` from the adhan npm package.
//!
//! Unlike the JS library (which returns `Date` objects that may be invalid
//! at extreme latitudes), uncomputable times are represented as `None`.

use chrono::{DateTime, Datelike, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use super::calculation_parameters::{
    CalculationParameters, PolarCircleResolution, Rounding,
};
use super::coordinates::Coordinates;
use super::solar_time::{
    SolarTime, season_adjusted_evening_twilight, season_adjusted_morning_twilight,
};

/// The five daily prayers plus sunrise (not a prayer, but needed for
/// high-latitude calculations and display).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Prayer {
    Fajr,
    Sunrise,
    Dhuhr,
    Asr,
    Maghrib,
    Isha,
    None,
}

// ---------------------------------------------------------------------------
// Date utilities (DateUtils.js / TimeComponents.js)
// ---------------------------------------------------------------------------

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

/// Milliseconds since the Unix epoch at 00:00:00 UTC for the given date.
fn utc_midnight_ms(year: i32, month: u32, day: u32) -> f64 {
    (days_from_civil(year, month, day) * 86_400_000) as f64
}

/// Port of `TimeComponents`: splits a fractional hour value into
/// h/m/s and combines it with the given UTC date, returning milliseconds
/// since the Unix epoch. NaN input yields NaN (an "Invalid Date" in JS).
fn time_components_to_utc_ms(year: i32, month: u32, day: u32, num: f64) -> f64 {
    if num.is_nan() {
        return f64::NAN;
    }
    let hours = num.floor();
    let minutes = ((num - hours) * 60.0).floor();
    let seconds = ((num - (hours + minutes / 60.0)) * 60.0 * 60.0).floor();
    utc_midnight_ms(year, month, day) + (hours * 3600.0 + minutes * 60.0 + seconds) * 1000.0
}

/// The seconds component (0..59) of a UTC timestamp in milliseconds,
/// mirroring `Date.getUTCSeconds()`.
fn utc_seconds(ms: f64) -> f64 {
    if ms.is_nan() {
        return f64::NAN;
    }
    (ms / 1000.0).floor().rem_euclid(60.0)
}

/// Port of `roundedMinute`.
fn rounded_minute_ms(ms: f64, rounding: Rounding) -> f64 {
    let seconds = utc_seconds(ms);
    let offset = match rounding {
        Rounding::Nearest => {
            if seconds >= 30.0 {
                60.0 - seconds
            } else {
                -seconds
            }
        }
        Rounding::Up => 60.0 - seconds,
        Rounding::None => 0.0,
    };
    ms + offset * 1000.0
}

fn ms_to_datetime(ms: f64) -> Option<DateTime<Utc>> {
    if ms.is_nan() {
        None
    } else {
        DateTime::from_timestamp_millis(ms.trunc() as i64)
    }
}

// ---------------------------------------------------------------------------
// Polar circle resolution (PolarCircleResolution.js)
// ---------------------------------------------------------------------------

const LATITUDE_VARIATION_STEP: f64 = 0.5;
const UNSAFE_LATITUDE: f64 = 65.0;

fn is_valid_solar_time(solar_time: &SolarTime) -> bool {
    !solar_time.sunrise.is_nan() && !solar_time.sunset.is_nan()
}

fn date_plus_days(date: NaiveDate, days: i64) -> NaiveDate {
    date + chrono::Duration::days(days)
}

/// Port of `aqrabYaumResolver`. Returns the solar times for the nearest date
/// with valid sunrise/sunset.
fn aqrab_yaum_resolver(
    coordinates: Coordinates,
    date: NaiveDate,
    days_added: i64,
    direction: i64,
) -> Option<(SolarTime, SolarTime)> {
    if days_added > (365.0_f64 / 2.0).ceil() as i64 {
        return None;
    }

    let test_date = date_plus_days(date, direction * days_added);
    let tomorrow = date_plus_days(test_date, 1);
    let solar_time = SolarTime::new(
        test_date.year(),
        test_date.month(),
        test_date.day(),
        coordinates,
    );
    let tomorrow_solar_time = SolarTime::new(
        tomorrow.year(),
        tomorrow.month(),
        tomorrow.day(),
        coordinates,
    );

    if !is_valid_solar_time(&solar_time) || !is_valid_solar_time(&tomorrow_solar_time) {
        return aqrab_yaum_resolver(
            coordinates,
            date,
            days_added + if direction > 0 { 0 } else { 1 },
            -direction,
        );
    }

    Some((solar_time, tomorrow_solar_time))
}

/// Port of `aqrabBaladResolver`. Steps the latitude toward the equator until
/// sunrise/sunset are valid.
fn aqrab_balad_resolver(
    coordinates: Coordinates,
    date: NaiveDate,
    latitude: f64,
) -> Option<(SolarTime, SolarTime)> {
    let test_coordinates = Coordinates::new(latitude, coordinates.longitude);
    let tomorrow = date_plus_days(date, 1);
    let solar_time = SolarTime::new(date.year(), date.month(), date.day(), test_coordinates);
    let tomorrow_solar_time = SolarTime::new(
        tomorrow.year(),
        tomorrow.month(),
        tomorrow.day(),
        test_coordinates,
    );

    if !is_valid_solar_time(&solar_time) || !is_valid_solar_time(&tomorrow_solar_time) {
        if latitude.abs() >= UNSAFE_LATITUDE {
            return aqrab_balad_resolver(
                coordinates,
                date,
                latitude - latitude.signum() * LATITUDE_VARIATION_STEP,
            );
        }
        return None;
    }

    Some((solar_time, tomorrow_solar_time))
}

/// Port of `polarCircleResolvedValues`.
fn polar_circle_resolved_values(
    resolver: PolarCircleResolution,
    date: NaiveDate,
    coordinates: Coordinates,
) -> (SolarTime, SolarTime) {
    let tomorrow = date_plus_days(date, 1);
    let default_return = || {
        (
            SolarTime::new(date.year(), date.month(), date.day(), coordinates),
            SolarTime::new(
                tomorrow.year(),
                tomorrow.month(),
                tomorrow.day(),
                coordinates,
            ),
        )
    };

    match resolver {
        PolarCircleResolution::AqrabYaum => {
            aqrab_yaum_resolver(coordinates, date, 1, 1).unwrap_or_else(default_return)
        }
        PolarCircleResolution::AqrabBalad => aqrab_balad_resolver(
            coordinates,
            date,
            coordinates.latitude - coordinates.latitude.signum() * LATITUDE_VARIATION_STEP,
        )
        .unwrap_or_else(default_return),
        PolarCircleResolution::Unresolved => default_return(),
    }
}

// ---------------------------------------------------------------------------
// PrayerTimes.js
// ---------------------------------------------------------------------------

/// The prayer times for a given date, coordinates and calculation parameters.
///
/// Fields are `Option<DateTime<Utc>>`: `None` means the time could not be
/// computed for that location/date (the JS library returns an Invalid Date
/// in that case, e.g. Fajr/Isha inside the polar circle with the default
/// `Unresolved` polar circle resolution).
#[derive(Debug, Clone)]
pub struct PrayerTimes {
    pub coordinates: Coordinates,
    pub date: NaiveDate,
    pub calculation_parameters: CalculationParameters,
    pub fajr: Option<DateTime<Utc>>,
    pub sunrise: Option<DateTime<Utc>>,
    pub dhuhr: Option<DateTime<Utc>>,
    pub asr: Option<DateTime<Utc>>,
    pub sunset: Option<DateTime<Utc>>,
    pub maghrib: Option<DateTime<Utc>>,
    pub isha: Option<DateTime<Utc>>,
}

impl PrayerTimes {
    /// Port of the `PrayerTimes` constructor. `date` is the calendar day the
    /// times are computed for (the JS library uses the date's local
    /// year/month/day components; pass the equivalent calendar date here).
    pub fn new(
        coordinates: Coordinates,
        date: NaiveDate,
        calculation_parameters: CalculationParameters,
    ) -> Self {
        let (year, month, day) = (date.year(), date.month(), date.day());
        let tomorrow = date_plus_days(date, 1);
        let day_of_year = date.ordinal();

        let mut solar_time = SolarTime::new(year, month, day, coordinates);
        let mut tomorrow_solar_time = SolarTime::new(
            tomorrow.year(),
            tomorrow.month(),
            tomorrow.day(),
            coordinates,
        );

        let mut dhuhr_time = time_components_to_utc_ms(year, month, day, solar_time.transit);
        let mut sunrise_time = time_components_to_utc_ms(year, month, day, solar_time.sunrise);
        let mut sunset_time = time_components_to_utc_ms(year, month, day, solar_time.sunset);

        if (sunrise_time.is_nan()
            || sunset_time.is_nan()
            || tomorrow_solar_time.sunrise.is_nan())
            && calculation_parameters.polar_circle_resolution != PolarCircleResolution::Unresolved
        {
            let resolved = polar_circle_resolved_values(
                calculation_parameters.polar_circle_resolution,
                date,
                coordinates,
            );
            solar_time = resolved.0;
            tomorrow_solar_time = resolved.1;
            dhuhr_time = time_components_to_utc_ms(year, month, day, solar_time.transit);
            sunrise_time = time_components_to_utc_ms(year, month, day, solar_time.sunrise);
            sunset_time = time_components_to_utc_ms(year, month, day, solar_time.sunset);
        }

        let asr_time = time_components_to_utc_ms(
            year,
            month,
            day,
            solar_time.afternoon(calculation_parameters.madhab.shadow_length()),
        );
        let tomorrow_sunrise = time_components_to_utc_ms(
            tomorrow.year(),
            tomorrow.month(),
            tomorrow.day(),
            tomorrow_solar_time.sunrise,
        );
        let night = (tomorrow_sunrise - sunset_time) / 1000.0;

        let mut fajr_time = time_components_to_utc_ms(
            year,
            month,
            day,
            solar_time.hour_angle(-calculation_parameters.fajr_angle, false),
        );

        // special case for moonsighting committee above latitude 55
        if calculation_parameters.is_moonsighting_committee() && coordinates.latitude >= 55.0 {
            let night_fraction = night / 7.0;
            fajr_time = sunrise_time - night_fraction * 1000.0;
        }

        let safe_fajr = if calculation_parameters.is_moonsighting_committee() {
            season_adjusted_morning_twilight(
                coordinates.latitude,
                day_of_year,
                year,
                sunrise_time,
            )
        } else {
            let (fajr_portion, _) = calculation_parameters.night_portions();
            let night_fraction = fajr_portion * night;
            sunrise_time - night_fraction * 1000.0
        };

        if fajr_time.is_nan() || safe_fajr > fajr_time {
            fajr_time = safe_fajr;
        }

        let isha_time = if calculation_parameters.isha_interval > 0 {
            sunset_time + (calculation_parameters.isha_interval * 60) as f64 * 1000.0
        } else {
            let mut isha_time = time_components_to_utc_ms(
                year,
                month,
                day,
                solar_time.hour_angle(-calculation_parameters.isha_angle, true),
            );

            // special case for moonsighting committee above latitude 55
            if calculation_parameters.is_moonsighting_committee() && coordinates.latitude >= 55.0
            {
                let night_fraction = night / 7.0;
                isha_time = sunset_time + night_fraction * 1000.0;
            }

            let safe_isha = if calculation_parameters.is_moonsighting_committee() {
                season_adjusted_evening_twilight(
                    coordinates.latitude,
                    day_of_year,
                    year,
                    sunset_time,
                    calculation_parameters.shafaq,
                )
            } else {
                let (_, isha_portion) = calculation_parameters.night_portions();
                let night_fraction = isha_portion * night;
                sunset_time + night_fraction * 1000.0
            };

            if isha_time.is_nan() || safe_isha < isha_time {
                isha_time = safe_isha;
            }

            isha_time
        };

        let mut maghrib_time = sunset_time;

        // JS: `if (calculationParameters.maghribAngle)` — truthy, so 0 and NaN skip.
        if calculation_parameters.maghrib_angle != 0.0
            && !calculation_parameters.maghrib_angle.is_nan()
        {
            let angle_based_maghrib = time_components_to_utc_ms(
                year,
                month,
                day,
                solar_time.hour_angle(-calculation_parameters.maghrib_angle, true),
            );

            if sunset_time < angle_based_maghrib && isha_time > angle_based_maghrib {
                maghrib_time = angle_based_maghrib;
            }
        }

        let adj = calculation_parameters.adjustments;
        let madj = calculation_parameters.method_adjustments;
        let fajr_adjustment = (adj.fajr + madj.fajr) as f64;
        let sunrise_adjustment = (adj.sunrise + madj.sunrise) as f64;
        let dhuhr_adjustment = (adj.dhuhr + madj.dhuhr) as f64;
        let asr_adjustment = (adj.asr + madj.asr) as f64;
        let maghrib_adjustment = (adj.maghrib + madj.maghrib) as f64;
        let isha_adjustment = (adj.isha + madj.isha) as f64;

        let rounding = calculation_parameters.rounding;
        let fajr = rounded_minute_ms(fajr_time + fajr_adjustment * 60_000.0, rounding);
        let sunrise = rounded_minute_ms(sunrise_time + sunrise_adjustment * 60_000.0, rounding);
        let dhuhr = rounded_minute_ms(dhuhr_time + dhuhr_adjustment * 60_000.0, rounding);
        let asr = rounded_minute_ms(asr_time + asr_adjustment * 60_000.0, rounding);
        let sunset = rounded_minute_ms(sunset_time, rounding);
        let maghrib = rounded_minute_ms(maghrib_time + maghrib_adjustment * 60_000.0, rounding);
        let isha = rounded_minute_ms(isha_time + isha_adjustment * 60_000.0, rounding);

        PrayerTimes {
            coordinates,
            date,
            calculation_parameters,
            fajr: ms_to_datetime(fajr),
            sunrise: ms_to_datetime(sunrise),
            dhuhr: ms_to_datetime(dhuhr),
            asr: ms_to_datetime(asr),
            sunset: ms_to_datetime(sunset),
            maghrib: ms_to_datetime(maghrib),
            isha: ms_to_datetime(isha),
        }
    }

    pub fn time_for_prayer(&self, prayer: Prayer) -> Option<DateTime<Utc>> {
        match prayer {
            Prayer::Fajr => self.fajr,
            Prayer::Sunrise => self.sunrise,
            Prayer::Dhuhr => self.dhuhr,
            Prayer::Asr => self.asr,
            Prayer::Maghrib => self.maghrib,
            Prayer::Isha => self.isha,
            Prayer::None => None,
        }
    }

    pub fn current_prayer(&self, date: DateTime<Utc>) -> Prayer {
        // JS compares with `>=`; an Invalid Date (our `None`) always compares false.
        let ge = |t: Option<DateTime<Utc>>| t.is_some_and(|t| date >= t);
        if ge(self.isha) {
            Prayer::Isha
        } else if ge(self.maghrib) {
            Prayer::Maghrib
        } else if ge(self.asr) {
            Prayer::Asr
        } else if ge(self.dhuhr) {
            Prayer::Dhuhr
        } else if ge(self.sunrise) {
            Prayer::Sunrise
        } else if ge(self.fajr) {
            Prayer::Fajr
        } else {
            Prayer::None
        }
    }

    pub fn next_prayer(&self, date: DateTime<Utc>) -> Prayer {
        let ge = |t: Option<DateTime<Utc>>| t.is_some_and(|t| date >= t);
        if ge(self.isha) {
            Prayer::None
        } else if ge(self.maghrib) {
            Prayer::Isha
        } else if ge(self.asr) {
            Prayer::Maghrib
        } else if ge(self.dhuhr) {
            Prayer::Asr
        } else if ge(self.sunrise) {
            Prayer::Dhuhr
        } else if ge(self.fajr) {
            Prayer::Sunrise
        } else {
            Prayer::Fajr
        }
    }
}
