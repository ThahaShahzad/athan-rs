//! Prayer-time calculation core: a faithful Rust port of the batoulapps
//! `adhan` JavaScript library (v4.4.3).

pub mod calculation_parameters;
pub mod coordinates;
#[allow(clippy::module_inception)] // layout mirrors the JS source tree per spec
pub mod prayer_times;
pub mod solar_time;

pub use calculation_parameters::{
    CalculationMethod, CalculationParameters, HighLatitudeRule, Madhab, PolarCircleResolution,
    PrayerAdjustments, Rounding, Shafaq,
};
pub use coordinates::Coordinates;
pub use prayer_times::{Prayer, PrayerTimes};
