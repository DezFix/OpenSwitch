// SPDX-License-Identifier: GPL-3.0-or-later
//! Windows helpers shared by the tray app and the settings window:
//! files in %APPDATA%, autostart, process names, layout switching.

use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};

use openswitch::config::{parse_always, parse_exceptions, Settings, DEFAULT_FILE};
use openswitch::i18n;
use openswitch::layout::{ukrainian_map, KeyMap, Lang, Language, Layout, Layouts};

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
/// Message the settings window posts while it records a hotkey: wparam 1 = let all keys
/// through for a few seconds (the window keeps re-sending it), 0 = back to normal.
pub const WM_SUSPEND: u32 = WM_APP + 7;
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
pub fn always_path() -> PathBuf {
    data_dir().join("always.txt")
}

const ALWAYS_HEADER: &str = "# Слова, которые OpenSwitch всегда исправляет (выучены из ваших ручных исправлений).\r\n\
# По одному на строку, в той раскладке, в которой вы их набираете.\r\n";

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
        Err(e) => (Settings::default(), vec![i18n::f("could not read {}: {}", &[&p.display(), &e])]),
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

/// Words learned from manual fixes (always.txt; missing file = empty list), with the code of
/// the layout to convert to when known.
pub fn load_always() -> std::collections::HashMap<String, Option<String>> {
    std::fs::read_to_string(always_path()).map(|t| parse_always(&t)).unwrap_or_default()
}

/// always.txt as sorted lines `word` / `word CODE` (for the settings window).
pub fn always_lines() -> Vec<String> {
    let mut v: Vec<String> = load_always()
        .into_iter()
        .map(|(w, code)| match code {
            Some(c) => format!("{w} {c}"),
            None => w,
        })
        .collect();
    v.sort();
    v
}

pub fn save_always(words: &[String]) -> std::io::Result<()> {
    ensure_data_dir();
    let mut t = String::from(ALWAYS_HEADER);
    for word in words {
        t.push_str(word);
        t.push_str("\r\n");
    }
    std::fs::write(always_path(), t)
}

/// Add (with the layout code) or remove (`code` None) one learned word.
pub fn update_always(word: &str, code: Option<&str>) {
    let mut words: Vec<String> =
        always_lines().into_iter().filter(|l| l.split_whitespace().next() != Some(word)).collect();
    if let Some(code) = code {
        words.push(format!("{word} {code}"));
    }
    words.sort();
    if let Err(e) = save_always(&words) {
        log::error!("could not write always.txt: {e}");
    }
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
        ShellExecuteW(
            null_mut(),
            w("open").as_ptr(),
            w(&p.display().to_string()).as_ptr(),
            null(),
            null(),
            SW_SHOWNORMAL,
        );
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

/// Ask the tray app to ignore keys (hotkeys included) while a hotkey is being recorded.
pub fn suspend_hotkeys(on: bool) {
    let h = tray_window();
    if !h.is_null() {
        unsafe { PostMessageW(h, WM_SUSPEND, on as usize, 0) };
    }
}

/// Modification times of the settings and exception files, to notice edits made by hand.
pub fn files_stamp() -> [Option<std::time::SystemTime>; 3] {
    let m = |p: PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    [m(settings_path()), m(exceptions_path()), m(always_path())]
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

/// Primary language id of a keyboard layout handle (low 10 bits of the language word).
fn primary_lang(hkl: HKL) -> usize {
    (hkl as usize) & 0x3FF
}

/// The dictionary language of a layout. Belarusian uses the Russian dictionary for now;
/// layouts of other languages do not take part in switching.
pub fn language_of_hkl(hkl: HKL) -> Option<Language> {
    match primary_lang(hkl) {
        0x09 => Some(Language::En),
        0x19 | 0x23 => Some(Language::Ru),
        0x22 => Some(Language::Uk),
        _ => None,
    }
}

/// The layouts that take part in switching, in Windows' order, with their handles.
/// A Ukrainian layout also counts as Russian when there is no Russian layout, so Russian
/// typed in it is not "fixed". Layouts in `disabled` (codes) are never switched to.
pub fn build_layouts(installed: &[HKL], disabled: &std::collections::HashSet<String>) -> (Layouts, Vec<HKL>) {
    let has_ru = installed.iter().any(|h| primary_lang(*h) == 0x19);
    let mut list = Vec::new();
    let mut hkls = Vec::new();
    for &hkl in installed {
        let Some(lang) = language_of_hkl(hkl) else { continue };
        let mut languages = vec![lang];
        if lang == Language::Uk && !has_ru {
            languages.push(Language::Ru);
        }
        let code = layout_code(hkl);
        let mut layout = Layout::new(read_keymap(hkl, lang), languages, code);
        layout.target = !disabled.contains(code);
        list.push(layout);
        hkls.push(hkl);
    }
    (Layouts::new(list), hkls)
}

/// Codes of the installed layouts that can take part in switching (for the settings window).
pub fn layout_codes() -> Vec<&'static str> {
    let mut v: Vec<&'static str> =
        installed_layouts().into_iter().filter(|h| language_of_hkl(*h).is_some()).map(layout_code).collect();
    v.dedup();
    v
}

/// Short code for the tray icon.
pub fn layout_code(hkl: HKL) -> &'static str {
    match primary_lang(hkl) {
        0x09 => "EN",
        0x19 => "RU",
        0x22 => "UA",
        0x23 => "BY",
        _ => "··",
    }
}

