//! Solar position math: math utilities, astronomical formulas, solar
//! coordinates and solar time.
//!
//! Port of `MathUtils.js`, `Astronomical.js`, `SolarCoordinates.js` and
//! `SolarTime.js` from the adhan npm package. All formulas are from
//! *Astronomical Algorithms* by Jean Meeus, exactly as in the JS source.

use super::coordinates::Coordinates;
use super::calculation_parameters::Shafaq;

// ---------------------------------------------------------------------------
// MathUtils.js
// ---------------------------------------------------------------------------

pub fn degrees_to_radians(degrees: f64) -> f64 {
    degrees * std::f64::consts::PI / 180.0
}

pub fn radians_to_degrees(radians: f64) -> f64 {
    radians * 180.0 / std::f64::consts::PI
}

pub fn normalize_to_scale(num: f64, max: f64) -> f64 {
    num - max * (num / max).floor()
}

pub fn unwind_angle(angle: f64) -> f64 {
    normalize_to_scale(angle, 360.0)
}

/// `Math.round` in JavaScript rounds half towards positive infinity;
/// Rust's `f64::round` rounds half away from zero. This matches JS.
fn js_round(x: f64) -> f64 {
    (x + 0.5).floor()
}

pub fn quadrant_shift_angle(angle: f64) -> f64 {
    if (-180.0..=180.0).contains(&angle) {
        angle
    } else {
        angle - 360.0 * js_round(angle / 360.0)
    }
}

// ---------------------------------------------------------------------------
// Astronomical.js
// ---------------------------------------------------------------------------

/// The geometric mean longitude of the sun in degrees.
pub fn mean_solar_longitude(julian_century: f64) -> f64 {
    let t = julian_century;
    // Equation from Astronomical Algorithms page 163
    let term1 = 280.4664567;
    let term2 = 36000.76983 * t;
    let term3 = 0.0003032 * t.powi(2);
    unwind_angle(term1 + term2 + term3)
}

/// The geometric mean longitude of the moon in degrees.
pub fn mean_lunar_longitude(julian_century: f64) -> f64 {
    let t = julian_century;
    // Equation from Astronomical Algorithms page 144
    let term1 = 218.3165;
    let term2 = 481267.8813 * t;
    unwind_angle(term1 + term2)
}

pub fn ascending_lunar_node_longitude(julian_century: f64) -> f64 {
    let t = julian_century;
    // Equation from Astronomical Algorithms page 144
    let term1 = 125.04452;
    let term2 = 1934.136261 * t;
    let term3 = 0.0020708 * t.powi(2);
    let term4 = t.powi(3) / 450000.0;
    unwind_angle(term1 - term2 + term3 + term4)
}

/// The mean anomaly of the sun.
pub fn mean_solar_anomaly(julian_century: f64) -> f64 {
    let t = julian_century;
    // Equation from Astronomical Algorithms page 163
    let term1 = 357.52911;
    let term2 = 35999.05029 * t;
    let term3 = 0.0001537 * t.powi(2);
    unwind_angle(term1 + term2 - term3)
}

/// The Sun's equation of the center in degrees.
pub fn solar_equation_of_the_center(julian_century: f64, mean_anomaly: f64) -> f64 {
    let t = julian_century;
    // Equation from Astronomical Algorithms page 164
    let mrad = degrees_to_radians(mean_anomaly);
    let term1 = (1.914602 - 0.004817 * t - 0.000014 * t.powi(2)) * mrad.sin();
    let term2 = (0.019993 - 0.000101 * t) * (2.0 * mrad).sin();
    let term3 = 0.000289 * (3.0 * mrad).sin();
    term1 + term2 + term3
}

/// The apparent longitude of the Sun, referred to the true equinox of the date.
pub fn apparent_solar_longitude(julian_century: f64, mean_longitude: f64) -> f64 {
    let t = julian_century;
    let l0 = mean_longitude;
    // Equation from Astronomical Algorithms page 164
    let longitude = l0 + solar_equation_of_the_center(t, mean_solar_anomaly(t));
    let omega = 125.04 - 1934.136 * t;
    let lambda = longitude - 0.00569 - 0.00478 * degrees_to_radians(omega).sin();
    unwind_angle(lambda)
}

/// The mean obliquity of the ecliptic, formula adopted by the IAU, in degrees.
pub fn mean_obliquity_of_the_ecliptic(julian_century: f64) -> f64 {
    let t = julian_century;
    // Equation from Astronomical Algorithms page 147
    let term1 = 23.439291;
    let term2 = 0.013004167 * t;
    let term3 = 0.0000001639 * t.powi(2);
    let term4 = 0.0000005036 * t.powi(3);
    term1 - term2 - term3 + term4
}

