// SPDX-License-Identifier: GPL-3.0-or-later
//! OpenSwitch — free, open-source automatic keyboard layout switcher (EN/RU) for Windows.
//! Copyright (C) 2026 OpenSwitch contributors
//!
//! This program is free software: you can redistribute it and/or modify it under the terms of
//! the GNU General Public License as published by the Free Software Foundation, either
//! version 3 of the License, or (at your option) any later version. See LICENSE.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

#[cfg(windows)]
mod admin;
#[cfg(windows)]
mod clipboard;
#[cfg_attr(not(windows), allow(dead_code))]
mod logging;
#[cfg(windows)]
mod password;
#[cfg(windows)]
mod setup;
#[cfg(windows)]
mod sys;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod win;

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == admin::HELPER) {
        // Elevated helper started by the settings window: create or delete the task.
        logging::init("admin");
        std::process::exit(admin::helper(args.get(i + 1).map(|s| s.as_str()) == Some("on")));
    }
    if std::env::args().any(|a| a == "--settings" || a == "/settings") {
        logging::init("settings");
        ui::run();
    } else if std::env::args().any(|a| a == "--setup" || a == "/setup") {
        logging::init("setup");
        setup::run();
    } else {
        logging::init("tray");
        win::run();
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("OpenSwitch работает только в Windows. Здесь можно запускать тесты: cargo test");
}