/// Virtual keys whose characters we read from a layout: letters and the punctuation keys that
/// carry letters in Cyrillic layouts (OEM 1-8 and the extra ISO key).
const MAPPED_KEYS: &[u8] = &[
    b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', b'I', b'J', b'K', b'L', b'M', b'N', b'O', b'P', b'Q', b'R', b'S',
    b'T', b'U', b'V', b'W', b'X', b'Y', b'Z', 0xBA, 0xBB, 0xBC, 0xBD, 0xBE, 0xBF, 0xC0, 0xDB, 0xDC, 0xDD, 0xDE, 0xDF,
    0xE2,
];

/// Character that `vk` types in layout `hkl` with the given Shift / CapsLock state.
fn key_char(vk: u8, hkl: HKL, shift: bool, caps: bool) -> Option<char> {
    let mut state = [0u8; 256];
    if shift {
        state[VK_SHIFT as usize] = 0x80;
        state[VK_LSHIFT as usize] = 0x80;
    }
    if caps {
        state[VK_CAPITAL as usize] = 0x01;
    }
    let mut buf = [0u16; 8];
    unsafe {
        let sc = MapVirtualKeyExW(vk as u32, MAPVK_VK_TO_VSC, hkl);
        // Flag 0x4: do not change the keyboard state (no dead-key side effects), Windows 10 1607+.
        let n = ToUnicodeEx(vk as u32, sc, state.as_ptr(), buf.as_mut_ptr(), buf.len() as i32, 0x4, hkl);
        if n < 0 {
            // Dead key. On older Windows the flag is ignored: flush the pending dead key.
            let space = MapVirtualKeyExW(VK_SPACE as u32, MAPVK_VK_TO_VSC, hkl);
            ToUnicodeEx(VK_SPACE as u32, space, [0u8; 256].as_ptr(), buf.as_mut_ptr(), buf.len() as i32, 0x4, hkl);
            return None;
        }
        if n != 1 {
            return None;
        }
    }
    char::from_u32(buf[0] as u32).filter(|c| !c.is_control())
}

/// Read the key -> character map of an installed layout, so that customised layouts
/// (e.g. Ukrainian with Russian letters added) convert correctly.
/// Falls back to the built-in table if the layout looks broken.
pub fn read_keymap(hkl: HKL, lang: Language) -> KeyMap {
    let mut m = KeyMap::empty();
    for &vk in MAPPED_KEYS {
        let normal = key_char(vk, hkl, false, false);
        let shifted = key_char(vk, hkl, true, false);
        let with_caps = key_char(vk, hkl, false, true);
        let caps_like_shift = with_caps.is_some() && with_caps == shifted && shifted != normal;
        m.set(vk, normal, shifted, caps_like_shift);
    }
    if m.letter_count() < 20 {
        log::warn!("layout {:#x}: only {} letter keys, using the built-in table", hkl as usize, m.letter_count());
        return match lang {
            Language::En => KeyMap::builtin(Lang::En),
            Language::Ru => KeyMap::builtin(Lang::Ru),
            Language::Uk => ukrainian_map(),
        };
    }
    m
}

/// Keyboard layout of the focused thread of `fg`.
pub fn current_hkl(fg: HWND) -> HKL {
    if fg.is_null() {
        return null_mut();
    }
    let (_, tid) = focus_target(fg);
    unsafe { GetKeyboardLayout(tid) }
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

/// Ask the foreground window to switch to layout `hkl` (fast path, works in most programs).
pub fn request_layout(hkl: HKL) {
    if hkl.is_null() {
        return;
    }
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
    i.Anonymous.ki =
        KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: OUR_MARK };
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
                if RegQueryValueExW(key, w(name).as_ptr(), null(), &mut ty, buf.as_mut_ptr() as *mut u8, &mut size) == 0
                {
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
