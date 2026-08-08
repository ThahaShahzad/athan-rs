//! Audio playback for the athan CLI using rodio.
//!
//! The five bundled MP3s are embedded in the binary with `include_bytes!`,
//! so both `cargo run` and an installed binary can play them without
//! resolving the assets directory.

use std::io::Cursor;

use rodio::{Decoder, OutputStreamBuilder, Sink};

use crate::config::is_valid_sound;
use crate::error::AppError;

/// The bundled MP3 bytes, keyed by sound key.
fn sound_bytes(sound_key: &str) -> Result<&'static [u8], AppError> {
    // Guard against path-traversal-style keys — sound keys are simple
    // identifiers validated against the catalog.
    if !is_valid_sound(sound_key) {
        return Err(AppError::msg(format!("Invalid sound key: \"{sound_key}\"")));
    }
    Ok(match sound_key {
        "adhan-alaqsa" => include_bytes!("../assets/audio/adhan-alaqsa.mp3") as &'static [u8],
        "adhan-fajr" => include_bytes!("../assets/audio/adhan-fajr.mp3"),
        "adhan-makkah" => include_bytes!("../assets/audio/adhan-makkah.mp3"),
        "adhan-madinah" => include_bytes!("../assets/audio/adhan-madinah.mp3"),
        "adhan-mishary" => include_bytes!("../assets/audio/adhan-mishary.mp3"),
        _ => unreachable!(),
    })
}

/// Decode a bundled sound. Split out from playback so decode errors are
/// distinguishable from audio-device errors.
fn decode(sound_key: &str) -> Result<Decoder<Cursor<&'static [u8]>>, AppError> {
    let bytes = sound_bytes(sound_key)?;
    Decoder::new(Cursor::new(bytes))
        .map_err(|e| AppError::msg(format!("Cannot decode bundled sound \"{sound_key}\": {e}")))
}

/// Play an athan sound, blocking until playback finishes (mirrors the Node
/// `playSound` promise, which resolves when playback finishes).
pub fn play_sound_blocking(sound_key: &str) -> Result<(), AppError> {
    let decoder = decode(sound_key)?;
    let mut stream = OutputStreamBuilder::open_default_stream().map_err(|e| {
        AppError::msg(format!(
            "Cannot play athan audio: no usable audio output device ({e})"
        ))
    })?;
    // Playback has already completed (or the process is exiting) by the time
    // the stream drops — rodio's drop warning is just noise here.
    stream.log_on_drop(false);
    let sink = Sink::connect_new(stream.mixer());
    sink.append(decoder);
    sink.sleep_until_end();
    Ok(())
}

/// Play an athan sound on a background thread; errors are printed to stderr
/// (mirrors the scheduler's fire-and-forget playback in the Node CLI).
pub fn play_sound_in_background(sound_key: String) {
    std::thread::spawn(move || {
        if let Err(err) = play_sound_blocking(&sound_key) {
            eprintln!("{err}");
        }
    });
}