/// The mean obliquity of the ecliptic, corrected for calculating the apparent
/// position of the sun, in degrees.
pub fn apparent_obliquity_of_the_ecliptic(
    julian_century: f64,
    mean_obliquity_of_the_ecliptic: f64,
) -> f64 {
    let t = julian_century;
    let epsilon0 = mean_obliquity_of_the_ecliptic;
    // Equation from Astronomical Algorithms page 165
    let o = 125.04 - 1934.136 * t;
    epsilon0 + 0.00256 * degrees_to_radians(o).cos()
}

/// Mean sidereal time, the hour angle of the vernal equinox, in degrees.
pub fn mean_sidereal_time(julian_century: f64) -> f64 {
    let t = julian_century;
    // Equation from Astronomical Algorithms page 165
    let jd = t * 36525.0 + 2451545.0;
    let term1 = 280.46061837;
    let term2 = 360.98564736629 * (jd - 2451545.0);
    let term3 = 0.000387933 * t.powi(2);
    let term4 = t.powi(3) / 38710000.0;
    unwind_angle(term1 + term2 + term3 - term4)
}

pub fn nutation_in_longitude(
    julian_century: f64,
    solar_longitude: f64,
    lunar_longitude: f64,
    ascending_node: f64,
) -> f64 {
    let _ = julian_century;
    let l0 = solar_longitude;
    let lp = lunar_longitude;
    let omega = ascending_node;
    // Equation from Astronomical Algorithms page 144
    let term1 = -17.2 / 3600.0 * degrees_to_radians(omega).sin();
    let term2 = 1.32 / 3600.0 * (2.0 * degrees_to_radians(l0)).sin();
    let term3 = 0.23 / 3600.0 * (2.0 * degrees_to_radians(lp)).sin();
    let term4 = 0.21 / 3600.0 * (2.0 * degrees_to_radians(omega)).sin();
    term1 - term2 - term3 + term4
}

pub fn nutation_in_obliquity(
    julian_century: f64,
    solar_longitude: f64,
    lunar_longitude: f64,
    ascending_node: f64,
) -> f64 {
    let _ = julian_century;
    let l0 = solar_longitude;
    let lp = lunar_longitude;
    let omega = ascending_node;
    // Equation from Astronomical Algorithms page 144
    let term1 = 9.2 / 3600.0 * degrees_to_radians(omega).cos();
    let term2 = 0.57 / 3600.0 * (2.0 * degrees_to_radians(l0)).cos();
    let term3 = 0.1 / 3600.0 * (2.0 * degrees_to_radians(lp)).cos();
    let term4 = 0.09 / 3600.0 * (2.0 * degrees_to_radians(omega)).cos();
    term1 + term2 + term3 - term4
}

pub fn altitude_of_celestial_body(
    observer_latitude: f64,
    declination: f64,
    local_hour_angle: f64,
) -> f64 {
    let phi = observer_latitude;
    let delta = declination;
    let h = local_hour_angle;
    // Equation from Astronomical Algorithms page 93
    let term1 = degrees_to_radians(phi).sin() * degrees_to_radians(delta).sin();
    let term2 = degrees_to_radians(phi).cos()
        * degrees_to_radians(delta).cos()
        * degrees_to_radians(h).cos();
    radians_to_degrees((term1 + term2).asin())
}

pub fn approximate_transit(longitude: f64, sidereal_time: f64, right_ascension: f64) -> f64 {
    let l = longitude;
    let theta0 = sidereal_time;
    let a2 = right_ascension;
    // Equation from Astronomical Algorithms page 102
    let lw = -l;
    normalize_to_scale((a2 + lw - theta0) / 360.0, 1.0)
}

/// The time at which the sun is at its highest point in the sky (in universal time).
pub fn corrected_transit(
    approximate_transit: f64,
    longitude: f64,
    sidereal_time: f64,
    right_ascension: f64,
    previous_right_ascension: f64,
    next_right_ascension: f64,
) -> f64 {
    let m0 = approximate_transit;
    let l = longitude;
    let theta0 = sidereal_time;
    let a2 = right_ascension;
    let a1 = previous_right_ascension;
    let a3 = next_right_ascension;
    // Equation from Astronomical Algorithms page 102
    let lw = -l;
    let theta = unwind_angle(theta0 + 360.985647 * m0);
    let a = unwind_angle(interpolate_angles(a2, a1, a3, m0));
    let h = quadrant_shift_angle(theta - lw - a);
    let dm = h / -360.0;
    (m0 + dm) * 24.0
}

