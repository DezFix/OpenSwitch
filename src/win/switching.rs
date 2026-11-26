// SPDX-License-Identifier: GPL-3.0-or-later
//! Verified layout switching: ask the window, check, fall back to the system shortcut.

use super::*;

/// Switch the foreground window to `lang` and make sure it really happened:
/// first ask the window politely (WM_INPUTLANGCHANGEREQUEST), check after a moment and,
/// if the program ignored the request, press the system layout shortcut (Alt+Shift /
/// Ctrl+Shift / Win+Space). Programs that needed the shortcut get it right away next time.
pub(super) fn switch_layout(lang: Lang) {
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_null() || current_lang(fg) == Some(lang) {
        return;
    }
    let Some(target) = target_hkl(lang) else {
        log::info!("layout {lang:?} is gone, cannot switch");
        with_app(|a| a.engine.cancel_forced());
        return;
    };
    let mode = with_app(|a| match &a.fg_exe {
        Some(e) if a.unreliable_apps.contains(e) => 2,
        Some(e) if a.slow_apps.contains(e) => 1,
        _ => 0,
    })
    .unwrap_or(0);
    let before = current_lang(fg);
    match mode {
        2 => {
            // We cannot read this program's layout reliably: just ask and hope.
            sys::request_layout(target);
            return;
        }
        1 => sys::press_system_toggle(),
        _ => sys::request_layout(target),
    }
    let hwnd = with_app(|a| {
        a.pending = Some(Pending { lang, fg, presses: mode, before });
        a.hwnd
    });
    if let Some(h) = hwnd {
        unsafe { SetTimer(h, TIMER_VERIFY, VERIFY_MS, None) };
    }
}

/// Which of our layouts `hkl` is.
pub(super) fn lang_of(hkls: &[HKL], hkl: HKL) -> Option<Lang> {
    hkls.iter().position(|&h| h == hkl).map(|i| Lang(i as u8))
}

/// The layout of the focused window of `fg` (None: a layout we do not switch, e.g. German).
pub(super) fn current_lang(fg: HWND) -> Option<Lang> {
    let hkl = sys::current_hkl(fg);
    with_app(|a| lang_of(&a.hkl, hkl)).flatten()
}

/// The Windows layout to switch to for `lang`, if it is still installed.
pub(super) fn target_hkl(lang: Lang) -> Option<HKL> {
    let hkl = with_app(|a| a.hkl.get(lang.index()).copied()).flatten()?;
    sys::installed_layouts().contains(&hkl).then_some(hkl)
}

/// Read every installed layout OpenSwitch has a dictionary for (their real key maps, so
/// customised layouts convert correctly). Runs at start-up, after the settings changed and
/// when a new layout shows up; never inside the keyboard hook.
pub(super) fn init_layouts() {
    let installed = sys::installed_layouts();
    log::info!(
        "installed layouts: {}",
        installed
            .iter()
            .map(|h| format!("{} {:#010x}", sys::layout_code(*h), *h as usize))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let disabled = with_app(|a| a.engine.settings.disabled_layouts.clone()).unwrap_or_default();
    let (mut layouts, hkls) = sys::build_layouts(&installed, &disabled);
    for (l, h) in layouts.list.iter().zip(&hkls) {
        log::info!(
            "layout {} ({:#010x}): {:?}, {} letter keys{}",
            l.code,
            *h as usize,
            l.languages,
            l.map.letter_count(),
            if l.target { "" } else { ", not switched to" }
        );
    }
    if layouts.len() < 2 {
        log::warn!("automatic switching needs at least two layouts of different languages (EN, RU, UA, BY)");
    }
    with_app(|a| {
        // Keep "last used" for layouts that are still there.
        for (i, h) in hkls.iter().enumerate() {
            if let Some(j) = a.hkl.iter().position(|x| x == h) {
                layouts.recency[i] = a.engine.detector.layouts.recency.get(j).copied().unwrap_or(0);
            }
        }
        a.engine.detector.layouts = layouts;
        a.hkl = hkls;
        a.unknown_hkl = null_mut();
        a.engine.reset();
    });
}

pub(super) fn verify_layout(hwnd: HWND) {
    let n_layouts = sys::installed_layouts().len().clamp(2, 8) as u32;
    // Number of system-shortcut presses to do now.
    let presses = with_app(|a| {
        let Some(p) = &mut a.pending else { return 0 };
        let fg = unsafe { GetForegroundWindow() };
        let now = lang_of(&a.hkl, sys::current_hkl(fg));
        if fg != p.fg {
            a.pending = None;
            return 0;
        }
        if now == Some(p.lang) {
            if p.presses > 0 {
                if let Some(e) = &a.fg_exe {
                    if a.slow_apps.insert(e.clone()) {
                        log::info!("{e}: ignores WM_INPUTLANGCHANGEREQUEST, will use the system shortcut");
                    }
                }
            }
            a.pending = None;
            return 0;
        }
        if p.presses == 0 {
            // The program ignored the request: try the system shortcut.
            p.before = now;
            p.presses = 1;
            return 1;
        }
        if now != p.before && p.presses < n_layouts {
            // Moved, but to some third layout: keep cycling.
            p.before = now;
            p.presses += 1;
            return 1;
        }
        // The shortcut changed nothing we can see: this program reports its layout wrongly.
        // Complete the cycle so the layout ends where the original request put it,
        // and never second-guess this program again.
        if let Some(e) = &a.fg_exe {
            log::info!("{e}: keyboard layout cannot be read back, will not verify switches");
            a.unreliable_apps.insert(e.clone());
            a.slow_apps.remove(e);
        }
        let undo = n_layouts.saturating_sub(p.presses);
        a.pending = None;
        a.engine.cancel_forced();
        undo
    })
    .unwrap_or(0);
    for _ in 0..presses {
        sys::press_system_toggle();
    }
    let done = with_app(|a| a.pending.is_none()).unwrap_or(true);
    if done {
        unsafe { KillTimer(hwnd, TIMER_VERIFY) };
    }
}
