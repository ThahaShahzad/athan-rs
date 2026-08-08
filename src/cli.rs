//! CLI entry point — clap definitions and command dispatch.
//!
//! Port of `cli.js` from the Node athan-cli. Commands:
//!   athan                  TUI dashboard (default)
//!   athan start            foreground scheduler — plays athan at prayer times
//!   athan times [date]     print today's (or a given date's) prayer times
//!   athan next             print the next prayer and countdown
//!   athan config show      print current config
//!   athan config set <key> <value>
//!   athan sounds           list available athan sounds
//!   athan test <sound>     play a sound once
//!   athan autostart enable|disable|status   manage OS autostart service

use chrono::{Local, NaiveDate, Utc};
use clap::{Parser, Subcommand};

use crate::config::{
    ATHAN_SOUNDS, Config, FAJR_SOUNDS, VALID_KEYS, all_sounds, config_path, is_valid_sound,
    load_config, save_config, set_config_value,
};
use crate::display::{
    METHOD_KEYS, PRAYER_INFO, calculate_prayer_times, current_and_next, format_countdown,
    format_time, method_label, prayer_name, time_by_key, time_until_next,
};
use crate::error::AppError;
use crate::location::prompt_for_location;
use crate::scheduler::start_scheduler;

#[derive(Parser)]
#[command(
    name = "athan",
    version,
    about = "Islamic prayer times CLI with athan audio playback"
)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the athan scheduler in the foreground (plays athan at prayer times)
    Start,
    /// Show prayer times for today or a given date (YYYY-MM-DD)
    Times {
        /// Date to show times for (YYYY-MM-DD); defaults to today
        date: Option<String>,
    },
    /// Show the next prayer and time until it
    Next,
    /// View or edit configuration
    Config {
        #[command(subcommand)]
        command: ConfigCommands,
    },
    /// Manage autostart on login/boot (runs `athan start`)
    Autostart {
        #[command(subcommand)]
        command: AutostartCommands,
    },
    /// List available athan sounds
    Sounds,
    /// Play a sound once (see `athan sounds`)
    Test {
        /// Sound key (see `athan sounds`)
        sound: String,
    },
}

#[derive(Subcommand)]
enum AutostartCommands {
    /// Install and start the OS-level autostart service
    Enable,
    /// Stop and remove the OS-level autostart service
    Disable,
    /// Show whether autostart is installed and running
    Status,
}

#[derive(Subcommand)]
enum ConfigCommands {
    /// Show current configuration
    Show,
    /// Set a config value. Keys: latitude, longitude, method, madhab, athanSound, fajrSound, athanEnabled
    Set {
        /// Config key
        key: String,
        /// New value
        #[arg(allow_negative_numbers = true)]
        value: String,
    },
}

/// Ensure config has a location; prompt on first run otherwise.
fn ensure_location() -> Result<Config, AppError> {
    let config = load_config();
    if config.latitude.is_some() && config.longitude.is_some() {
        return Ok(config);
    }
    let loc = prompt_for_location()?;
    let updated = Config {
        latitude: Some(loc.lat),
        longitude: Some(loc.lng),
        ..config
    };
    save_config(&updated)?;
    println!("Location saved to {}\n", config_path().display());
    Ok(updated)
}

fn parse_date_arg(date_str: Option<&str>) -> Result<NaiveDate, AppError> {
    match date_str {
        None => Ok(Local::now().date_naive()),
        Some(s) => NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
            AppError::msg(format!("Invalid date: \"{s}\". Use YYYY-MM-DD."))
        }),
    }
}

pub fn run() -> Result<(), AppError> {
    let cli = Cli::parse();

    match cli.command {
        None => {
            let config = ensure_location()?;
            crate::tui::run_tui(&config)
        }
        Some(Commands::Start) => cmd_start(),
        Some(Commands::Times { date }) => cmd_times(date.as_deref()),
        Some(Commands::Next) => cmd_next(),
        Some(Commands::Config {
            command: ConfigCommands::Show,
        }) => cmd_config_show(),
        Some(Commands::Config {
            command: ConfigCommands::Set { key, value },
        }) => cmd_config_set(&key, &value),
        Some(Commands::Autostart {
            command: AutostartCommands::Enable,
        }) => crate::autostart::enable(),
        Some(Commands::Autostart {
            command: AutostartCommands::Disable,
        }) => crate::autostart::disable(),
        Some(Commands::Autostart {
            command: AutostartCommands::Status,
        }) => crate::autostart::status(),
        Some(Commands::Sounds) => cmd_sounds(),
        Some(Commands::Test { sound }) => cmd_test(&sound),
    }
}

fn cmd_start() -> Result<(), AppError> {
    let config = ensure_location()?;
    if !config.athan_enabled {
        println!(
            "Athan is disabled (athanEnabled=false). Enable it with: athan config set athanEnabled true"
        );
        std::process::exit(1);
    }
    println!(
        "Athan scheduler running for {}, {} ({}, {}).",
        config.latitude.unwrap_or(f64::NAN),
        config.longitude.unwrap_or(f64::NAN),
        method_label(&config.method),
        config.madhab
    );
    println!("Press Ctrl+C to stop.\n");

    let _handle = start_scheduler(|prayer_key| {
        let stamp = Local::now().format("%-I:%M:%S %p").to_string();
        println!("[{stamp}] Playing athan for {prayer_key}");
    });

    // Keep the process alive; exit cleanly on SIGINT/SIGTERM.
    ctrlc::set_handler(|| {
        println!("\nScheduler stopped.");
        std::process::exit(0);
    })
    .map_err(|e| AppError::msg(format!("Failed to install signal handler: {e}")))?;

    loop {
        std::thread::park();
    }
}

