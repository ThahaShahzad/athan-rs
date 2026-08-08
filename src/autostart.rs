//! OS autostart integration — run `athan start` on login/boot.
//!
//!   - Linux:   systemd user unit ~/.config/systemd/user/athan.service
//!   - macOS:   LaunchAgent ~/Library/LaunchAgents/org.athan.cli.plist
//!   - Windows: startup folder athan.bat (best-effort)

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Absolute path of the running executable (what the service will launch).
fn current_exe() -> Result<PathBuf, AppError> {
    let exe = std::env::current_exe()?;
    Ok(exe.canonicalize().unwrap_or(exe))
}

pub fn enable() -> Result<(), AppError> {
    imp::enable(&current_exe()?)
}

pub fn disable() -> Result<(), AppError> {
    imp::disable()
}

pub fn status() -> Result<(), AppError> {
    imp::status()
}

/// systemd user unit content (kept cfg-free so tests can exercise it anywhere).
fn systemd_unit(exe: &Path) -> String {
    format!(
        "[Unit]\n\
         Description=Athan prayer-time scheduler\n\n\
         [Service]\n\
         ExecStart={} start\n\
         Restart=on-failure\n\n\
         [Install]\n\
         WantedBy=default.target\n",
        exe.display()
    )
}

/// macOS LaunchAgent plist content (kept cfg-free so tests can exercise it anywhere).
#[allow(dead_code)] // used by the macOS imp and by tests on all platforms
fn launchd_plist(exe: &Path) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n\
         <dict>\n\
         \t<key>Label</key>\n\
         \t<string>org.athan.cli</string>\n\
         \t<key>ProgramArguments</key>\n\
         \t<array>\n\
         \t\t<string>{}</string>\n\
         \t\t<string>start</string>\n\
         \t</array>\n\
         \t<key>RunAtLoad</key>\n\
         \t<true/>\n\
         \t<key>KeepAlive</key>\n\
         \t<true/>\n\
         </dict>\n\
         </plist>\n",
        exe.display()
    )
}

/// Windows startup-folder batch file content (kept cfg-free for tests).
#[allow(dead_code)] // used by the Windows imp and by tests on all platforms
fn startup_bat(exe: &Path) -> String {
    format!("\"{}\" start\r\n", exe.display())
}

// ---------------------------------------------------------------------------
// Linux (systemd user service)
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
mod imp {
    use std::process::Command;

    use super::*;

    const UNIT_NAME: &str = "athan.service";