#[allow(clippy::too_many_arguments)]
pub fn corrected_hour_angle(
    approximate_transit: f64,
    angle: f64,
    coordinates: Coordinates,
    after_transit: bool,
    sidereal_time: f64,
    right_ascension: f64,
    previous_right_ascension: f64,
    next_right_ascension: f64,
    declination: f64,
    previous_declination: f64,
    next_declination: f64,
) -> f64 {
    let m0 = approximate_transit;
    let h0 = angle;
    let theta0 = sidereal_time;
    let a2 = right_ascension;
    let a1 = previous_right_ascension;
    let a3 = next_right_ascension;
    let d2 = declination;
    let d1 = previous_declination;
    let d3 = next_declination;
    // Equation from Astronomical Algorithms page 102
    let lw = -coordinates.longitude;
    let term1 = degrees_to_radians(h0).sin()
        - degrees_to_radians(coordinates.latitude).sin() * degrees_to_radians(d2).sin();
    let term2 = degrees_to_radians(coordinates.latitude).cos() * degrees_to_radians(d2).cos();
    // NOTE: acos of a value outside [-1, 1] yields NaN, exactly like the JS
    // version; the caller's high-latitude logic handles the NaN case.
    let cap_h0 = radians_to_degrees((term1 / term2).acos());
    let m = if after_transit {
        m0 + cap_h0 / 360.0
    } else {
        m0 - cap_h0 / 360.0
    };
    let theta = unwind_angle(theta0 + 360.985647 * m);
    let a = unwind_angle(interpolate_angles(a2, a1, a3, m));
    let delta = interpolate(d2, d1, d3, m);
    let h = theta - lw - a;
    let h_alt = altitude_of_celestial_body(coordinates.latitude, delta, h);
    let term3 = h_alt - h0;
    let term4 = 360.0
        * degrees_to_radians(delta).cos()
        * degrees_to_radians(coordinates.latitude).cos()
        * degrees_to_radians(h).sin();
    let dm = term3 / term4;
    (m + dm) * 24.0
}

/// Interpolation of a value given equidistant previous and next values and a
/// factor equal to the fraction of the interpolated point's time over the
/// time between values.
pub fn interpolate(y2: f64, y1: f64, y3: f64, n: f64) -> f64 {
    // Equation from Astronomical Algorithms page 24
    let a = y2 - y1;
    let b = y3 - y2;
    let c = b - a;
    y2 + n / 2.0 * (a + b + n * c)
}

/// Interpolation of three angles, accounting for angle unwinding.
pub fn interpolate_angles(y2: f64, y1: f64, y3: f64, n: f64) -> f64 {
    // Equation from Astronomical Algorithms page 24
    let a = unwind_angle(y2 - y1);
    let b = unwind_angle(y3 - y2);
    let c = b - a;
    y2 + n / 2.0 * (a + b + n * c)
}

/// The Julian Day for the given Gregorian date components.
pub fn julian_day(year: i32, month: u32, day: u32, hours: f64) -> f64 {
    // Equation from Astronomical Algorithms page 60
    let (y, m) = if month > 2 {
        (year as f64, month as f64)
    } else {
        ((year - 1) as f64, (month + 12) as f64)
    };
    let d = day as f64 + hours / 24.0;
    let a = (y / 100.0).trunc();
    let b = (2.0 - a + (a / 4.0).trunc()).trunc();
    let i0 = (365.25 * (y + 4716.0)).trunc();
    let i1 = (30.6001 * (m + 1.0)).trunc();
    i0 + i1 + d + b - 1524.5
}

/// Julian century from the epoch.
pub fn julian_century(julian_day: f64) -> f64 {
    // Equation from Astronomical Algorithms page 163
    (julian_day - 2451545.0) / 36525.0
}

/// Whether or not a year is a leap year (has 366 days).
pub fn is_leap_year(year: i32) -> bool {
    if year % 4 != 0 {
        return false;
    }
    if year % 100 == 0 && year % 400 != 0 {
        return false;
    }
    true
}

