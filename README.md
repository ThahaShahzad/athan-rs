# athan-rs

Cross-platform command-line app that calculates Islamic prayer times and plays
the athan (call to prayer) at those times. A Rust port of the
[athan-web](../athan-web) PWA, with the same behavior as the Node
[athan-cli](../athan-cli).

The prayer-time math core (`src/prayer_times/`) is a faithful port of the
batoulapps [`adhan`](https://github.com/batoulapps/adhan) JavaScript library
(v4.4.3), verified against 600 reference fixtures generated from the JS
package (12 calculation methods × 2 madhabs × 5 locations × 5 dates) with
zero deviation.

## Requirements

- Rust (stable, edition 2024)
- An audio output device (playback is via [rodio](https://github.com/RustAudio/rodio);
  on Linux this means ALSA, e.g. PulseAudio/PipeWire's ALSA layer — no
  external player binary is needed, the MP3s are bundled into the executable)

## Install

One-liner (Linux/macOS — downloads the latest release binary to
`~/.local/bin/athan`):

```sh
curl -fsSL https://raw.githubusercontent.com/ThahaShahzad/athan-rs/main/install.sh | sh
```

Or build from source:

```sh
cargo build --release
cargo install --path .   # installs the `athan` command
```

## Usage

```
athan                  TUI dashboard (default)
athan start            foreground scheduler — plays athan at prayer times
athan times [date]     prayer times for today or a date (YYYY-MM-DD)
athan next             next prayer and countdown
athan config show      show current configuration
athan config set <key> <value>
athan sounds           list available athan sounds
athan test <sound>     play a sound once
athan autostart enable|disable|status
                       manage the login/boot autostart service
```

On first run (any command that needs a location) you'll be prompted to enter
latitude/longitude manually or to detect an approximate location from your IP
address via [ipapi.co](https://ipapi.co) (only with your consent).

### Running at startup (daemon-style)

`athan start` runs the scheduler in the foreground — play athan at prayer
times, recalculate at midnight, and survive restarts without replaying a
prayer (per-day `lastPlayed` state is persisted). To run it on login/boot,
let athan register itself with your platform's service manager:

```sh
athan autostart enable    # install + start the service
athan autostart status    # installed? loaded/running?
athan autostart disable   # stop + remove the service
```

How it works per OS:

- **Linux** — installs a systemd *user* unit at
  `~/.config/systemd/user/athan.service` (`ExecStart=<athan> start`,
  `Restart=on-failure`) and runs `systemctl --user enable --now`.
  To survive logout, enable lingering: `loginctl enable-linger "$USER"`.
- **macOS** — installs a LaunchAgent at
  `~/Library/LaunchAgents/org.athan.cli.plist` (`RunAtLoad` + `KeepAlive`)
  and loads it with `launchctl`.
- **Windows** — drops a best-effort `athan.bat` into the Startup folder
  (`%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`).

### TUI keys

- `q` — quit
- `t` — test-play the configured athan sound
- `d` — toggle the athan scheduler on/off

The dashboard shows a live clock with the Hijri date (tabular calendar,
labelled approximate — it can differ by ±1 day from Umm al-Qura), a countdown
to the next prayer, all six prayer times (next one highlighted), and the
night-thirds (Qiyam) window — the last third of the night, the recommended
time for Tahajjud.

## Configuration

Config lives at:

- Linux/macOS: `$XDG_CONFIG_HOME/athan/config.json` (or `~/.config/athan/config.json`)
- Windows: `%APPDATA%\athan\config.json`

```json
{
  "latitude": 40.7128,
  "longitude": -74.006,
  "method": "NorthAmerica",
  "madhab": "Shafi",
  "athanSound": "adhan-alaqsa",
  "fajrSound": "adhan-fajr",
  "athanEnabled": true
}
```

The scheduler's `lastPlayed` state is stored alongside it in `state.json`
(same directory). The schema is identical to the Node athan-cli, so both apps
can share one config directory.

Valid methods: `NorthAmerica`, `MuslimWorldLeague`, `Egyptian`, `UmmAlQura`,
`Karachi`, `Dubai`, `Kuwait`, `Qatar`, `Singapore`, `Tehran`, `Turkey`,
`MoonsightingCommittee`. Valid madhabs: `Shafi`, `Hanafi`.

## Sounds

Bundled athan recordings (see `athan sounds`):

- `adhan-alaqsa` — Al-Aqsa
- `adhan-fajr` — Al-Aqsa Fajr (played for Fajr)
- `adhan-makkah` — Makkah
- `adhan-madinah` — Madinah
- `adhan-mishary` — Mishary Rashid

## Development

```sh
cargo test                  # includes the 600-row JS-fixture correctness gate
cargo clippy --all-targets  # must stay warning-free
```

Regenerate the reference fixtures (requires Node.js):

```sh
cd scripts && npm install && cd ..
TZ=UTC node scripts/gen-fixtures.mjs
```

## Releases (maintainers)

Push a `v*` tag to trigger `.github/workflows/release.yml` — it builds
stripped release binaries for linux-x86_64, macos-x86_64, macos-aarch64, and
windows-x86_64 and attaches them to a GitHub release as
`athan-<os>-<arch>` (`.exe` on Windows), which is what `install.sh`
downloads:

```sh
git tag v0.1.0 && git push origin v0.1.0
```
