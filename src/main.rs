//! OpenSwitch — free, open-source automatic keyboard layout switcher (EN/RU) for Windows.
//! Copyright (C) 2026 OpenSwitch contributors
//!
//! This program is free software: you can redistribute it and/or modify it under the terms of
//! the GNU General Public License as published by the Free Software Foundation, either
//! version 3 of the License, or (at your option) any later version. See LICENSE.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod sys;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    if std::env::args().any(|a| a == "--settings" || a == "/settings") {
        ui::run();
    } else {
        win::run();
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("OpenSwitch работает только в Windows. Здесь можно запускать тесты: cargo test");
}
