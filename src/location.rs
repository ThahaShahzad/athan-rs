//! Location handling for the CLI: manual lat/lng input or IP geolocation
//! via <https://ipapi.co/json/> (with user consent).
//!
//! Port of `location.js` from the Node athan-cli.

use std::io::{BufRead, Write};

use serde::Deserialize;

use crate::error::AppError;

pub struct Location {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Deserialize)]
struct IpApiResponse {
    latitude: Option<f64>,
    longitude: Option<f64>,
}

/// Get approximate location from IP address.
/// Uses ipapi.co (free, no API key required, ~city-level accuracy).
pub fn fetch_location_by_ip() -> Result<Location, AppError> {
    let response = reqwest::blocking::Client::new()
        .get("https://ipapi.co/json/")
        .header(reqwest::header::ACCEPT, "application/json")
        .send()?;

    if !response.status().is_success() {
        return Err(AppError::msg("IP geolocation service unavailable."));
    }

    let data: IpApiResponse = response.json()?;
    match (data.latitude, data.longitude) {
        (Some(lat), Some(lng)) => Ok(Location {
            lat: (lat * 10000.0).round() / 10000.0,
            lng: (lng * 10000.0).round() / 10000.0,
        }),
        _ => Err(AppError::msg("IP geolocation returned invalid data.")),
    }
}

pub fn is_valid_lat(n: f64) -> bool {
    n.is_finite() && n.abs() <= 90.0
}

pub fn is_valid_lng(n: f64) -> bool {
    n.is_finite() && n.abs() <= 180.0
}

/// Interactively resolve the user's location on first run.
/// Offers manual lat/lng entry or IP-based geolocation (with consent).
pub fn prompt_for_location() -> Result<Location, AppError> {
    let stdin = std::io::stdin();
    let mut lines = stdin.lock().lines();

    // Reads one line. EOF (closed/piped stdin running out) aborts the prompt
    // instead of spinning on empty answers forever.
    let mut ask = |prompt: &str| -> Result<String, AppError> {
        print!("{prompt}");
        std::io::stdout().flush()?;
        match lines.next() {
            Some(Ok(line)) => Ok(line),
            Some(Err(e)) => Err(e.into()),
            None => Err(AppError::msg("No input available (stdin closed).")),
        }
    };

    println!("\nNo location configured yet.");

    loop {
        let answer = ask("Enter coordinates manually (m) or detect from your IP address (i)? [m/i] ")?;
        let choice = answer.trim().to_lowercase();

        if choice == "i" {
            let consent = ask(
                "This sends your IP address to https://ipapi.co to estimate your location. Continue? [y/N] ",
            )?;
            if consent.trim().to_lowercase() != "y" {
                continue;
            }
            match fetch_location_by_ip() {
                Ok(loc) => {
                    println!("Detected location: {}, {}", loc.lat, loc.lng);
                    let ok = ask("Use this location? [Y/n] ")?;
                    if ok.trim().to_lowercase() == "n" {
                        continue;
                    }
                    return Ok(loc);
                }
                Err(err) => {
                    eprintln!("IP geolocation failed: {err}");
                    continue;
                }
            }
        }

        if choice == "m" || choice.is_empty() {
            let lat_str = ask("Latitude (e.g. 40.7128): ")?;
            let lng_str = ask("Longitude (e.g. -74.0060): ")?;
            let lat: f64 = lat_str.trim().parse().unwrap_or(f64::NAN);
            let lng: f64 = lng_str.trim().parse().unwrap_or(f64::NAN);
            if !is_valid_lat(lat) || !is_valid_lng(lng) {
                eprintln!("Invalid coordinates. Latitude must be -90..90, longitude -180..180.");
                continue;
            }
            return Ok(Location { lat, lng });
        }
    }
}
