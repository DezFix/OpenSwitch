//! Windows tray app: low-level keyboard/mouse hooks, input injection, verified layout
//! switching, tray icon and menu.

use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};

use openswitch::detect::Detector;
use openswitch::engine::{Action, Engine};
use openswitch::layout::{flip_text, guess_text_lang, Lang};

use crate::sys::{self, w, APP_NAME, OUR_MARK, WINDOW_CLASS, WM_RELOAD};

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::System::DataExchange::*;
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Memory::*;
use windows_sys::Win32::System::SystemInformation::GetTickCount64;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

const WM_TRAY: u32 = WM_APP + 1;
const WM_TOGGLE_AUTO: u32 = WM_APP + 2;
const WM_SWITCH_LAYOUT: u32 = WM_APP + 4;
const TIMER_ICON: usize = 1;
const TIMER_REHOOK: usize = 2;
const TIMER_VERIFY: usize = 3;
const VERIFY_MS: u32 = 300;
const CF_UNICODETEXT_ID: u32 = 13;

const ID_AUTO: usize = 100;
const ID_STARTUP: usize = 101;
const ID_SETTINGS: usize = 102;
const ID_RELOAD: usize = 104;
const ID_FOLDER: usize = 105;
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

struct App {
    engine: Engine,
    hwnd: HWND,
    last_fg: HWND,
    fg_exe: Option<String>,
    fg_excluded: bool,
    icon_state: Option<(Option<Lang>, bool)>,
    icon: HICON,
    taskbar_created: u32,
    pending: Option<Pending>,
    /// Programs that ignore WM_INPUTLANGCHANGEREQUEST: use the system shortcut there right away.
    slow_apps: HashSet<String>,
    /// Programs whose layout cannot be read back: never press the shortcut there.
    unreliable_apps: HashSet<String>,
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

// ---------------------------------------------------------------- input injection

fn key_input(vk: u16, up: bool) -> INPUT {
    let mut i: INPUT = unsafe { std::mem::zeroed() };
    i.r#type = INPUT_KEYBOARD;
    i.Anonymous.ki = KEYBDINPUT {
        wVk: vk,
        wScan: 0,
        dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
        time: 0,
        dwExtraInfo: OUR_MARK,
    };
    i
}

fn unicode_input(unit: u16, up: bool) -> INPUT {
    let mut i: INPUT = unsafe { std::mem::zeroed() };
    i.r#type = INPUT_KEYBOARD;
    i.Anonymous.ki = KEYBDINPUT {
        wVk: 0,
        wScan: unit,
        dwFlags: KEYEVENTF_UNICODE | if up { KEYEVENTF_KEYUP } else { 0 },
        time: 0,
        dwExtraInfo: OUR_MARK,
    };
    i
}

fn push_tap(v: &mut Vec<INPUT>, vk: u16) {
    v.push(key_input(vk, false));
    v.push(key_input(vk, true));
}

fn push_text(v: &mut Vec<INPUT>, text: &str) {
    for c in text.chars() {
        match c {
            '\r' => {}
            '\n' => push_tap(v, VK_RETURN),
            _ => {
                let mut buf = [0u16; 2];
                for &u in c.encode_utf16(&mut buf).iter() {
                    v.push(unicode_input(u, false));
                    v.push(unicode_input(u, true));
                }
            }
        }
    }
}

fn send(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    unsafe {
        SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

// ---------------------------------------------------------------- verified layout switching

/// Switch the foreground window to `lang` and make sure it really happened:
/// first ask the window politely (WM_INPUTLANGCHANGEREQUEST), check after a moment and,
/// if the program ignored the request, press the system layout shortcut (Alt+Shift /
/// Ctrl+Shift / Win+Space). Programs that needed the shortcut get it right away next time.
fn switch_layout(lang: Lang) {
    let fg = unsafe { GetForegroundWindow() };
    if fg.is_null() || sys::current_lang(fg) == Some(lang) {
        return;
    }
    if !sys::installed_layouts().iter().any(|h| sys::lang_of_hkl(*h) == Some(lang)) {
        with_app(|a| a.engine.cancel_forced());
        return;
    }
    let mode = with_app(|a| match &a.fg_exe {
        Some(e) if a.unreliable_apps.contains(e) => 2,
        Some(e) if a.slow_apps.contains(e) => 1,
        _ => 0,
    })
    .unwrap_or(0);
    let before = sys::current_lang(fg);
    match mode {
        2 => {
            // We cannot read this program's layout reliably: just ask and hope.
            sys::request_layout(lang);
            return;
        }
        1 => sys::press_system_toggle(),
        _ => sys::request_layout(lang),
    }
    let hwnd = with_app(|a| {
        a.pending = Some(Pending { lang, fg, presses: mode, before });
        a.hwnd
    });
    if let Some(h) = hwnd {
        unsafe { SetTimer(h, TIMER_VERIFY, VERIFY_MS, None) };
    }
}

fn verify_layout(hwnd: HWND) {
    let n_layouts = sys::installed_layouts().len().clamp(2, 8) as u32;
    // Number of system-shortcut presses to do now.
    let presses = with_app(|a| {
        let Some(p) = &mut a.pending else { return 0 };
        let fg = unsafe { GetForegroundWindow() };
        let now = sys::current_lang(fg);
        if fg != p.fg {
            a.pending = None;
            return 0;
        }
        if now == Some(p.lang) {
            if p.presses > 0 {
                if let Some(e) = &a.fg_exe {
                    a.slow_apps.insert(e.clone());
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

// ---------------------------------------------------------------- hooks

fn is_down(vk: u16) -> bool {
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

unsafe extern "system" fn kb_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
        // Ignore everything injected (our own input, password managers' auto-type, remote tools...)
        if kb.flags & LLKHF_INJECTED == 0 && kb.vkCode < 256 {
            let msg = wparam as u32;
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            if handle_key(kb.vkCode as u8, down) {
                return 1;
            }
        }
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

unsafe extern "system" fn mouse_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 {
        let m = wparam as u32;
        if m == WM_LBUTTONDOWN || m == WM_RBUTTONDOWN || m == WM_MBUTTONDOWN || m == WM_XBUTTONDOWN {
            with_app(|app| app.engine.reset());
        }
    }
    CallNextHookEx(null_mut(), code, wparam, lparam)
}

fn is_modifier(vk: u8) -> bool {
    matches!(vk, 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
}

/// Returns true if the key must be swallowed.
fn handle_key(vk: u8, down: bool) -> bool {
    let result = with_app(|app| {
        let fg = unsafe { GetForegroundWindow() };
        if fg != app.last_fg {
            app.last_fg = fg;
            app.engine.reset();
            app.fg_exe = sys::exe_name(fg);
            app.fg_excluded = app.fg_exe.as_ref().is_some_and(|n| app.engine.settings.exclude.contains(n));
        }
        let layout = sys::current_lang(fg);
        if down && !is_modifier(vk) {
            app.engine.sync_modifiers(
                is_down(VK_LSHIFT),
                is_down(VK_RSHIFT),
                [is_down(VK_LCONTROL), is_down(VK_RCONTROL)],
                [is_down(VK_LMENU), is_down(VK_RMENU)],
                [is_down(VK_LWIN), is_down(VK_RWIN)],
            );
        }
        let now = unsafe { GetTickCount64() };
        (app.engine.on_key(vk, down, now, layout, !app.fg_excluded), app.hwnd)
    });
    let Some((out, hwnd)) = result else { return false };
    perform(out.actions, hwnd);
    out.swallow
}

fn perform(actions: Vec<Action>, hwnd: HWND) {
    let mut inputs = Vec::new();
    for a in actions {
        match a {
            Action::Backspace(n) => (0..n).for_each(|_| push_tap(&mut inputs, VK_BACK)),
            Action::Type(t) => push_text(&mut inputs, &t),
            Action::Tap(vk) => push_tap(&mut inputs, vk as u16),
            Action::SwitchLayout(l) => {
                send(&inputs);
                inputs.clear();
                switch_layout(l);
            }
            Action::ConvertSelection => {
                if !SELECTION_BUSY.swap(true, Ordering::SeqCst) {
                    std::thread::spawn(|| {
                        convert_selection();
                        SELECTION_BUSY.store(false, Ordering::SeqCst);
                    });
                }
            }
            Action::ToggleAuto => unsafe {
                PostMessageW(hwnd, WM_TOGGLE_AUTO, 0, 0);
            },
            Action::AddException(word) => sys::append_exception(&word),
        }
    }
    send(&inputs);
}

unsafe fn install_hooks() {
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

unsafe fn remove_hooks() {
    for h in [&KB_HOOK, &MOUSE_HOOK] {
        let old = h.swap(null_mut(), Ordering::SeqCst);
        if !old.is_null() {
            UnhookWindowsHookEx(old);
        }
    }
}

// ---------------------------------------------------------------- clipboard / selection

unsafe fn clipboard_text() -> Option<String> {
    for _ in 0..10 {
        if OpenClipboard(null_mut()) != 0 {
            let mut res = None;
            let h = GetClipboardData(CF_UNICODETEXT_ID);
            if !h.is_null() {
                let p = GlobalLock(h) as *const u16;
                if !p.is_null() {
                    let mut len = 0;
                    while *p.add(len) != 0 {
                        len += 1;
                    }
                    res = Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, len)));
                    GlobalUnlock(h);
                }
            }
            CloseClipboard();
            return res;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    None
}

unsafe fn set_clipboard_text(text: &str) {
    let data: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    for _ in 0..10 {
        if OpenClipboard(null_mut()) != 0 {
            EmptyClipboard();
            let h = GlobalAlloc(GMEM_MOVEABLE, data.len() * 2);
            if !h.is_null() {
                let p = GlobalLock(h) as *mut u16;
                if !p.is_null() {
                    std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
                    GlobalUnlock(h);
                    if SetClipboardData(CF_UNICODETEXT_ID, h).is_null() {
                        GlobalFree(h);
                    }
                } else {
                    GlobalFree(h);
                }
            }
            CloseClipboard();
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// Runs on a worker thread: copy selection, convert, type it back, re-select it.
fn convert_selection() {
    unsafe {
        // Wait until the user releases the hotkey modifiers (up to 1 s) so Ctrl+C is clean.
        for _ in 0..50 {
            if ![VK_SHIFT, VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN].iter().any(|&k| is_down(k)) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let mut inputs = Vec::new();
        for k in [VK_LSHIFT, VK_RSHIFT, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN] {
            if is_down(k) {
                inputs.push(key_input(k, true));
            }
        }
        let saved = clipboard_text();
        let seq = GetClipboardSequenceNumber();
        inputs.push(key_input(VK_CONTROL, false));
        push_tap(&mut inputs, b'C' as u16);
        inputs.push(key_input(VK_CONTROL, true));
        send(&inputs);

        let mut changed = false;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            if GetClipboardSequenceNumber() != seq {
                changed = true;
                break;
            }
        }
        if !changed {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
        let Some(text) = clipboard_text() else { return };
        // Some editors (VS Code...) copy the whole line when nothing is selected.
        let single_line_with_newline = text.ends_with('\n') && text.trim_end().lines().count() <= 1;
        if text.is_empty() || single_line_with_newline || text.chars().count() > 5000 {
            if let Some(s) = &saved {
                set_clipboard_text(s);
            }
            return;
        }
        let target = guess_text_lang(&text).other();
        let converted = flip_text(&text);
        let mut inputs = Vec::new();
        push_text(&mut inputs, &converted);
        let len = converted.chars().filter(|c| *c != '\r').count();
        if len <= 500 {
            inputs.push(key_input(VK_SHIFT, false));
            (0..len).for_each(|_| push_tap(&mut inputs, VK_LEFT));
            inputs.push(key_input(VK_SHIFT, true));
        }
        send(&inputs);
        let main = MAIN_HWND.load(Ordering::SeqCst) as HWND;
        PostMessageW(main, WM_SWITCH_LAYOUT, target as usize, 0);
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Some(s) = &saved {
            set_clipboard_text(s);
        }
    }
}

// ---------------------------------------------------------------- tray icon

unsafe fn make_icon(text: &str, bg: u32) -> HICON {
    let size = GetSystemMetrics(SM_CXSMICON).clamp(16, 64);
    let screen = GetDC(null_mut());
    let dc = CreateCompatibleDC(screen);
    let mut bmi: BITMAPINFO = std::mem::zeroed();
    bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    bmi.bmiHeader.biWidth = size;
    bmi.bmiHeader.biHeight = -size;
    bmi.bmiHeader.biPlanes = 1;
    bmi.bmiHeader.biBitCount = 32;
    bmi.bmiHeader.biCompression = BI_RGB;
    let mut bits: *mut c_void = null_mut();
    let color = CreateDIBSection(dc, &bmi, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
    let old = SelectObject(dc, color);

    let brush = CreateSolidBrush(bg);
    let rc = RECT { left: 0, top: 0, right: size, bottom: size };
    FillRect(dc, &rc, brush);
    DeleteObject(brush);

    let font = CreateFontW(
        -(size * 11 / 16),
        0,
        0,
        0,
        FW_BOLD as i32,
        0,
        0,
        0,
        DEFAULT_CHARSET as u32,
        OUT_DEFAULT_PRECIS as u32,
        CLIP_DEFAULT_PRECIS as u32,
        ANTIALIASED_QUALITY as u32,
        (DEFAULT_PITCH | FF_SWISS) as u32,
        w("Segoe UI").as_ptr(),
    );
    let old_font = SelectObject(dc, font);
    SetBkMode(dc, TRANSPARENT as i32);
    SetTextColor(dc, 0x00FF_FFFF);
    let mut rc2 = rc;
    let t: Vec<u16> = text.encode_utf16().collect();
    DrawTextW(dc, t.as_ptr(), t.len() as i32, &mut rc2, DT_CENTER | DT_VCENTER | DT_SINGLELINE);
    SelectObject(dc, old_font);
    DeleteObject(font);
    GdiFlush();

    // Rounded corners via per-pixel alpha.
    let px = std::slice::from_raw_parts_mut(bits as *mut u32, (size * size) as usize);
    let r = (size as f32) * 0.22;
    for y in 0..size {
        for x in 0..size {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let cx = fx.clamp(r, size as f32 - r);
            let cy = fy.clamp(r, size as f32 - r);
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            let a = (r - d + 0.5).clamp(0.0, 1.0);
            let p = &mut px[(y * size + x) as usize];
            let rgb = *p & 0x00FF_FFFF;
            let scale = |c: u32| ((c as f32) * a) as u32;
            let (rr, gg, bb) = (scale((rgb >> 16) & 0xFF), scale((rgb >> 8) & 0xFF), scale(rgb & 0xFF));
            *p = (((a * 255.0) as u32) << 24) | (rr << 16) | (gg << 8) | bb;
        }
    }
    SelectObject(dc, old);

    let mask_bits = vec![0u8; (((size + 15) / 16) * 2 * size) as usize];
    let mask = CreateBitmap(size, size, 1, 1, mask_bits.as_ptr() as *const c_void);
    let info = ICONINFO { fIcon: 1, xHotspot: 0, yHotspot: 0, hbmMask: mask, hbmColor: color };
    let icon = CreateIconIndirect(&info);
    DeleteObject(mask);
    DeleteObject(color);
    DeleteDC(dc);
    ReleaseDC(null_mut(), screen);
    icon
}

fn copy_tip(dst: &mut [u16; 128], s: &str) {
    let v: Vec<u16> = s.encode_utf16().take(127).collect();
    dst[..v.len()].copy_from_slice(&v);
    dst[v.len()] = 0;
}

unsafe fn update_tray(app: &mut App, force: bool, add: bool) {
    let fg = GetForegroundWindow();
    // When our own menu/window is active keep showing the previous layout.
    let lang = if fg == app.hwnd { app.icon_state.and_then(|s| s.0) } else { sys::current_lang(fg) };
    let auto = app.engine.settings.auto_switch;
    let state = (lang, auto);
    if !force && app.icon_state == Some(state) {
        return;
    }
    app.icon_state = Some(state);
    let text = lang.map(|l| l.code()).unwrap_or("··");
    // colours are 0x00BBGGRR
    let bg = match (auto, lang) {
        (false, _) => 0x0080_8080,
        (true, Some(Lang::Ru)) => 0x0035_3AD6,
        (true, _) => 0x00C8_7A1E,
    };
    let icon = make_icon(text, bg);
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = app.hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    nid.uCallbackMessage = WM_TRAY;
    nid.hIcon = icon;
    copy_tip(
        &mut nid.szTip,
        if auto { "OpenSwitch — автопереключение включено" } else { "OpenSwitch — автопереключение выключено" },
    );
    Shell_NotifyIconW(if add { NIM_ADD } else { NIM_MODIFY }, &nid);
    if !app.icon.is_null() {
        DestroyIcon(app.icon);
    }
    app.icon = icon;
}

unsafe fn remove_tray(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    Shell_NotifyIconW(NIM_DELETE, &nid);
}

fn set_auto(on: bool) {
    with_app(|app| {
        app.engine.settings.auto_switch = on;
        unsafe { update_tray(app, true, false) };
    });
    sys::save_setting("auto_switch", if on { "true" } else { "false" });
}

fn reload() -> Vec<String> {
    let (settings, warnings) = sys::load_settings();
    let exceptions = sys::load_exceptions();
    with_app(|app| {
        app.engine.detector.sensitivity = settings.sensitivity;
        app.engine.detector.single_letters = settings.single_letters;
        app.engine.detector.never_switch = exceptions;
        app.engine.settings = settings;
        app.last_fg = null_mut();
        unsafe { update_tray(app, true, false) };
    });
    warnings
}

fn message_box(hwnd: HWND, text: &str, flags: MESSAGEBOX_STYLE) {
    unsafe {
        MessageBoxW(hwnd, w(text).as_ptr(), w(APP_NAME).as_ptr(), flags);
    }
}

unsafe fn show_menu(hwnd: HWND) {
    let auto = with_app(|a| a.engine.settings.auto_switch).unwrap_or(true);
    let m = CreatePopupMenu();
    let item = |id: usize, text: &str, checked: bool| {
        AppendMenuW(m, MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED }, id, w(text).as_ptr());
    };
    item(ID_SETTINGS, "Настройки…", false);
    SetMenuDefaultItem(m, ID_SETTINGS as u32, 0);
    AppendMenuW(m, MF_SEPARATOR, 0, null());
    item(ID_AUTO, "Автопереключение раскладки", auto);
    item(ID_STARTUP, "Запускать вместе с Windows", sys::startup_enabled());
    AppendMenuW(m, MF_SEPARATOR, 0, null());
    item(ID_FOLDER, "Открыть папку настроек", false);
    item(ID_RELOAD, "Перечитать настройки", false);
    item(ID_ABOUT, "О программе", false);
    AppendMenuW(m, MF_SEPARATOR, 0, null());
    item(ID_EXIT, "Выход", false);

    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, null());
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(m);

    match cmd as usize {
        ID_SETTINGS => sys::spawn_self(&["--settings"]),
        ID_AUTO => set_auto(!auto),
        ID_STARTUP => sys::set_startup(!sys::startup_enabled()),
        ID_FOLDER => sys::open_path(&sys::data_dir()),
        ID_RELOAD => {
            let warn = reload();
            if !warn.is_empty() {
                message_box(hwnd, &format!("Ошибки в settings.ini:\n{}", warn.join("\n")), MB_ICONWARNING);
            }
        }
        ID_ABOUT => {
            let s = with_app(|a| a.engine.settings.clone()).unwrap_or_default();
            message_box(
                hwnd,
                &format!(
                    "OpenSwitch {} — свободный переключатель раскладки EN/RU.\n\
                     Лицензия GNU GPL v3.\n\n\
                     {} — исправить последнее слово (ещё раз — вернуть)\n\
                     {}\
                     {} — исправить выделенный текст\n\
                     {} — вкл/выкл автопереключение\n\n\
                     Клик по значку — вкл/выкл автопереключение.",
                    env!("CARGO_PKG_VERSION"),
                    s.hotkey_convert_word,
                    if s.double_shift { "Shift Shift (двойное нажатие) — то же самое\n" } else { "" },
                    s.hotkey_convert_selection,
                    s.hotkey_toggle_auto,
                ),
                MB_ICONINFORMATION,
            );
        }
        ID_EXIT => {
            DestroyWindow(hwnd);
        }
        _ => {}
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
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
        WM_SWITCH_LAYOUT => {
            switch_layout(if wparam == Lang::Ru as usize { Lang::Ru } else { Lang::En });
            0
        }
        WM_TIMER => {
            match wparam {
                TIMER_ICON => {
                    with_app(|app| update_tray(app, false, false));
                }
                TIMER_VERIFY => verify_layout(hwnd),
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

pub fn run() {
    unsafe {
        let mutex = CreateMutexW(null(), 1, w("Local\\OpenSwitch.SingleInstance").as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            // Already running: open the settings window instead of a second tray icon.
            sys::spawn_self(&["--settings"]);
            return;
        }
        SetProcessDPIAware();
        sys::migrate_startup();

        let (settings, warnings) = sys::load_settings();
        let mut detector = Detector::new();
        detector.never_switch = sys::load_exceptions();
        detector.sensitivity = settings.sensitivity;
        detector.single_letters = settings.single_letters;
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
            message_box(null_mut(), "Не удалось создать окно.", MB_ICONERROR);
            return;
        }
        MAIN_HWND.store(hwnd as usize, Ordering::SeqCst);
        let taskbar_created = RegisterWindowMessageW(w("TaskbarCreated").as_ptr());

        APP.with(|a| {
            *a.borrow_mut() = Some(App {
                engine,
                hwnd,
                last_fg: null_mut(),
                fg_exe: None,
                fg_excluded: false,
                icon_state: None,
                icon: null_mut(),
                taskbar_created,
                pending: None,
                slow_apps: HashSet::new(),
                unreliable_apps: HashSet::new(),
            })
        });
        with_app(|app| update_tray(app, true, true));
        install_hooks();
        if KB_HOOK.load(Ordering::SeqCst).is_null() {
            message_box(hwnd, "Не удалось установить перехватчик клавиатуры.", MB_ICONERROR);
        }
        SetTimer(hwnd, TIMER_ICON, 250, None);
        SetTimer(hwnd, TIMER_REHOOK, 60_000, None);

        if !warnings.is_empty() {
            message_box(hwnd, &format!("Ошибки в settings.ini:\n{}", warnings.join("\n")), MB_ICONWARNING);
        }

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        remove_hooks();
        with_app(|app| {
            if !app.icon.is_null() {
                DestroyIcon(app.icon);
            }
        });
        CloseHandle(mutex);
    }
}