/// Season-adjusted morning twilight (MoonsightingCommittee safe Fajr).
/// `sunrise_ms` is milliseconds since the Unix epoch; returns ms too.
pub fn season_adjusted_morning_twilight(
    latitude: f64,
    day_of_year: u32,
    year: i32,
    sunrise_ms: f64,
) -> f64 {
    let a = 75.0 + 28.65 / 55.0 * latitude.abs();
    let b = 75.0 + 19.44 / 55.0 * latitude.abs();
    let c = 75.0 + 32.74 / 55.0 * latitude.abs();
    let d = 75.0 + 48.1 / 55.0 * latitude.abs();

    let dyy = days_since_solstice(day_of_year, year, latitude) as f64;
    let adjustment = if dyy < 91.0 {
        a + (b - a) / 91.0 * dyy
    } else if dyy < 137.0 {
        b + (c - b) / 46.0 * (dyy - 91.0)
    } else if dyy < 183.0 {
        c + (d - c) / 46.0 * (dyy - 137.0)
    } else if dyy < 229.0 {
        d + (c - d) / 46.0 * (dyy - 183.0)
    } else if dyy < 275.0 {
        c + (b - c) / 46.0 * (dyy - 229.0)
    } else {
        b + (a - b) / 91.0 * (dyy - 275.0)
    };

    sunrise_ms + js_round(adjustment * -60.0) * 1000.0
}

/// Season-adjusted evening twilight (MoonsightingCommittee safe Isha).
pub fn season_adjusted_evening_twilight(
    latitude: f64,
    day_of_year: u32,
    year: i32,
    sunset_ms: f64,
    shafaq: Shafaq,
) -> f64 {
    let (a, b, c, d) = match shafaq {
        Shafaq::Ahmer => (
            62.0 + 17.4 / 55.0 * latitude.abs(),
            62.0 - 7.16 / 55.0 * latitude.abs(),
            62.0 + 5.12 / 55.0 * latitude.abs(),
            62.0 + 19.44 / 55.0 * latitude.abs(),
        ),
        Shafaq::Abyad => (
            75.0 + 25.6 / 55.0 * latitude.abs(),
            75.0 + 7.16 / 55.0 * latitude.abs(),
            75.0 + 36.84 / 55.0 * latitude.abs(),
            75.0 + 81.84 / 55.0 * latitude.abs(),
        ),
        Shafaq::General => (
            75.0 + 25.6 / 55.0 * latitude.abs(),
            75.0 + 2.05 / 55.0 * latitude.abs(),
            75.0 - 9.21 / 55.0 * latitude.abs(),
            75.0 + 6.14 / 55.0 * latitude.abs(),
        ),
    };

    let dyy = days_since_solstice(day_of_year, year, latitude) as f64;
    let adjustment = if dyy < 91.0 {
        a + (b - a) / 91.0 * dyy
    } else if dyy < 137.0 {
        b + (c - b) / 46.0 * (dyy - 91.0)
    } else if dyy < 183.0 {
        c + (d - c) / 46.0 * (dyy - 137.0)
    } else if dyy < 229.0 {
        d + (c - d) / 46.0 * (dyy - 183.0)
    } else if dyy < 275.0 {
        c + (b - c) / 46.0 * (dyy - 229.0)
    } else {
        b + (a - b) / 91.0 * (dyy - 275.0)
    };

    sunset_ms + js_round(adjustment * 60.0) * 1000.0
}

pub fn days_since_solstice(day_of_year: u32, year: i32, latitude: f64) -> i32 {
    let northern_offset = 10;
    let southern_offset = if is_leap_year(year) { 173 } else { 172 };
    let days_in_year = if is_leap_year(year) { 366 } else { 365 };

    if latitude >= 0.0 {
        let mut days = day_of_year as i32 + northern_offset;
        if days >= days_in_year {
            days -= days_in_year;
        }
        days
    } else {
        let mut days = day_of_year as i32 - southern_offset;
        if days < 0 {
            days += days_in_year;
        }
        days
    }
}

// ---------------------------------------------------------------------------
// SolarCoordinates.js
// ---------------------------------------------------------------------------

/// The declination, right ascension and apparent sidereal time of the sun for
/// a given Julian day.
#[derive(Debug, Clone, Copy)]
pub struct SolarCoordinates {
    /// The declination of the sun, the angle between the rays of the Sun and
    /// the plane of the Earth's equator, in degrees.
    pub declination: f64,
    /// Right ascension of the Sun, the angular distance on the celestial
    /// equator from the vernal equinox to the hour circle, in degrees.
    pub right_ascension: f64,
    /// Apparent sidereal time, the hour angle of the vernal equinox, in degrees.
    pub apparent_sidereal_time: f64,
}

