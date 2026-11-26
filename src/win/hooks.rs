// SPDX-License-Identifier: GPL-3.0-or-later
//! Low-level keyboard and mouse hooks, focus and foreground events, engine actions.

use super::*;

pub(super) fn is_down(vk: u16) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

pub(super) unsafe extern "system" fn kb_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
        // Ignore everything injected (our own input, password managers' auto-type, remote tools...)
        if kb.flags & LLKHF_INJECTED == 0 && kb.vkCode < 256 {
            let msg = wparam as u32;
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let vk = kb.vkCode as u8;
            let started = Instant::now();
            // A panic must never unwind into Windows (that aborts the process) and must never
            // leave a key swallowed: on panic the key is passed through.
            let swallow = catch_unwind(AssertUnwindSafe(|| handle_key(vk, down))).unwrap_or_else(|_| {
                with_app(|app| app.engine.reset_modifiers());
                false
            });
            let ms = started.elapsed().as_millis();
            if ms > SLOW_HOOK_MS {
                log::warn!("slow keyboard hook: {ms} ms");
            }
            if swallow {
                return 1;
            }
        }
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

pub(super) unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let m = wparam as u32;
        if m == WM_LBUTTONDOWN || m == WM_RBUTTONDOWN || m == WM_MBUTTONDOWN || m == WM_XBUTTONDOWN {
            let _ = catch_unwind(|| with_app(|app| app.engine.reset()));
        }
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

/// WinEvent callback (runs on our message loop): the foreground window changed.
pub(super) unsafe extern "system" fn foreground_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _thread: u32,
    _time: u32,
) {
    let _ = catch_unwind(|| set_foreground(hwnd));
}

/// WinEvent callback (runs on our message loop): keyboard focus moved to another control.
pub(super) unsafe extern "system" fn focus_event(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    let _ = catch_unwind(|| on_focus(hwnd, id_object, id_child));
}

/// Find out whether the newly focused control is a password field: Win32 edits right here,
/// everything else on the worker thread (the answer comes as `WM_PASSWORD`).
pub(super) fn on_focus(hwnd: HWND, id_object: i32, id_child: i32) {
    if hwnd.is_null() || !with_app(|a| a.engine.settings.skip_passwords).unwrap_or(false) {
        return;
    }
    let quick =
        if id_object == OBJID_CLIENT && id_child == CHILDID_SELF as i32 { password::win32_edit(hwnd) } else { None };
    let root = unsafe { GetAncestor(hwnd, GA_ROOT) };
    let now = unsafe { GetTickCount64() };
    with_app(|app| {
        app.password_seq += 1;
        app.password_root = root;
        app.password = match quick {
            Some(true) => Password::Yes,
            Some(false) => Password::No,
            None => {
                let q = password::Query { seq: app.password_seq, hwnd, id_object, id_child };
                match &app.password_tx {
                    Some(tx) if tx.send(q).is_ok() => Password::Checking(app.password_seq, now + PASSWORD_WAIT_MS),
                    _ => Password::No,
                }
            }
        };
        if app.password == Password::Yes {
            app.engine.reset();
        }
    });
}

/// Remember the new foreground window and resolve its program name. Called from the
/// WinEvent hook and from `WM_FOREGROUND` (never from inside the keyboard hook: opening
/// another process can be slow).
pub(super) fn set_foreground(fg: HWND) {
    let exe = sys::exe_name(fg);
    with_app(|app| {
        if fg != app.last_fg {
            app.last_fg = fg;
            app.engine.reset();
            if app.password_root != fg {
                // No focus event from this window yet: the style of its focused control
                // is all we know (the focus event usually follows right away).
                let pw =
                    app.engine.settings.skip_passwords && password::win32_edit(sys::focus_target(fg).0) == Some(true);
                app.password = if pw { Password::Yes } else { Password::No };
                app.password_root = fg;
            }
        }
        let rule = exe.as_deref().map_or(AppRule::Normal, |n| app.engine.settings.app_rule(n));
        if fg != app.rules_fg {
            apply_rules(app, fg, exe.as_deref(), rule);
        }
        app.fg_rule = rule;
        app.fg_exe = exe;
        app.fg_known = true;
    });
}

