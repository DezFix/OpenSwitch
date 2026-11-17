//! Windows helpers shared by the tray app and the settings window:
//! files in %APPDATA%, autostart, process names, layout switching.

use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use openswitch::config::{parse_exceptions, Settings, DEFAULT_FILE};
use openswitch::layout::Lang;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::System::Registry::*;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const APP_NAME: &str = "OpenSwitch";
pub const OLD_APP_NAME: &str = "OpenPunto";
pub const WINDOW_CLASS: &str = "OpenSwitchWindow";
/// Message the settings window posts to the tray app after saving.
pub const WM_RELOAD: u32 = WM_APP + 3;
/// Marker in dwExtraInfo of every event we inject.
pub const OUR_MARK: usize = 0x0F_E7_C0_DE;

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";

pub fn w(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---------------------------------------------------------------- files

fn appdata() -> PathBuf {
    std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}
pub fn data_dir() -> PathBuf {
    appdata().join(APP_NAME)
}
pub fn settings_path() -> PathBuf {
    data_dir().join("settings.ini")
}
pub fn exceptions_path() -> PathBuf {
    data_dir().join("exceptions.txt")
}

const EXCEPTIONS_HEADER: &str = "# Слова, которые OpenSwitch никогда не будет исправлять автоматически.\r\n\
# По одному на строку, в той раскладке, в которой вы их набираете.\r\n\
# Слово добавляется сюда само, если вы отменили автоисправление клавишей Pause.\r\n";

/// Create the data folder; on first run take over settings of the old OpenPunto build.
pub fn ensure_data_dir() {
    let dir = data_dir();
    if !dir.exists() {
        let old = appdata().join(OLD_APP_NAME);
        let _ = std::fs::create_dir_all(&dir);
        for f in ["settings.ini", "exceptions.txt"] {
            let src = old.join(f);
            if src.exists() {
                if let Ok(t) = std::fs::read_to_string(&src) {
                    let _ = std::fs::write(dir.join(f), t.replace(OLD_APP_NAME, APP_NAME));
                }
            }
        }
    }
    if !settings_path().exists() {
        let _ = std::fs::write(settings_path(), DEFAULT_FILE.replace('\n', "\r\n"));
    }
    if !exceptions_path().exists() {
        let _ = std::fs::write(exceptions_path(), EXCEPTIONS_HEADER);
    }
}

pub fn load_settings() -> (Settings, Vec<String>) {
    ensure_data_dir();
    let p = settings_path();
    match std::fs::read_to_string(&p) {
        Ok(t) => Settings::parse(&t),
        Err(e) => (Settings::default(), vec![format!("не удалось прочитать {}: {e}", p.display())]),
    }
}

pub fn save_settings(s: &Settings) -> std::io::Result<()> {
    ensure_data_dir();
    std::fs::write(settings_path(), s.to_text().replace('\n', "\r\n"))
}

pub fn load_exceptions() -> std::collections::HashSet<String> {
    ensure_data_dir();
    std::fs::read_to_string(exceptions_path()).map(|t| parse_exceptions(&t)).unwrap_or_default()
}

pub fn save_exceptions(words: &[String]) -> std::io::Result<()> {
    ensure_data_dir();
    let mut t = String::from(EXCEPTIONS_HEADER);
    for word in words {
        t.push_str(word);
        t.push_str("\r\n");
    }
    std::fs::write(exceptions_path(), t)
}

pub fn append_exception(word: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(exceptions_path()) {
        let _ = write!(f, "{word}\r\n");
    }
}

pub fn save_setting(key: &str, value: &str) {
    let p = settings_path();
    let text = std::fs::read_to_string(&p).unwrap_or_else(|_| DEFAULT_FILE.to_string());
    let new = Settings::set_in_text(&text.replace("\r\n", "\n"), key, value);
    let _ = std::fs::write(&p, new.replace('\n', "\r\n"));
}

pub fn open_path(p: &Path) {
    unsafe {
        ShellExecuteW(null_mut(), w("open").as_ptr(), w(&p.display().to_string()).as_ptr(), null(), null(), SW_SHOWNORMAL);
    }
}

/// Start another copy of our own exe with the given arguments.
pub fn spawn_self(args: &[&str]) {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).args(args).spawn();
    }
}

/// The hidden window of the running tray app, if any.
pub fn tray_window() -> HWND {
    unsafe { FindWindowW(w(WINDOW_CLASS).as_ptr(), null()) }
}

pub fn notify_reload() -> bool {
    let h = tray_window();
    !h.is_null() && unsafe { PostMessageW(h, WM_RELOAD, 0, 0) } != 0
}

// ---------------------------------------------------------------- autostart

fn reg_value_exists(name: &str) -> bool {
    unsafe {
        let mut key: HKEY = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, w(RUN_KEY).as_ptr(), 0, KEY_QUERY_VALUE, &mut key) != 0 {
            return false;
        }
        let r = RegQueryValueExW(key, w(name).as_ptr(), null(), null_mut(), null_mut(), null_mut());
        RegCloseKey(key);
        r == 0
    }
}

pub fn startup_enabled() -> bool {
    reg_value_exists(APP_NAME)
}

pub fn set_startup(on: bool) {
    unsafe {
        let mut key: HKEY = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, w(RUN_KEY).as_ptr(), 0, KEY_SET_VALUE, &mut key) != 0 {
            return;
        }
        if on {
            if let Ok(exe) = std::env::current_exe() {
                let val = w(&format!("\"{}\"", exe.display()));
                RegSetValueExW(key, w(APP_NAME).as_ptr(), 0, REG_SZ, val.as_ptr() as *const u8, (val.len() * 2) as u32);
            }
        } else {
            RegDeleteValueW(key, w(APP_NAME).as_ptr());
        }
        RegCloseKey(key);
    }
}