fn cmd_times(date_str: Option<&str>) -> Result<(), AppError> {
    let config = ensure_location()?;
    let date = parse_date_arg(date_str)?;
    let lat = config.latitude.unwrap_or(f64::NAN);
    let lng = config.longitude.unwrap_or(f64::NAN);
    let times = calculate_prayer_times(lat, lng, date, &config.method, &config.madhab);

    println!(
        "Prayer times for {}",
        date.format("%A, %B %-d, %Y")
    );
    println!(
        "Location: {}, {} • {} • {}\n",
        lat,
        lng,
        method_label(&config.method),
        config.madhab
    );
    for p in PRAYER_INFO {
        println!("  {:9} {}", p.en, format_time(time_by_key(&times, p.key)));
    }
    Ok(())
}

fn cmd_next() -> Result<(), AppError> {
    let config = ensure_location()?;
    let now = Utc::now();
    let times = calculate_prayer_times(
        config.latitude.unwrap_or(f64::NAN),
        config.longitude.unwrap_or(f64::NAN),
        Local::now().date_naive(),
        &config.method,
        &config.madhab,
    );
    let (current, next) = current_and_next(&times, now);
    let until = time_until_next(&times, now);

    println!(
        "Current prayer: {}",
        if current == "none" {
            "—".to_string()
        } else {
            prayer_name(&current).to_string()
        }
    );
    if next == "none" || until.is_none() {
        println!("Next prayer:     Fajr (tomorrow)");
    } else {
        println!(
            "Next prayer:     {} at {} (in {})",
            prayer_name(&next),
            format_time(time_by_key(&times, &next)),
            format_countdown(until.unwrap_or_default())
        );
    }
    Ok(())
}

fn cmd_config_show() -> Result<(), AppError> {
    let config = load_config();
    println!("Config file: {}\n", config_path().display());
    for &key in VALID_KEYS {
        let value = match key {
            "latitude" => config
                .latitude
                .map(|v| v.to_string())
                .unwrap_or_else(|| "(not set)".to_string()),
            "longitude" => config
                .longitude
                .map(|v| v.to_string())
                .unwrap_or_else(|| "(not set)".to_string()),
            "method" => config.method.clone(),
            "madhab" => config.madhab.clone(),
            "athanSound" => config.athan_sound.clone(),
            "fajrSound" => config.fajr_sound.clone(),
            "athanEnabled" => config.athan_enabled.to_string(),
            _ => unreachable!(),
        };
        println!("  {key:14} {value}");
    }
    Ok(())
}

fn cmd_config_set(key: &str, value: &str) -> Result<(), AppError> {
    if key == "method" && !METHOD_KEYS.contains(&value) {
        return Err(AppError::msg(format!(
            "Unknown method \"{value}\". Valid methods: {}",
            METHOD_KEYS.join(", ")
        )));
    }
    if key == "madhab" && value != "Shafi" && value != "Hanafi" {
        return Err(AppError::msg("madhab must be \"Shafi\" or \"Hanafi\""));
    }
    if (key == "athanSound" || key == "fajrSound") && !is_valid_sound(value) {
        return Err(AppError::msg(format!(
            "Unknown sound \"{value}\". Run `athan sounds` to list valid sounds."
        )));
    }
    let updated = set_config_value(key, value)?;
    let printed = match key {
        "latitude" => updated.latitude.map(|v| v.to_string()).unwrap_or_default(),
        "longitude" => updated.longitude.map(|v| v.to_string()).unwrap_or_default(),
        "method" => updated.method,
        "madhab" => updated.madhab,
        "athanSound" => updated.athan_sound,
        "fajrSound" => updated.fajr_sound,
        "athanEnabled" => updated.athan_enabled.to_string(),
        _ => unreachable!(),
    };
    println!("Set {key} = {printed}");
    Ok(())
}

fn cmd_sounds() -> Result<(), AppError> {
    let config = load_config();
    println!("Athan sounds (dhuhr/asr/maghrib/isha):");
    for s in ATHAN_SOUNDS {
        let marker = if s.key == config.athan_sound { " (selected)" } else { "" };
        println!("  {:16} {:16} {}{marker}", s.key, s.label, s.size);
    }
    println!("\nFajr sounds:");
    for s in FAJR_SOUNDS {
        let marker = if s.key == config.fajr_sound { " (selected)" } else { "" };
        println!("  {:16} {:16} {}{marker}", s.key, s.label, s.size);
    }
    Ok(())
}

fn cmd_test(sound: &str) -> Result<(), AppError> {
    if !all_sounds().any(|s| s.key == sound) {
        return Err(AppError::msg(format!(
            "Unknown sound \"{sound}\". Run `athan sounds` to list valid sounds."
        )));
    }
    println!("Playing {sound}... (Ctrl+C to stop)");
    crate::audio::play_sound_blocking(sound)
}