impl SolarCoordinates {
    pub fn new(julian_day: f64) -> Self {
        let t = julian_century(julian_day);
        let l0 = mean_solar_longitude(t);
        let lp = mean_lunar_longitude(t);
        let omega = ascending_lunar_node_longitude(t);
        let lambda = degrees_to_radians(apparent_solar_longitude(t, l0));
        let theta0 = mean_sidereal_time(t);
        let dpsi = nutation_in_longitude(t, l0, lp, omega);
        let depsilon = nutation_in_obliquity(t, l0, lp, omega);
        let epsilon0 = mean_obliquity_of_the_ecliptic(t);
        let epsilon_apparent = degrees_to_radians(apparent_obliquity_of_the_ecliptic(t, epsilon0));

        // Equation from Astronomical Algorithms page 165
        let declination =
            radians_to_degrees((epsilon_apparent.sin() * lambda.sin()).asin());
        // Equation from Astronomical Algorithms page 165
        let right_ascension = unwind_angle(radians_to_degrees(
            (epsilon_apparent.cos() * lambda.sin()).atan2(lambda.cos()),
        ));
        // Equation from Astronomical Algorithms page 88
        let apparent_sidereal_time =
            theta0 + dpsi * 3600.0 * degrees_to_radians(epsilon0 + depsilon).cos() / 3600.0;

        SolarCoordinates {
            declination,
            right_ascension,
            apparent_sidereal_time,
        }
    }
}

// ---------------------------------------------------------------------------
// SolarTime.js
// ---------------------------------------------------------------------------

/// All times are in hours (0..24, fractional), NaN when the sun does not
/// reach the required altitude at this location/date.
#[derive(Debug, Clone, Copy)]
pub struct SolarTime {
    pub observer: Coordinates,
    pub solar: SolarCoordinates,
    pub prev_solar: SolarCoordinates,
    pub next_solar: SolarCoordinates,
    pub approx_transit: f64,
    pub transit: f64,
    pub sunrise: f64,
    pub sunset: f64,
}

impl SolarTime {
    pub fn new(year: i32, month: u32, day: u32, coordinates: Coordinates) -> Self {
        let julian_day = julian_day(year, month, day, 0.0);
        let solar = SolarCoordinates::new(julian_day);
        let prev_solar = SolarCoordinates::new(julian_day - 1.0);
        let next_solar = SolarCoordinates::new(julian_day + 1.0);
        let m0 = approximate_transit(
            coordinates.longitude,
            solar.apparent_sidereal_time,
            solar.right_ascension,
        );
        let solar_altitude = -50.0 / 60.0;
        let transit = corrected_transit(
            m0,
            coordinates.longitude,
            solar.apparent_sidereal_time,
            solar.right_ascension,
            prev_solar.right_ascension,
            next_solar.right_ascension,
        );
        let sunrise = corrected_hour_angle(
            m0,
            solar_altitude,
            coordinates,
            false,
            solar.apparent_sidereal_time,
            solar.right_ascension,
            prev_solar.right_ascension,
            next_solar.right_ascension,
            solar.declination,
            prev_solar.declination,
            next_solar.declination,
        );
        let sunset = corrected_hour_angle(
            m0,
            solar_altitude,
            coordinates,
            true,
            solar.apparent_sidereal_time,
            solar.right_ascension,
            prev_solar.right_ascension,
            next_solar.right_ascension,
            solar.declination,
            prev_solar.declination,
            next_solar.declination,
        );

        SolarTime {
            observer: coordinates,
            solar,
            prev_solar,
            next_solar,
            approx_transit: m0,
            transit,
            sunrise,
            sunset,
        }
    }

    pub fn hour_angle(&self, angle: f64, after_transit: bool) -> f64 {
        corrected_hour_angle(
            self.approx_transit,
            angle,
            self.observer,
            after_transit,
            self.solar.apparent_sidereal_time,
            self.solar.right_ascension,
            self.prev_solar.right_ascension,
            self.next_solar.right_ascension,
            self.solar.declination,
            self.prev_solar.declination,
            self.next_solar.declination,
        )
    }

    /// Time for the given shadow length factor (Asr).
    pub fn afternoon(&self, shadow_length: f64) -> f64 {
        let tangent = (self.observer.latitude - self.solar.declination).abs();
        let inverse = shadow_length + degrees_to_radians(tangent).tan();
        let angle = radians_to_degrees((1.0 / inverse).atan());
        self.hour_angle(angle, true)
    }
}