    fn unit_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("systemd")
            .join("user")
            .join(UNIT_NAME)
    }

    /// Run `systemctl --user <args>`, returning a clean error on failure.
    fn systemctl(args: &[&str]) -> Result<String, AppError> {
        let output = Command::new("systemctl")
            .arg("--user")
            .args(args)
            .output()
            .map_err(|e| {
                AppError::msg(format!(
                    "Failed to run systemctl (is systemd installed?): {e}"
                ))
            })?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(AppError::msg(format!(
                "systemctl --user {} failed: {}",
                args.join(" "),
                if stderr.is_empty() {
                    format!("exit status {}", output.status)
                } else {
                    stderr
                }
            )))
        }
    }

    /// Like `systemctl`, but nonzero exit just returns the stdout (for status probes).
    fn systemctl_probe(args: &[&str]) -> Option<String> {
        Command::new("systemctl")
            .arg("--user")
            .args(args)
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    }

    pub fn enable(exe: &Path) -> Result<(), AppError> {
        let path = unit_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, systemd_unit(exe))?;
        systemctl(&["daemon-reload"])?;
        systemctl(&["enable", "--now", UNIT_NAME])?;
        println!("Autostart enabled: {}", path.display());
        println!("The athan scheduler is now running as a systemd user service.");
        Ok(())
    }

    pub fn disable() -> Result<(), AppError> {
        let path = unit_path();
        if !path.exists() {
            println!("Autostart is not enabled ({} not found).", path.display());
            return Ok(());
        }
        systemctl(&["disable", "--now", UNIT_NAME])?;
        fs::remove_file(&path)?;
        systemctl(&["daemon-reload"])?;
        println!("Autostart disabled and unit removed: {}", path.display());
        Ok(())
    }

    pub fn status() -> Result<(), AppError> {
        let path = unit_path();
        println!("Unit file: {}", path.display());
        println!("Installed: {}", if path.exists() { "yes" } else { "no" });
        match systemctl_probe(&["is-enabled", UNIT_NAME]) {
            Some(s) if !s.is_empty() => println!("Enabled:   {s}"),
            _ => println!("Enabled:   unknown (systemctl unavailable)"),
        }
        match systemctl_probe(&["is-active", UNIT_NAME]) {
            Some(s) if !s.is_empty() => println!("Active:    {s}"),
            _ => println!("Active:    unknown (systemctl unavailable)"),
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// macOS (LaunchAgent)
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod imp {
    use std::process::Command;

    use super::*;

    const LABEL: &str = "org.athan.cli";

    fn plist_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library")
            .join("LaunchAgents")
            .join("org.athan.cli.plist")
    }

    fn launchctl(args: &[&str]) -> Result<(), AppError> {
        let output = Command::new("launchctl")
            .args(args)
            .output()
            .map_err(|e| AppError::msg(format!("Failed to run launchctl: {e}")))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(AppError::msg(format!(
                "launchctl {} failed: {}",
                args.join(" "),
                if stderr.is_empty() {
                    format!("exit status {}", output.status)
                } else {
                    stderr
                }
            )))
        }
    }

    pub fn enable(exe: &Path) -> Result<(), AppError> {
        let path = plist_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, launchd_plist(exe))?;
        launchctl(&["load", &path.to_string_lossy()])?;
        println!("Autostart enabled: {}", path.display());
        println!("The athan scheduler is now running as a launchd agent.");
        Ok(())
    }

    pub fn disable() -> Result<(), AppError> {
        let path = plist_path();
        if !path.exists() {
            println!("Autostart is not enabled ({} not found).", path.display());
            return Ok(());
        }
        // Best-effort unload — the agent may not be loaded right now.
        let _ = Command::new("launchctl")
            .args(["unload", &path.to_string_lossy()])
            .output();
        fs::remove_file(&path)?;
        println!("Autostart disabled and agent removed: {}", path.display());
        Ok(())
    }

    pub fn status() -> Result<(), AppError> {
        let path = plist_path();
        println!("Agent file: {}", path.display());
        println!("Installed:  {}", if path.exists() { "yes" } else { "no" });
        let loaded = Command::new("launchctl")
            .args(["list"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(LABEL))
            .unwrap_or(false);
        println!("Loaded:     {}", if loaded { "yes" } else { "no" });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Windows (startup folder batch file)
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
mod imp {
    use super::*;

    fn bat_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
            .join("Startup")
            .join("athan.bat")
    }

    pub fn enable(exe: &Path) -> Result<(), AppError> {
        let path = bat_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, startup_bat(exe))?;
        println!("Autostart enabled: {}", path.display());
        println!("The athan scheduler will start on login.");
        Ok(())
    }

    pub fn disable() -> Result<(), AppError> {
        let path = bat_path();
        if !path.exists() {
            println!("Autostart is not enabled ({} not found).", path.display());
            return Ok(());
        }
        fs::remove_file(&path)?;
        println!("Autostart disabled: {}", path.display());
        Ok(())
    }

    pub fn status() -> Result<(), AppError> {
        let path = bat_path();
        println!("Startup file: {}", path.display());
        println!("Installed:    {}", if path.exists() { "yes" } else { "no" });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn systemd_unit_contains_exe_and_start() {
        let unit = systemd_unit(Path::new("/home/user/.local/bin/athan"));
        assert!(unit.contains("ExecStart=/home/user/.local/bin/athan start"));
        assert!(unit.contains("WantedBy=default.target"));
        assert!(unit.contains("Restart=on-failure"));
    }

    #[test]
    fn launchd_plist_contains_exe_start_and_flags() {
        let plist = launchd_plist(Path::new("/usr/local/bin/athan"));
        assert!(plist.contains("<string>/usr/local/bin/athan</string>"));
        assert!(plist.contains("<string>start</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
        assert!(plist.contains("<key>KeepAlive</key>"));
        assert!(plist.contains("org.athan.cli"));
    }

    #[test]
    fn startup_bat_quotes_exe_and_runs_start() {
        let bat = startup_bat(Path::new("C:\\Program Files\\athan\\athan.exe"));
        assert_eq!(bat, "\"C:\\Program Files\\athan\\athan.exe\" start\r\n");
    }
}
