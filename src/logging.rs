// SPDX-License-Identifier: GPL-3.0-or-later
//! Small file logger and panic hook.
//!
//! The log lives in `%LOCALAPPDATA%\OpenSwitch\logs\openswitch.log` and is rotated at 1 MB
//! (one old file is kept as `openswitch.1.log`).
//!
//! PRIVACY: never log what the user types. Log events, program names, error codes — not words.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{Level, LevelFilter, Log, Metadata, Record};

const MAX_SIZE: u64 = 1024 * 1024;
const FILE_NAME: &str = "openswitch.log";
const OLD_FILE_NAME: &str = "openswitch.1.log";

pub fn log_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    base.join("OpenSwitch").join("logs")
}

struct FileLogger {
    /// Name of the process role ("tray" / "settings"), written on every line.
    role: &'static str,
    file: Mutex<Option<File>>,
}

impl FileLogger {
    fn open() -> Option<File> {
        let dir = log_dir();
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join(FILE_NAME);
        if std::fs::metadata(&path).map(|m| m.len() > MAX_SIZE).unwrap_or(false) {
            let _ = std::fs::rename(&path, dir.join(OLD_FILE_NAME));
        }
        OpenOptions::new().create(true).append(true).open(path).ok()
    }

    fn write_line(&self, level: Level, target: &str, msg: &std::fmt::Arguments) {
        let Ok(mut guard) = self.file.lock() else { return };
        // Rotate while running, too.
        let too_big = guard.as_ref().and_then(|f| f.metadata().ok()).is_some_and(|m| m.len() > MAX_SIZE);
        if guard.is_none() || too_big {
            *guard = None;
            *guard = FileLogger::open();
        }
        if let Some(f) = guard.as_mut() {
            let _ = writeln!(f, "{} {:<5} [{}] {}: {}", timestamp(), level, self.role, target, msg);
            let _ = f.flush();
        }
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            self.write_line(record.level(), record.target(), record.args());
        }
    }

    fn flush(&self) {
        if let Ok(mut g) = self.file.lock() {
            if let Some(f) = g.as_mut() {
                let _ = f.flush();
            }
        }
    }
}

/// UTC time as `YYYY-MM-DD HH:MM:SS.mmm` without pulling in a date crate.
fn timestamp() -> String {
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = d.as_secs();
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, dd) = civil_from_days(days as i64);
    format!("{y:04}-{m:02}-{dd:02} {:02}:{:02}:{:02}.{:03}Z", rem / 3600, (rem / 60) % 60, rem % 60, d.subsec_millis())
}

/// Days since 1970-01-01 -> (year, month, day). Howard Hinnant's algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Install the file logger and a panic hook that writes the panic (with backtrace) to the log.
/// `role` distinguishes the tray process from the settings window in the shared log file.
pub fn init(role: &'static str) {
    let level = if cfg!(debug_assertions) || std::env::var_os("OPENSWITCH_DEBUG").is_some() {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    let logger = Box::new(FileLogger { role, file: Mutex::new(FileLogger::open()) });
    if log::set_boxed_logger(logger).is_ok() {
        log::set_max_level(level);
    }

    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let bt = std::backtrace::Backtrace::force_capture();
        log::error!("PANIC in thread '{}': {}\n{}", thread.name().unwrap_or("<unnamed>"), info, bt);
        log::logger().flush();
        default_hook(info);
    }));

    log::info!(
        "OpenSwitch {} started ({} {}, {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        if cfg!(debug_assertions) { "debug" } else { "release" }
    );
}

#[cfg(test)]
mod tests {
    use super::civil_from_days;

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
        assert_eq!(civil_from_days(20_775), (2026, 11, 18));
    }
}