/// Another window came to the front: when the program changed, remember the layout of the
/// one left ("remember the layout") and set the layout of the new one ("start with English",
/// "remember the layout").
fn apply_rules(app: &mut App, fg: HWND, exe: Option<&str>, rule: AppRule) {
    let prev = std::mem::replace(&mut app.rules_fg, fg);
    let prev_exe = std::mem::replace(&mut app.rules_exe, exe.map(str::to_string));
    if prev_exe.as_deref() == exe {
        return; // another window of the same program
    }
    if let Some(prev_exe) = prev_exe {
        if app.engine.settings.app_rule(&prev_exe) == AppRule::Remember && unsafe { IsWindow(prev) } != 0 {
            if let Some(l) = lang_of(&app.hkl, sys::current_hkl(prev)) {
                app.remembered.insert(prev_exe, l);
            }
        }
    }
    let layouts = &app.engine.detector.layouts;
    let target = match rule {
        AppRule::English => layouts.all().find(|&l| layouts.is_latin(l)),
        AppRule::Remember => exe.and_then(|n| app.remembered.get(n).copied()),
        _ => None,
    };
    if let Some(l) = target {
        unsafe { PostMessageW(app.hwnd, WM_SWITCH_LAYOUT, lang_to_wparam(l), 0) };
    }
}