/// If the old OpenPunto build was set to start with Windows, move that to OpenSwitch.
pub fn migrate_startup() {
    if reg_value_exists(OLD_APP_NAME) {
        unsafe {
            let mut key: HKEY = null_mut();
            if RegOpenKeyExW(HKEY_CURRENT_USER, w(RUN_KEY).as_ptr(), 0, KEY_SET_VALUE, &mut key) == 0 {
                RegDeleteValueW(key, w(OLD_APP_NAME).as_ptr());
                RegCloseKey(key);
            }
        }
        set_startup(true);
    }
}

// ---------------------------------------------------------------- processes

pub fn exe_name(hwnd: HWND) -> Option<String> {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        path.rsplit(['\\', '/']).next().map(|s| s.to_lowercase())
    }
}

/// Executable names of programs that currently have visible windows.
pub fn window_exes() -> Vec<String> {
    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let out = &mut *(lparam as *mut Vec<String>);
        if IsWindowVisible(hwnd) != 0 && GetWindowTextLengthW(hwnd) > 0 && GetWindow(hwnd, GW_OWNER).is_null() {
            if let Some(n) = exe_name(hwnd) {
                if !out.contains(&n) {
                    out.push(n);
                }
            }
        }
        1
    }
    let mut v: Vec<String> = Vec::new();
    unsafe {
        EnumWindows(Some(cb), &mut v as *mut Vec<String> as LPARAM);
    }
    v.retain(|n| n != "explorer.exe" && n != "openswitch.exe" && n != "applicationframehost.exe");
    v.sort();
    v
}

// ---------------------------------------------------------------- keyboard layouts

pub fn lang_of_hkl(hkl: HKL) -> Option<Lang> {
    match (hkl as usize) & 0x3FF {
        0x09 => Some(Lang::En),
        0x19 => Some(Lang::Ru),
        _ => None,
    }
}

/// Focused window inside the foreground window and its thread id.
pub fn focus_target(fg: HWND) -> (HWND, u32) {
    unsafe {
        let tid = GetWindowThreadProcessId(fg, null_mut());
        let mut gti: GUITHREADINFO = std::mem::zeroed();
        gti.cbSize = std::mem::size_of::<GUITHREADINFO>() as u32;
        if GetGUIThreadInfo(tid, &mut gti) != 0 && !gti.hwndFocus.is_null() {
            let ftid = GetWindowThreadProcessId(gti.hwndFocus, null_mut());
            return (gti.hwndFocus, if ftid != 0 { ftid } else { tid });
        }
        (fg, tid)
    }
}

pub fn current_lang(fg: HWND) -> Option<Lang> {
    if fg.is_null() {
        return None;
    }
    let (_, tid) = focus_target(fg);
    lang_of_hkl(unsafe { GetKeyboardLayout(tid) })
}

pub fn installed_layouts() -> Vec<HKL> {
    unsafe {
        let n = GetKeyboardLayoutList(0, null_mut());
        if n <= 0 {
            return vec![];
        }
        let mut list: Vec<HKL> = vec![null_mut(); n as usize];
        let n = GetKeyboardLayoutList(n, list.as_mut_ptr());
        list.truncate(n.max(0) as usize);
        list
    }
}

/// Ask the foreground window to switch layout (fast path, works in most programs).
pub fn request_layout(lang: Lang) {
    let Some(hkl) = installed_layouts().into_iter().find(|h| lang_of_hkl(*h) == Some(lang)) else {
        return;
    };
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() {
            return;
        }
        let (focus, _) = focus_target(fg);
        PostMessageW(focus, WM_INPUTLANGCHANGEREQUEST, 0, hkl as isize);
        if focus != fg {
            PostMessageW(fg, WM_INPUTLANGCHANGEREQUEST, 0, hkl as isize);
        }
    }
}

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

/// Windows' own "next layout" shortcut from HKCU\Keyboard Layout\Toggle.
fn system_toggle_keys() -> Vec<u16> {
    let mut value = String::new();
    unsafe {
        let mut key: HKEY = null_mut();
        if RegOpenKeyExW(HKEY_CURRENT_USER, w("Keyboard Layout\\Toggle").as_ptr(), 0, KEY_QUERY_VALUE, &mut key) == 0 {
            for name in ["Language Hotkey", "Hotkey"] {
                let mut buf = [0u16; 16];
                let mut size = (buf.len() * 2) as u32;
                let mut ty = 0;
                if RegQueryValueExW(key, w(name).as_ptr(), null(), &mut ty, buf.as_mut_ptr() as *mut u8, &mut size) == 0 {
                    value = String::from_utf16_lossy(&buf[..(size as usize / 2)]).trim_end_matches('\0').to_string();
                    break;
                }
            }
            RegCloseKey(key);
        }
    }
    match value.as_str() {
        "2" => vec![VK_LCONTROL, VK_LSHIFT],
        "4" => vec![VK_OEM_3],
        "1" | "" => vec![VK_LMENU, VK_LSHIFT],
        _ => vec![VK_LWIN, VK_SPACE], // "3" = not assigned: Win+Space always works on Windows 8+
    }
}

/// Slow path: press the system layout shortcut once.
pub fn press_system_toggle() {
    let keys = system_toggle_keys();
    let mut inputs: Vec<INPUT> = keys.iter().map(|&k| key_input(k, false)).collect();
    inputs.extend(keys.iter().rev().map(|&k| key_input(k, true)));
    unsafe {
        SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}
