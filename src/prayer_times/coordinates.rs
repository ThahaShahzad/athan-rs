//! Geographic coordinates of the observer.
//!
//! Port of `Coordinates.js` from the adhan npm package.

use serde::{Deserialize, Serialize};

/// Latitude/longitude of the location prayer times are calculated for.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Coordinates {
    pub latitude: f64,
    pub longitude: f64,
}

impl Coordinates {
    pub fn new(latitude: f64, longitude: f64) -> Self {
        Coordinates {
            latitude,
            longitude,
        }
    }
}
