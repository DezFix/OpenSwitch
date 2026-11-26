// SPDX-License-Identifier: GPL-3.0-or-later
//! Tray icon, its menu and message boxes.

use super::*;

pub(super) unsafe fn make_icon(text: &str, bg: u32) -> HICON {
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

pub(super) fn copy_tip(dst: &mut [u16; 128], s: &str) {
    let v: Vec<u16> = s.encode_utf16().take(127).collect();
    dst[..v.len()].copy_from_slice(&v);
    dst[v.len()] = 0;
}

pub(super) unsafe fn update_tray(app: &mut App, force: bool, add: bool) {
    let fg = GetForegroundWindow();
    // When our own menu/window is active keep showing the previous layout.
    let auto = app.engine.settings.auto_switch;
    let (text, lang) = match app.icon_state {
        Some((code, lang, _)) if fg == app.hwnd => (code, lang),
        _ => {
            let hkl = sys::current_hkl(fg);
            (sys::layout_code(hkl), sys::language_of_hkl(hkl))
        }
    };
    let state = (text, lang, auto);
    if !force && app.icon_state == Some(state) {
        return;
    }
    app.icon_state = Some(state);
    // colours are 0x00BBGGRR
    let bg = match (auto, lang) {
        (false, _) => 0x0080_8080,
        (true, Some(l)) if !l.is_latin() => 0x0035_3AD6,
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
        if auto { t("OpenSwitch — auto switching is on") } else { t("OpenSwitch — auto switching is off") },
    );
    Shell_NotifyIconW(if add { NIM_ADD } else { NIM_MODIFY }, &nid);
    if !app.icon.is_null() {
        DestroyIcon(app.icon);
    }
    app.icon = icon;
}

/// A notification next to the tray icon.
pub(super) unsafe fn show_balloon(hwnd: HWND, title: &str, text: &str) {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = NIIF_INFO;
    for (dst, src) in [(&mut nid.szInfoTitle[..], title), (&mut nid.szInfo[..], text)] {
        let units: Vec<u16> = src.encode_utf16().take(dst.len() - 1).collect();
        dst[..units.len()].copy_from_slice(&units);
    }
    Shell_NotifyIconW(NIM_MODIFY, &nid);
}

pub(super) unsafe fn remove_tray(hwnd: HWND) {
    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    Shell_NotifyIconW(NIM_DELETE, &nid);
}

pub(super) fn set_auto(on: bool) {
    with_app(|app| {
        app.engine.settings.auto_switch = on;
        unsafe { update_tray(app, true, false) };
    });
    sys::save_setting("auto_switch", if on { "true" } else { "false" });
}

pub(super) fn reload() -> Vec<String> {
    let (settings, warnings) = sys::load_settings();
    log::info!("settings reloaded ({} warnings)", warnings.len());
    i18n::set(i18n::resolve(&settings.language));
    let exceptions = sys::load_exceptions();
    let always = sys::load_always();
    with_app(|app| {
        app.engine.detector.sensitivity = settings.sensitivity;
        app.engine.detector.single_letters = settings.single_letters;
        app.engine.detector.skip_code = settings.skip_code;
        app.engine.detector.never_switch = exceptions;
        app.engine.detector.always_switch = always;
        app.engine.settings = settings;
        app.last_fg = null_mut();
        app.files_stamp = sys::files_stamp();
        unsafe { update_tray(app, true, false) };
    });
    init_layouts();
    warnings
}

pub(super) fn message_box(hwnd: HWND, text: &str, flags: MESSAGEBOX_STYLE) {
    unsafe {
        MessageBoxW(hwnd, w(text).as_ptr(), w(APP_NAME).as_ptr(), flags);
    }
}

pub(super) unsafe fn show_menu(hwnd: HWND) {
    let auto = with_app(|a| a.engine.settings.auto_switch).unwrap_or(true);
    let m = CreatePopupMenu();
    let item = |id: usize, text: &str, checked: bool| {
        AppendMenuW(m, MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED }, id, w(text).as_ptr());
    };
    item(ID_SETTINGS, t("Settings…"), false);
    SetMenuDefaultItem(m, ID_SETTINGS as u32, 0);
    AppendMenuW(m, MF_SEPARATOR, 0, null());
    item(ID_AUTO, t("Auto layout switching"), auto);
    if with_app(|a| a.engine.settings.run_as_admin).unwrap_or(false) {
        // The administrator task starts it: a Run key would start a second copy at logon.
        AppendMenuW(m, MF_STRING | MF_CHECKED | MF_GRAYED, ID_STARTUP, w(t("Start with Windows")).as_ptr());
    } else {
        item(ID_STARTUP, t("Start with Windows"), sys::startup_enabled());
    }
    item(ID_ABOUT, t("About"), false);
    if let Some(tag) = newer_version() {
        item(ID_UPDATE, &i18n::f("Download OpenSwitch {}…", &[&tag.trim_start_matches('v')]), false);
    }
    AppendMenuW(m, MF_SEPARATOR, 0, null());
    item(ID_EXIT, t("Exit"), false);

    let mut pt = POINT { x: 0, y: 0 };
    GetCursorPos(&mut pt);
    SetForegroundWindow(hwnd);
    let cmd = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, pt.x, pt.y, 0, hwnd, null());
    PostMessageW(hwnd, WM_NULL, 0, 0);
    DestroyMenu(m);

    match cmd as usize {
        ID_SETTINGS => sys::spawn_self(&["--settings"]),
        ID_AUTO => set_auto(!auto),
        ID_STARTUP => {
            if !with_app(|a| a.engine.settings.run_as_admin).unwrap_or(false) {
                sys::set_startup(!sys::startup_enabled());
            }
        }
        ID_UPDATE => open_new_version(),
        ID_ABOUT => {
            let s = with_app(|a| a.engine.settings.clone()).unwrap_or_default();
            let double_shift = if s.double_shift { t("Shift Shift (double press) — the same\n") } else { "" };
            message_box(
                hwnd,
                &i18n::f(
                    "OpenSwitch {} — free EN/RU/UA layout switcher.\nGNU GPL v3 license.\n\n{} — fix the last word (again — undo)\n{}\n{} — fix selected text\n{} — enable/disable auto switching\n\nClick the icon — enable/disable auto switching.",
                    &[&env!("CARGO_PKG_VERSION"), &s.hotkey_convert_word, &double_shift, &s.hotkey_convert_selection, &s.hotkey_toggle_auto],
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