pub(super) fn is_modifier(vk: u8) -> bool {
    matches!(vk, 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
}

/// Returns true if the key must be swallowed. Runs inside the keyboard hook: keep it fast.
pub(super) fn handle_key(vk: u8, down: bool) -> bool {
    let result = with_app(|app| {
        let now = unsafe { GetTickCount64() };
        if now < app.suspended_until {
            app.engine.reset();
            return None;
        }
        let fg = unsafe { GetForegroundWindow() };
        if fg != app.last_fg {
            // The WinEvent has not arrived yet. Do not look the program up here (slow);
            // ask the message loop to do it and treat the window as "excluded" until then.
            // That is the safe choice and lasts only until the message loop runs again,
            // i.e. a few keys at most; manual conversion (Pause) works meanwhile.
            app.last_fg = fg;
            app.engine.reset();
            app.fg_exe = None;
            app.fg_known = false;
            app.fg_rule = AppRule::Normal;
            unsafe { PostMessageW(app.hwnd, WM_FOREGROUND, fg as usize, 0) };
        }
        if app.fg_rule == AppRule::Off {
            // "Off" for this program: not even hotkeys.
            app.engine.reset();
            return None;
        }
        let hkl = sys::current_hkl(fg);
        let layout = lang_of(&app.hkl, hkl);
        if layout.is_none() && hkl != app.unknown_hkl && sys::language_of_hkl(hkl).is_some() {
            // A layout added while we run: read the layouts again on the message loop.
            app.unknown_hkl = hkl;
            unsafe { PostMessageW(app.hwnd, WM_LAYOUT_SEEN, hkl as usize, 0) };
        }
        if down && !is_modifier(vk) {
            app.engine.sync_modifiers(
                is_down(VK_LSHIFT),
                is_down(VK_RSHIFT),
                [is_down(VK_LCONTROL), is_down(VK_RCONTROL)],
                [is_down(VK_LMENU), is_down(VK_RMENU)],
                [is_down(VK_LWIN), is_down(VK_RWIN)],
            );
        }
        let skip = app.engine.settings.skip_passwords;
        if skip && app.password == Password::Yes {
            // Password field: fix nothing, keep nothing typed; only "toggle auto" works.
            app.engine.reset();
            let mut out = app.engine.on_key(vk, down, now, layout, false);
            app.engine.reset();
            out.actions.retain(|a| matches!(a, Action::ToggleAuto));
            out.swallow &= !out.actions.is_empty();
            return Some((out, app.hwnd));
        }
        let checking = skip && matches!(app.password, Password::Checking(_, until) if now < until);
        let auto_allowed = app.fg_known && app.fg_rule != AppRule::Manual && !checking;
        let mut out = app.engine.on_key(vk, down, now, layout, auto_allowed);
        if out.auto_fix && app.engine.settings.sound {
            unsafe { PostMessageW(app.hwnd, WM_SOUND, 0, 0) };
        }
        if let Some(fix) = out.fixed.take() {
            history::push(&mut app.history, fix);
            unsafe { PostMessageW(app.hwnd, WM_HISTORY, 0, 0) };
        }
        Some((out, app.hwnd))
    });
    let Some(Some((out, hwnd))) = result else { return false };
    perform(out.actions, hwnd);
    out.swallow
}

/// Execute engine actions. Typing is sent right away (it must reach the window before the
/// next physical key); everything else is deferred so the hook returns quickly.
pub(super) fn perform(actions: Vec<Action>, hwnd: HWND) {
    let mut inputs = Vec::new();
    for a in actions {
        match a {
            Action::Backspace(n) => (0..n).for_each(|_| push_tap(&mut inputs, VK_BACK)),
            Action::Type(t) => push_text(&mut inputs, &t),
            Action::Tap(vk) => push_tap(&mut inputs, vk as u16),
            Action::SwitchLayout(l) => unsafe {
                PostMessageW(hwnd, WM_SWITCH_LAYOUT, lang_to_wparam(l), 0);
            },
            Action::ConvertSelection => {
                if !SELECTION_BUSY.swap(true, Ordering::SeqCst) {
                    let layouts = with_app(|a| a.engine.detector.layouts.clone()).unwrap_or_default();
                    std::thread::spawn(move || {
                        if catch_unwind(AssertUnwindSafe(|| convert_selection(&layouts))).is_err() {
                            log::error!("convert_selection panicked");
                        }
                        SELECTION_BUSY.store(false, Ordering::SeqCst);
                    });
                }
            }
            Action::ToggleAuto => unsafe {
                PostMessageW(hwnd, WM_TOGGLE_AUTO, 0, 0);
            },
            Action::AddException(word) => file_job(FileJob::Exception(word)),
            Action::Learn(word, code) => file_job(FileJob::Always(word, Some(code))),
            Action::Forget(word) => file_job(FileJob::Always(word, None)),
        }
    }
    send(&inputs);
}

pub(super) enum FileJob {
    Exception(String),
    /// Learn (with the layout code) or forget (None) a word.
    Always(String, Option<String>),
}

/// File writes requested from the hook run on one worker thread, in order (a word learned
/// and forgotten right after must end up forgotten).
pub(super) fn file_job(job: FileJob) {
    use std::sync::mpsc::{channel, Sender};
    use std::sync::{Mutex, OnceLock};
    static TX: OnceLock<Mutex<Sender<FileJob>>> = OnceLock::new();
    let tx = TX.get_or_init(|| {
        let (tx, rx) = channel::<FileJob>();
        std::thread::spawn(move || {
            for job in rx {
                match job {
                    FileJob::Exception(w) => sys::append_exception(&w),
                    FileJob::Always(w, code) => sys::update_always(&w, code.as_deref()),
                }
            }
        });
        Mutex::new(tx)
    });
    if let Ok(tx) = tx.lock() {
        let _ = tx.send(job);
    }
}

pub(super) fn lang_to_wparam(l: Lang) -> WPARAM {
    l.index()
}

pub(super) fn wparam_to_lang(w: WPARAM) -> Lang {
    Lang(w as u8)
}

pub(super) unsafe fn install_hooks() {
    let hinst = GetModuleHandleW(null());
    let old = KB_HOOK.swap(SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb_proc), hinst, 0), Ordering::SeqCst);
    if !old.is_null() {
        UnhookWindowsHookEx(old);
    }
    let old = MOUSE_HOOK.swap(SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hinst, 0), Ordering::SeqCst);
    if !old.is_null() {
        UnhookWindowsHookEx(old);
    }
}

pub(super) unsafe fn remove_hooks() {
    for h in [&KB_HOOK, &MOUSE_HOOK] {
        let old = h.swap(null_mut(), Ordering::SeqCst);
        if !old.is_null() {
            UnhookWindowsHookEx(old);
        }
    }
}
