// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows tray app: low-level keyboard/mouse hooks, input injection, verified layout
//! switching, tray icon and menu.
//!
//! Rule for the hook callbacks (`kb_proc`, `mouse_proc`): they must return within a few
//! milliseconds, or Windows silently removes the hook. They only touch cached state and
//! call `SendInput`; everything slow (process names, files, layout switching, clipboard)
//! happens on the message loop after the hook returned, or on a worker thread.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::c_void;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::time::Instant;

use openswitch::config::AppRule;
use openswitch::detect::Detector;
use openswitch::engine::{Action, Engine, Fix};
use openswitch::i18n::{self, t};
use openswitch::layout::{Lang, Layouts};

use crate::admin;
use crate::clipboard;
use crate::history;
use crate::password::{self, WM_PASSWORD};
use crate::sys::{self, w, APP_NAME, OUR_MARK, WINDOW_CLASS, WM_HISTORY_CLEAR, WM_QUIT_APP, WM_RELOAD, WM_SUSPEND};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

mod hooks;
mod input;
mod selection;
mod sound;
mod switching;
mod tray;

use hooks::*;
use input::*;
use selection::*;
use switching::*;
use tray::*;

const WM_TRAY: u32 = WM_APP + 1;
const WM_TOGGLE_AUTO: u32 = WM_APP + 2;
const WM_SWITCH_LAYOUT: u32 = WM_APP + 4;
const WM_FOREGROUND: u32 = WM_APP + 5;
const WM_LAYOUT_SEEN: u32 = WM_APP + 6;
const WM_SOUND: u32 = WM_APP + 10;
const WM_HISTORY: u32 = WM_APP + 12;
const TIMER_ICON: usize = 1;
const TIMER_REHOOK: usize = 2;
const TIMER_VERIFY: usize = 3;
const TIMER_WATCH: usize = 4;
/// How often the settings files are checked for edits made outside the settings window.
const WATCH_MS: u32 = 2000;
/// How long one WM_SUSPEND request lasts (the settings window repeats it while recording).
const SUSPEND_MS: u64 = 3000;
const VERIFY_MS: u32 = 300;
/// A hook call slower than this is logged (Windows' own limit is around 300-1000 ms).
const SLOW_HOOK_MS: u128 = 30;
/// How long automatic fixes wait for the password check of a newly focused field.
const PASSWORD_WAIT_MS: u64 = 500;

const ID_AUTO: usize = 100;
const ID_STARTUP: usize = 101;
const ID_SETTINGS: usize = 102;
const ID_ABOUT: usize = 106;
const ID_EXIT: usize = 107;

/// A layout switch we asked for and still have to confirm.
struct Pending {
    lang: Lang,
    fg: HWND,
    /// System shortcut presses done so far (0 = only asked the window).
    presses: u32,
    /// Layout seen before the last press.
    before: Option<Lang>,
}

/// Is the focused field a password field?
#[derive(Clone, Copy, PartialEq)]
enum Password {
    No,
    Yes,
    /// Asked the worker thread (request number, give up at this tick).
    Checking(usize, u64),
}

struct App {
    engine: Engine,
    hwnd: HWND,
    last_fg: HWND,
    fg_exe: Option<String>,
    /// Rule of the foreground program (Normal while unknown).
    fg_rule: AppRule,
    /// The window and program the program rules were last applied to.
    rules_fg: HWND,
    rules_exe: Option<String>,
    /// Layout each "remember the layout" program had when the user left it.
    remembered: HashMap<String, Lang>,
    /// The last automatic fixes (memory only) and the shared block the settings window reads.
    history: VecDeque<Fix>,
    history_map: Option<history::Writer>,
    /// False between a foreground change seen by the hook and the program name lookup.
    fg_known: bool,
    /// What the tray icon shows: layout code (EN/RU/UA/...), its side, auto switching on/off.
    icon_state: Option<(&'static str, Option<openswitch::layout::Language>, bool)>,
    /// Windows layout handle of each layout in `engine.detector.layouts` (same order).
    hkl: Vec<HKL>,
    /// A supported layout we have not read yet (installed while running): rebuild once.
    unknown_hkl: HKL,
    icon: HICON,
    taskbar_created: u32,
    pending: Option<Pending>,
    /// Programs that ignore WM_INPUTLANGCHANGEREQUEST: use the system shortcut there right away.
    slow_apps: HashSet<String>,
    /// Programs whose layout cannot be read back: never press the shortcut there.
    unreliable_apps: HashSet<String>,
    /// Keys are passed through untouched until this tick (the settings window records a hotkey).
    suspended_until: u64,
    /// Modification times of settings.ini / exceptions.txt when they were last read.
    files_stamp: [Option<std::time::SystemTime>; 3],
    password: Password,
    /// Number of the last focus event (to drop late answers of the worker).
    password_seq: usize,
    /// Top-level window of the last focus event.
    password_root: HWND,
    password_tx: Option<Sender<password::Query>>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}
static KB_HOOK: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static MOUSE_HOOK: AtomicPtr<c_void> = AtomicPtr::new(null_mut());
static SELECTION_BUSY: AtomicBool = AtomicBool::new(false);
static MAIN_HWND: AtomicUsize = AtomicUsize::new(0);

fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok().and_then(|mut a| a.as_mut().map(f)))
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Never let a panic unwind into Windows.
    catch_unwind(AssertUnwindSafe(|| wnd_proc_inner(hwnd, msg, wparam, lparam))).unwrap_or_else(|_| {
        log::error!("panic while handling window message {msg:#x}");
        0
    })
}

