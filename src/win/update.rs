// SPDX-License-Identifier: GPL-3.0-or-later
//! The optional daily check for a new version: runs on a worker thread, tells the user with a
//! tray notification and a menu item. Opening the release page is up to the user.

use std::sync::Mutex;

use openswitch::update::release_page;

use super::*;
use crate::updates::{check_now, Check, State};

static BUSY: AtomicBool = AtomicBool::new(false);
/// Tag of a newer release found by the last check.
static NEWER: Mutex<Option<String>> = Mutex::new(None);

/// A newer release, if the last check found one (for the tray menu).
pub(super) fn newer_version() -> Option<String> {
    NEWER.lock().ok().and_then(|n| n.clone())
}

/// Timer tick: ask GitHub when the user allowed it and a day has passed.
pub(super) fn maybe_check_update(hwnd: HWND) {
    if !with_app(|a| a.engine.settings.check_updates).unwrap_or(false) || !State::load().due() {
        return;
    }
    if BUSY.swap(true, Ordering::SeqCst) {
        return;
    }
    let hwnd = hwnd as usize;
    std::thread::spawn(move || {
        if let Check::Newer(tag) = check_now() {
            if let Ok(mut n) = NEWER.lock() {
                *n = Some(tag);
            }
            unsafe { PostMessageW(hwnd as HWND, WM_UPDATE, 0, 0) };
        }
        BUSY.store(false, Ordering::SeqCst);
    });
}

/// A newer release was found: notify once per version.
pub(super) fn update_found(hwnd: HWND) {
    let Some(tag) = newer_version() else { return };
    let mut st = State::load();
    if st.announced == tag {
        return;
    }
    st.announced = tag.clone();
    st.save();
    let version = tag.trim_start_matches('v');
    unsafe {
        show_balloon(hwnd, &i18n::f("OpenSwitch {} is available", &[&version]), t("Click to open the download page."))
    };
}

pub(super) fn open_new_version() {
    if let Some(tag) = newer_version() {
        sys::open_url(&release_page(&tag));
    }
}
