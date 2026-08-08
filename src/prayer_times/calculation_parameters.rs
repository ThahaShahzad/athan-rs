//! Calculation method parameter sets, madhab, high-latitude rules, rounding.
//!
//! Port of `CalculationParameters.js`, `CalculationMethod.js`, `Madhab.js`,
//! `HighLatitudeRule.js`, `Rounding.js`, `Shafaq.js` and
//! `PolarCircleResolution.js` (the enum part) from the adhan npm package.

use serde::{Deserialize, Serialize};

/// Madhab to determine how Asr is calculated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Madhab {
    Shafi,
    Hanafi,
}

impl Madhab {
    pub fn shadow_length(self) -> f64 {
        match self {
            Madhab::Shafi => 1.0,
            Madhab::Hanafi => 2.0,
        }
    }
}

/// Rule to determine the earliest time for Fajr and latest time for Isha
/// needed for high latitude locations where Fajr and Isha may not truly exist
/// or may present a hardship unless bound to a reasonable time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HighLatitudeRule {
    MiddleOfTheNight,
    SeventhOfTheNight,
    TwilightAngle,
}

impl HighLatitudeRule {
    pub fn recommended(latitude: f64) -> Self {
        if latitude > 48.0 {
            HighLatitudeRule::SeventhOfTheNight
        } else {
            HighLatitudeRule::MiddleOfTheNight
        }
    }
}

/// How seconds are rounded when calculating prayer times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rounding {
    Nearest,
    Up,
    None,
}

/// Shafaq is the twilight in the sky. Different madhabs define the appearance
/// of twilight differently. Used by the MoonsightingCommittee method for the
/// different ways to calculate Isha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Shafaq {
    /// General is a combination of Ahmer and Abyad.
    General,
    /// Ahmer means the twilight is the red glow in the sky. Used by the
    /// Shafi, Maliki, and Hanbali madhabs.
    Ahmer,
    /// Abyad means the twilight is the white glow in the sky. Used by the
    /// Hanafi madhab.
    Abyad,
}

/// Rule to determine how to resolve prayer times inside the Polar Circle
/// where daylight or night may persist for more than 24 hours depending
/// on the season.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolarCircleResolution {
    AqrabBalad,
    AqrabYaum,
    Unresolved,
}

/// The 12 calculation methods supported by the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalculationMethod {
    MuslimWorldLeague,
    Egyptian,
    Karachi,
    UmmAlQura,
    Dubai,
    MoonsightingCommittee,
    NorthAmerica,
    Kuwait,
    Qatar,
    Singapore,
    Tehran,
    Turkey,
    Other,
}

impl CalculationMethod {
    /// All 12 named methods (excluding `Other`), in the same order the web
    /// app lists them.
    pub const ALL: [CalculationMethod; 12] = [
        CalculationMethod::MuslimWorldLeague,
        CalculationMethod::Egyptian,
        CalculationMethod::Karachi,
        CalculationMethod::UmmAlQura,
        CalculationMethod::Dubai,
        CalculationMethod::MoonsightingCommittee,
        CalculationMethod::NorthAmerica,
        CalculationMethod::Kuwait,
        CalculationMethod::Qatar,
        CalculationMethod::Singapore,
        CalculationMethod::Tehran,
        CalculationMethod::Turkey,
    ];

