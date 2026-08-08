//! Schedules athan playback at prayer times.
//!
//! Port of `scheduler.js` from the Node athan-cli. The Node version arms a
//! `setTimeout` for the next prayer plus a 30 s drift-guard interval; this
//! port uses a single 1 s tick loop that fires when within 2 s of a prayer
//! time (the drift guard), which subsumes both timers. Prayer times are
//! recalculated when the local date rolls over (midnight recalc), and the
//! persisted `lastPlayed` state prevents replaying a prayer after a restart.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, Utc};

use crate::audio::play_sound_in_background;
use crate::config::{Config, load_config, load_state, save_state};
use crate::display::{ATHAN_PRAYERS, calculate_prayer_times, time_by_key};

/// Local YYYY-MM-DD (state file key, same as the Node `todayKey`).
fn today_key(date: DateTime<Local>) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        date.month(),
        date.day()
    )
}

/// Check whether a prayer was already played today (persisted).
pub fn already_played(prayer_key: &str) -> bool {
    let state = load_state();
    state
        .last_played
        .get(&today_key(Local::now()))
        .is_some_and(|played| played.iter().any(|p| p == prayer_key))
}

/// Record a prayer as played today (persisted). Prunes older days.
pub fn mark_played(prayer_key: &str) {
    let mut state = load_state();
    let key = today_key(Local::now());
    // Keep only today's record — previous days are irrelevant.
    let mut played = state.last_played.remove(&key).unwrap_or_default();
    if !played.iter().any(|p| p == prayer_key) {
        played.push(prayer_key.to_string());
    }
    state.last_played.clear();
    state.last_played.insert(key, played);
    if let Err(err) = save_state(&state) {
        eprintln!("Failed to save scheduler state: {err}");
    }
}

/// Play the athan for a prayer (fajr uses fajrSound, others athanSound).
/// Returns false (without playing) if it already played today.
fn fire_athan<F>(prayer_key: &str, config: &Config, on_athan_start: &F) -> bool
where
    F: Fn(&str),
{
    if already_played(prayer_key) {
        return false;
    }

    mark_played(prayer_key);
    let sound_key = if prayer_key == "fajr" {
        config.fajr_sound.clone()
    } else {
        config.athan_sound.clone()
    };
    on_athan_start(prayer_key);
    play_sound_in_background(sound_key);
    true
}

/// Handle for a running scheduler; dropping or calling `stop` ends the loop.
pub struct SchedulerHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl SchedulerHandle {
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }

    pub fn is_running(&self) -> bool {
        !self.stop.load(Ordering::SeqCst)
            && self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }
}

impl Drop for SchedulerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start the scheduler on a background thread. The config is reloaded on
/// every tick so `athan config set` takes effect without a restart, and
/// prayer times are recalculated when the local date changes.
///
/// `on_athan_start(prayer_key)` is called when an athan fires.
pub fn start_scheduler<F>(on_athan_start: F) -> SchedulerHandle
where
    F: Fn(&str) + Send + 'static,
{
    let stop = Arc::new(AtomicBool::new(false));
    let stop_thread = Arc::clone(&stop);

    let thread = std::thread::spawn(move || {
        // Prayer times for the day currently armed.
        let mut armed_date = Local::now().date_naive();
        let mut config = load_config();
        let mut times = calculate_prayer_times(
            config.latitude.unwrap_or(f64::NAN),
            config.longitude.unwrap_or(f64::NAN),
            armed_date,
            &config.method,
            &config.madhab,
        );

        while !stop_thread.load(Ordering::Relaxed) {
            let now = Utc::now();
            let local_now = now.with_timezone(&Local);
            config = load_config();

            // Midnight rollover: recalculate for the new day.
            if local_now.date_naive() != armed_date {
                armed_date = local_now.date_naive();
                times = calculate_prayer_times(
                    config.latitude.unwrap_or(f64::NAN),
                    config.longitude.unwrap_or(f64::NAN),
                    armed_date,
                    &config.method,
                    &config.madhab,
                );
            }

            if config.athan_enabled {
                for &key in ATHAN_PRAYERS {
                    let Some(time) = time_by_key(&times, key) else {
                        continue;
                    };
                    // If within 2 seconds of a prayer time, play it.
                    let diff = (time - now).num_milliseconds().abs();
                    if diff < 2000 {
                        fire_athan(key, &config, &on_athan_start);
                    }
                }
            }

            // Sleep in short increments so stop() is responsive.
            for _ in 0..10 {
                if stop_thread.load(Ordering::Relaxed) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });

    SchedulerHandle {
        stop,
        thread: Some(thread),
    }
}