unsafe fn wnd_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAY => {
            match lparam as u32 {
                WM_LBUTTONUP => {
                    let auto = with_app(|a| a.engine.settings.auto_switch).unwrap_or(true);
                    set_auto(!auto);
                }
                WM_RBUTTONUP | WM_CONTEXTMENU => show_menu(hwnd),
                _ => {}
            }
            0
        }
        WM_TOGGLE_AUTO => {
            let auto = with_app(|a| a.engine.settings.auto_switch).unwrap_or(true);
            set_auto(!auto);
            0
        }
        WM_RELOAD => {
            reload();
            0
        }
        WM_HISTORY => {
            with_app(|app| {
                if let Some(map) = &app.history_map {
                    map.write(&app.history);
                }
            });
            0
        }
        WM_HISTORY_CLEAR => {
            with_app(|app| {
                app.history.clear();
                if let Some(map) = &app.history_map {
                    map.write(&app.history);
                }
            });
            0
        }
        WM_SOUND => {
            sound::click();
            0
        }
        WM_QUIT_APP => {
            log::info!("asked to quit (restart with other rights)");
            DestroyWindow(hwnd);
            0
        }
        WM_SUSPEND => {
            let until = if wparam != 0 { GetTickCount64() + SUSPEND_MS } else { 0 };
            with_app(|app| {
                app.suspended_until = until;
                app.engine.reset_modifiers();
            });
            0
        }
        WM_SWITCH_LAYOUT => {
            switch_layout(wparam_to_lang(wparam));
            0
        }
        WM_LAYOUT_SEEN => {
            init_layouts();
            0
        }
        WM_PASSWORD => {
            with_app(|app| {
                if matches!(app.password, Password::Checking(seq, _) if seq == wparam) {
                    app.password = if lparam != 0 { Password::Yes } else { Password::No };
                    if lparam != 0 {
                        app.engine.reset();
                    }
                }
            });
            0
        }
        WM_FOREGROUND => {
            // Posted by the keyboard hook; ignore it if the user has switched again since.
            let fg = wparam as HWND;
            if GetForegroundWindow() == fg {
                set_foreground(fg);
            }
            0
        }
        WM_TIMER => {
            match wparam {
                TIMER_ICON => {
                    with_app(|app| update_tray(app, false, false));
                }
                TIMER_VERIFY => verify_layout(hwnd),
                TIMER_WATCH => {
                    // settings.ini or exceptions.txt edited by hand: apply without asking.
                    let changed = with_app(|app| app.files_stamp != sys::files_stamp()).unwrap_or(false);
                    if changed {
                        let warn = reload();
                        if !warn.is_empty() {
                            log::warn!("settings.ini: {}", warn.join("; "));
                        }
                    }
                }
                TIMER_REHOOK => {
                    // Windows silently drops low-level hooks that were slow once; re-install periodically.
                    install_hooks();
                }
                _ => {}
            }
            0
        }
        WM_DESTROY => {
            remove_tray(hwnd);
            PostQuitMessage(0);
            0
        }
        _ => {
            let tc = with_app(|a| a.taskbar_created).unwrap_or(0);
            if tc != 0 && msg == tc {
                with_app(|app| update_tray(app, true, true));
                return 0;
            }
            DefWindowProcW(hwnd, msg, wparam, lparam)
        }
    }
}

/// Take the single-instance mutex; null if another copy holds it. A copy running with
/// administrator rights makes the call fail with "access denied" instead of "already exists".
unsafe fn single_instance() -> HANDLE {
    let m = CreateMutexW(null(), 1, w("Local\\OpenSwitch.SingleInstance").as_ptr());
    let err = GetLastError();
    if m.is_null() || err == ERROR_ALREADY_EXISTS || err == ERROR_ACCESS_DENIED {
        if !m.is_null() {
            CloseHandle(m);
        }
        return null_mut();
    }
    m
}