    pub fn name(self) -> &'static str {
        match self {
            CalculationMethod::MuslimWorldLeague => "MuslimWorldLeague",
            CalculationMethod::Egyptian => "Egyptian",
            CalculationMethod::Karachi => "Karachi",
            CalculationMethod::UmmAlQura => "UmmAlQura",
            CalculationMethod::Dubai => "Dubai",
            CalculationMethod::MoonsightingCommittee => "MoonsightingCommittee",
            CalculationMethod::NorthAmerica => "NorthAmerica",
            CalculationMethod::Kuwait => "Kuwait",
            CalculationMethod::Qatar => "Qatar",
            CalculationMethod::Singapore => "Singapore",
            CalculationMethod::Tehran => "Tehran",
            CalculationMethod::Turkey => "Turkey",
            CalculationMethod::Other => "Other",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "MuslimWorldLeague" => CalculationMethod::MuslimWorldLeague,
            "Egyptian" => CalculationMethod::Egyptian,
            "Karachi" => CalculationMethod::Karachi,
            "UmmAlQura" => CalculationMethod::UmmAlQura,
            "Dubai" => CalculationMethod::Dubai,
            "MoonsightingCommittee" => CalculationMethod::MoonsightingCommittee,
            "NorthAmerica" => CalculationMethod::NorthAmerica,
            "Kuwait" => CalculationMethod::Kuwait,
            "Qatar" => CalculationMethod::Qatar,
            "Singapore" => CalculationMethod::Singapore,
            "Tehran" => CalculationMethod::Tehran,
            "Turkey" => CalculationMethod::Turkey,
            "Other" => CalculationMethod::Other,
            _ => return None,
        })
    }

    /// The parameter set for this method, exactly as in
    /// `CalculationMethod.js`.
    pub fn parameters(self) -> CalculationParameters {
        let mut params = match self {
            // Muslim World League
            CalculationMethod::MuslimWorldLeague => {
                let mut p = CalculationParameters::new(self, 18.0, 17.0);
                p.method_adjustments.dhuhr = 1;
                p
            }
            // Egyptian General Authority of Survey
            CalculationMethod::Egyptian => {
                let mut p = CalculationParameters::new(self, 19.5, 17.5);
                p.method_adjustments.dhuhr = 1;
                p
            }
            // University of Islamic Sciences, Karachi
            CalculationMethod::Karachi => {
                let mut p = CalculationParameters::new(self, 18.0, 18.0);
                p.method_adjustments.dhuhr = 1;
                p
            }
            // Umm al-Qura University, Makkah
            CalculationMethod::UmmAlQura => {
                CalculationParameters::new(self, 18.5, 0.0).with_isha_interval(90)
            }
            // Dubai
            CalculationMethod::Dubai => {
                let mut p = CalculationParameters::new(self, 18.2, 18.2);
                p.method_adjustments.sunrise = -3;
                p.method_adjustments.dhuhr = 3;
                p.method_adjustments.asr = 3;
                p.method_adjustments.maghrib = 3;
                p
            }
            // Moonsighting Committee
            CalculationMethod::MoonsightingCommittee => {
                let mut p = CalculationParameters::new(self, 18.0, 18.0);
                p.method_adjustments.dhuhr = 5;
                p.method_adjustments.maghrib = 3;
                p
            }
            // ISNA
            CalculationMethod::NorthAmerica => {
                let mut p = CalculationParameters::new(self, 15.0, 15.0);
                p.method_adjustments.dhuhr = 1;
                p
            }
            // Kuwait
            CalculationMethod::Kuwait => CalculationParameters::new(self, 18.0, 17.5),
            // Qatar
            CalculationMethod::Qatar => {
                CalculationParameters::new(self, 18.0, 0.0).with_isha_interval(90)
            }
            // Singapore
            CalculationMethod::Singapore => {
                let mut p = CalculationParameters::new(self, 20.0, 18.0);
                p.method_adjustments.dhuhr = 1;
                p.rounding = Rounding::Up;
                p
            }
            // Institute of Geophysics, University of Tehran
            CalculationMethod::Tehran => CalculationParameters::new(self, 17.7, 14.0)
                .with_maghrib_angle(4.5),
            // Dianet
            CalculationMethod::Turkey => {
                let mut p = CalculationParameters::new(self, 18.0, 17.0);
                p.method_adjustments.sunrise = -7;
                p.method_adjustments.dhuhr = 5;
                p.method_adjustments.asr = 4;
                p.method_adjustments.maghrib = 7;
                p
            }
            // Other
            CalculationMethod::Other => CalculationParameters::new(self, 0.0, 0.0),
        };
        // Mirror the JS constructor: method defaults to 'Other' if null.
        if params.method_name.is_empty() {
            params.method_name = "Other".to_string();
        }
        params.method_name = self.name().to_string();
        params
    }
}

/// Manual or method-driven adjustments (in minutes) added to each prayer time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrayerAdjustments {
    pub fajr: i32,
    pub sunrise: i32,
    pub dhuhr: i32,
    pub asr: i32,
    pub maghrib: i32,
    pub isha: i32,
}

/// Parameters used for prayer time calculation.
///
/// Port of `CalculationParameters.js`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalculationParameters {
    /// Name of the method, used to apply special behavior in calculations.
    pub method_name: String,
    /// Angle of the sun below the horizon used for calculating Fajr.
    pub fajr_angle: f64,
    /// Angle of the sun below the horizon used for calculating Isha.
    pub isha_angle: f64,
    /// Minutes after Maghrib to determine time for Isha; if greater than 0
    /// then `isha_angle` is not used.
    pub isha_interval: i32,
    /// Angle of the sun below the horizon used for calculating Maghrib.
    /// Only used by the Tehran method to account for lightness in the sky.
    pub maghrib_angle: f64,
    pub madhab: Madhab,
    pub high_latitude_rule: HighLatitudeRule,
    /// Manual adjustments (in minutes) to be added to each prayer time.
    pub adjustments: PrayerAdjustments,
    /// Adjustments set by a calculation method. Should not be manually modified.
    pub method_adjustments: PrayerAdjustments,
    pub polar_circle_resolution: PolarCircleResolution,
    pub rounding: Rounding,
    /// Used by the MoonsightingCommittee method to determine how to calculate Isha.
    pub shafaq: Shafaq,
}

impl CalculationParameters {
    pub fn new(method: CalculationMethod, fajr_angle: f64, isha_angle: f64) -> Self {
        CalculationParameters {
            method_name: method.name().to_string(),
            fajr_angle,
            isha_angle,
            isha_interval: 0,
            maghrib_angle: 0.0,
            madhab: Madhab::Shafi,
            high_latitude_rule: HighLatitudeRule::MiddleOfTheNight,
            adjustments: PrayerAdjustments::default(),
            method_adjustments: PrayerAdjustments::default(),
            polar_circle_resolution: PolarCircleResolution::Unresolved,
            rounding: Rounding::Nearest,
            shafaq: Shafaq::General,
        }
    }

    pub fn with_isha_interval(mut self, isha_interval: i32) -> Self {
        self.isha_interval = isha_interval;
        self
    }

    pub fn with_maghrib_angle(mut self, maghrib_angle: f64) -> Self {
        self.maghrib_angle = maghrib_angle;
        self
    }

    pub fn is_moonsighting_committee(&self) -> bool {
        self.method_name == "MoonsightingCommittee"
    }

    /// Night portions (fraction of the night) for Fajr and Isha based on the
    /// high latitude rule. Port of `CalculationParameters.nightPortions()`.
    pub fn night_portions(&self) -> (f64, f64) {
        match self.high_latitude_rule {
            HighLatitudeRule::MiddleOfTheNight => (1.0 / 2.0, 1.0 / 2.0),
            HighLatitudeRule::SeventhOfTheNight => (1.0 / 7.0, 1.0 / 7.0),
            HighLatitudeRule::TwilightAngle => (self.fajr_angle / 60.0, self.isha_angle / 60.0),
        }
    }
}
