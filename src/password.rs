// SPDX-License-Identifier: GPL-3.0-or-later
//! Is the focused field a password field? Nothing is fixed (and nothing typed is kept in
//! memory) while the user types a password.
//!
//! Two checks:
//! - classic Win32 edit controls (and RichEdit, WinForms, Delphi edits) carry the
//!   `ES_PASSWORD` style: read right away on our thread, it is cheap;
//! - browsers, WPF, UWP and other programs that draw their own fields report the
//!   "protected" state (`STATE_SYSTEM_PROTECTED`) through Microsoft Active Accessibility
//!   (UI Automation's `IsPassword` maps to it). That is a call into the other process, so it
//!   runs on a worker thread and the answer comes back as `WM_PASSWORD`.

use std::ffi::c_void;
use std::ptr::null;
use std::sync::mpsc::{channel, Receiver, Sender};

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows_sys::Win32::System::Variant::{VariantClear, VARIANT, VT_I4};
use windows_sys::Win32::UI::Accessibility::AccessibleObjectFromEvent;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Answer of the worker: wparam = request number, lparam = 1 for a password field.
pub const WM_PASSWORD: u32 = WM_APP + 8;

/// A focus event to look at on the worker thread.
pub struct Query {
    pub seq: usize,
    pub hwnd: HWND,
    pub id_object: i32,
    pub id_child: i32,
}
// HWND is a plain handle valid in any thread of the session.
unsafe impl Send for Query {}

/// `Some(is_password)` for edit controls whose style tells it, `None` for everything else
/// (the accessibility check is needed then).
pub fn win32_edit(hwnd: HWND) -> Option<bool> {
    if hwnd.is_null() {
        return None;
    }
    let mut buf = [0u16; 64];
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if n <= 0 {
        return None;
    }
    let class = String::from_utf16_lossy(&buf[..n as usize]).to_ascii_lowercase();
    if !class.contains("edit") {
        return None;
    }
    let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) };
    Some(style & ES_PASSWORD != 0)
}

/// Start the worker thread; it posts `WM_PASSWORD` to `notify`.
pub fn spawn_worker(notify: HWND) -> Sender<Query> {
    let (tx, rx) = channel::<Query>();
    let notify = notify as usize;
    std::thread::Builder::new()
        .name("password-check".into())
        .spawn(move || worker(rx, notify as HWND))
        .map_err(|e| log::warn!("password worker: {e}"))
        .ok();
    tx
}

fn worker(rx: Receiver<Query>, notify: HWND) {
    unsafe { CoInitializeEx(null(), COINIT_MULTITHREADED as u32) };
    while let Ok(mut q) = rx.recv() {
        // Focus can jump several times in a row (menus, tabs): only the last one matters.
        while let Ok(next) = rx.try_recv() {
            q = next;
        }
        let pw = std::panic::catch_unwind(|| unsafe { is_protected(q.hwnd, q.id_object, q.id_child) }).unwrap_or(false);
        unsafe { PostMessageW(notify, WM_PASSWORD, q.seq, pw as isize) };
    }
}

/// The first methods of IAccessible's vtable (IUnknown, IDispatch, then IAccessible).
#[repr(C)]
struct IAccessibleVtbl {
    query_interface: usize,
    add_ref: usize,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    get_type_info_count: usize,
    get_type_info: usize,
    get_ids_of_names: usize,
    invoke: usize,
    get_acc_parent: usize,
    get_acc_child_count: usize,
    get_acc_child: usize,
    get_acc_name: usize,
    get_acc_value: usize,
    get_acc_description: usize,
    get_acc_role: usize,
    get_acc_state: unsafe extern "system" fn(*mut c_void, VARIANT, *mut VARIANT) -> i32,
}

/// Does the object of a focus event have the "protected" (password) state?
unsafe fn is_protected(hwnd: HWND, id_object: i32, id_child: i32) -> bool {
    let mut acc: *mut c_void = std::ptr::null_mut();
    let mut child: VARIANT = std::mem::zeroed();
    if AccessibleObjectFromEvent(hwnd, id_object as u32, id_child as u32, &mut acc, &mut child) < 0 || acc.is_null() {
        return false;
    }
    let vtbl = *(acc as *const *const IAccessibleVtbl);
    let mut state: VARIANT = std::mem::zeroed();
    let hr = ((*vtbl).get_acc_state)(acc, child, &mut state);
    let pw = hr >= 0
        && state.Anonymous.Anonymous.vt == VT_I4
        && (state.Anonymous.Anonymous.Anonymous.lVal as u32) & STATE_SYSTEM_PROTECTED != 0;
    VariantClear(&mut state);
    VariantClear(&mut child);
    ((*vtbl).release)(acc);
    pw
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys::w;
    use std::ptr::null_mut;

    /// A hidden top-level EDIT control with the given extra style.
    fn edit(style: u32) -> HWND {
        unsafe {
            CreateWindowExW(
                0,
                w("EDIT").as_ptr(),
                w("").as_ptr(),
                WS_POPUP | style,
                0,
                0,
                100,
                20,
                null_mut(),
                null_mut(),
                null_mut(),
                null(),
            )
        }
    }

    #[test]
    fn password_edit_is_found() {
        unsafe { CoInitializeEx(null(), COINIT_MULTITHREADED as u32) };
        let pw = edit(ES_PASSWORD as u32);
        let plain = edit(0);
        assert!(!pw.is_null() && !plain.is_null());
        assert_eq!(win32_edit(pw), Some(true));
        assert_eq!(win32_edit(plain), Some(false));
        unsafe {
            // The same answer through accessibility (as for browsers): checks the vtable call.
            assert!(is_protected(pw, OBJID_CLIENT, CHILDID_SELF as i32));
            assert!(!is_protected(plain, OBJID_CLIENT, CHILDID_SELF as i32));
            DestroyWindow(pw);
            DestroyWindow(plain);
        }
        assert_eq!(win32_edit(null_mut()), None);
    }
}