pub fn run() {
    unsafe {
        let from_task = std::env::args().any(|a| a == admin::FROM_TASK);
        let elevated = admin::is_elevated();
        let mut mutex = single_instance();
        if from_task {
            // Started by the task: the copy that ran the task may still be closing.
            for _ in 0..30 {
                if !mutex.is_null() {
                    break;
                }
                Sleep(100);
                mutex = single_instance();
            }
        }
        if mutex.is_null() {
            if !from_task {
                // Already running: open the settings window instead of a second tray icon.
                log::info!("already running, opening settings instead");
                sys::spawn_self(&["--settings"]);
            }
            return;
        }
        // DPI awareness (Per-Monitor V2) is declared in the application manifest.
        sys::migrate_startup();

        let (settings, warnings) = sys::load_settings();
        if settings.run_as_admin && !elevated && !from_task {
            // "Run as administrator" is on: let the task start the elevated copy (no UAC).
            ReleaseMutex(mutex);
            CloseHandle(mutex);
            if admin::run_task() {
                log::info!("started the administrator task, exiting");
                return;
            }
            log::warn!("could not start the administrator task, running without administrator rights");
            mutex = single_instance();
            if mutex.is_null() {
                return;
            }
        }
        if settings.run_as_admin && elevated && sys::startup_enabled() {
            // The task starts us at logon: a second start from the Run key is not needed.
            sys::set_startup(false);
        }
        log::info!("administrator rights: {elevated}");
        i18n::set(i18n::resolve(&settings.language));
        let mut detector = Detector::new();
        detector.never_switch = sys::load_exceptions();
        detector.always_switch = sys::load_always();
        detector.sensitivity = settings.sensitivity;
        detector.single_letters = settings.single_letters;
        detector.skip_code = settings.skip_code;
        let mut engine = Engine::new(detector, settings);
        engine.caps = (GetKeyState(VK_CAPITAL as i32) & 1) != 0;

        let hinst = GetModuleHandleW(null());
        let class = w(WINDOW_CLASS);
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: null_mut(),
            hCursor: null_mut(),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            class.as_ptr(),
            w(APP_NAME).as_ptr(),
            WS_POPUP,
            0,
            0,
            0,
            0,
            null_mut(),
            null_mut(),
            hinst,
            null(),
        );
        if hwnd.is_null() {
            message_box(null_mut(), t("Could not create the window."), MB_ICONERROR);
            return;
        }
        MAIN_HWND.store(hwnd as usize, Ordering::SeqCst);
        let taskbar_created = RegisterWindowMessageW(w("TaskbarCreated").as_ptr());
        // With administrator rights Windows drops messages from normal programs (the shell,
        // our settings window started by hand): let ours through.
        for m in [WM_TRAY, WM_RELOAD, WM_SUSPEND, WM_QUIT_APP, WM_HISTORY_CLEAR, taskbar_created] {
            ChangeWindowMessageFilterEx(hwnd, m, MSGFLT_ALLOW, null_mut());
        }

        APP.with(|a| {
            *a.borrow_mut() = Some(App {
                engine,
                hwnd,
                last_fg: null_mut(),
                fg_exe: None,
                fg_rule: AppRule::Normal,
                rules_fg: null_mut(),
                rules_exe: None,
                remembered: HashMap::new(),
                history: VecDeque::new(),
                history_map: history::Writer::create(),
                fg_known: false,
                icon_state: None,
                hkl: Vec::new(),
                unknown_hkl: null_mut(),
                icon: null_mut(),
                taskbar_created,
                pending: None,
                slow_apps: HashSet::new(),
                unreliable_apps: HashSet::new(),
                suspended_until: 0,
                files_stamp: sys::files_stamp(),
                password: Password::No,
                password_seq: 0,
                password_root: null_mut(),
                password_tx: Some(password::spawn_worker(hwnd)),
            })
        });
        with_app(|app| update_tray(app, true, true));
        set_foreground(GetForegroundWindow());
        init_layouts();
        // Foreground changes are reported here, out of context, on this thread's message loop.
        let fg_hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            null_mut(),
            Some(foreground_event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if fg_hook.is_null() {
            log::warn!("SetWinEventHook failed: {}", GetLastError());
        }
        // Focus changes inside windows: to notice password fields.
        let focus_hook = SetWinEventHook(
            EVENT_OBJECT_FOCUS,
            EVENT_OBJECT_FOCUS,
            null_mut(),
            Some(focus_event),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        );
        if focus_hook.is_null() {
            log::warn!("SetWinEventHook(focus) failed: {}", GetLastError());
        }
        install_hooks();
        if KB_HOOK.load(Ordering::SeqCst).is_null() {
            log::error!("SetWindowsHookExW(WH_KEYBOARD_LL) failed: {}", GetLastError());
            message_box(hwnd, t("Could not install the keyboard hook."), MB_ICONERROR);
        } else {
            log::info!("keyboard hook installed");
        }
        SetTimer(hwnd, TIMER_ICON, 250, None);
        SetTimer(hwnd, TIMER_REHOOK, 60_000, None);
        SetTimer(hwnd, TIMER_WATCH, WATCH_MS, None);

        if !with_app(|a| a.engine.settings.setup_done).unwrap_or(true) {
            // First run: show the guide (it marks itself done when closed).
            sys::spawn_self(&["--setup"]);
        }
        if !warnings.is_empty() {
            log::warn!("settings.ini: {}", warnings.join("; "));
            message_box(hwnd, &i18n::f("Errors in settings.ini:\n{}", &[&warnings.join("\n")]), MB_ICONWARNING);
        }

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        remove_hooks();
        if !fg_hook.is_null() {
            UnhookWinEvent(fg_hook);
        }
        if !focus_hook.is_null() {
            UnhookWinEvent(focus_hook);
        }
        log::info!("exit");
        with_app(|app| {
            if !app.icon.is_null() {
                DestroyIcon(app.icon);
            }
        });
        CloseHandle(mutex);
    }
}
