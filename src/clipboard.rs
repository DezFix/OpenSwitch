// SPDX-License-Identifier: GPL-3.0-or-later
//! Clipboard helpers for "convert selection": take a full snapshot of the clipboard
//! (every format that lives in global memory: text, HTML, RTF, images as DIB, file lists, ...)
//! and put it back afterwards, so the user's clipboard survives the conversion.

use std::ptr::null_mut;
use std::time::Duration;

use windows_sys::Win32::Foundation::{GlobalFree, HANDLE, HWND};
use windows_sys::Win32::System::DataExchange::*;
use windows_sys::Win32::System::Memory::*;

use crate::sys::w;

const CF_UNICODETEXT: u32 = 13;
/// Do not keep more than this in memory (a huge screenshot is ~30 MB as DIB).
const MAX_SNAPSHOT_BYTES: usize = 256 * 1024 * 1024;

/// Formats whose clipboard handle is NOT an HGLOBAL (GDI objects, metafiles, owner-display).
/// They cannot be copied byte-wise; Windows re-synthesises CF_BITMAP from CF_DIB anyway.
fn is_handle_format(fmt: u32) -> bool {
    matches!(
        fmt,
        2   // CF_BITMAP
        | 3 // CF_METAFILEPICT
        | 9 // CF_PALETTE
        | 14 // CF_ENHMETAFILE
        | 0x80 // CF_OWNERDISPLAY
        | 0x82 // CF_DSPBITMAP
        | 0x83 // CF_DSPMETAFILEPICT
        | 0x8E // CF_DSPENHMETAFILE
    ) || (0x300..=0x3FF).contains(&fmt) // CF_GDIOBJFIRST..CF_GDIOBJLAST
}

/// RAII wrapper: the clipboard is closed when this goes out of scope.
struct Open;

impl Open {
    fn new(owner: HWND) -> Option<Open> {
        for _ in 0..25 {
            if unsafe { OpenClipboard(owner) } != 0 {
                return Some(Open);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        log::warn!("clipboard is busy (OpenClipboard failed)");
        None
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        unsafe { CloseClipboard() };
    }
}

unsafe fn read_global(h: HANDLE) -> Option<Vec<u8>> {
    if h.is_null() {
        return None;
    }
    let size = GlobalSize(h);
    let p = GlobalLock(h) as *const u8;
    if p.is_null() {
        return None;
    }
    let v = std::slice::from_raw_parts(p, size).to_vec();
    GlobalUnlock(h);
    Some(v)
}

/// Put `data` on the (already opened and emptied) clipboard. Returns false on failure.
unsafe fn put(fmt: u32, data: &[u8]) -> bool {
    let h = GlobalAlloc(GMEM_MOVEABLE, data.len().max(1));
    if h.is_null() {
        return false;
    }
    let p = GlobalLock(h) as *mut u8;
    if p.is_null() {
        GlobalFree(h);
        return false;
    }
    std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
    GlobalUnlock(h);
    if SetClipboardData(fmt, h).is_null() {
        GlobalFree(h);
        return false;
    }
    true
}

/// Mark what we put on the clipboard as "not for clipboard history / cloud / monitors"
/// (Win+V history, clipboard managers). Must be called while the clipboard is open.
unsafe fn mark_private() {
    let dword_zero = 0u32.to_le_bytes();
    for (name, data) in [
        ("ExcludeClipboardContentFromMonitorProcessing", &[0u8][..]),
        ("CanIncludeInClipboardHistory", &dword_zero[..]),
        ("CanUploadToCloudClipboard", &dword_zero[..]),
    ] {
        let fmt = RegisterClipboardFormatW(w(name).as_ptr());
        if fmt != 0 {
            put(fmt, data);
        }
    }
}

/// Everything that was on the clipboard, in enumeration order.
pub struct Snapshot {
    formats: Vec<(u32, Vec<u8>)>,
}

impl Snapshot {
    pub fn take() -> Option<Snapshot> {
        let _open = Open::new(null_mut())?;
        let mut formats = Vec::new();
        let mut total = 0usize;
        let mut fmt = 0u32;
        unsafe {
            loop {
                fmt = EnumClipboardFormats(fmt);
                if fmt == 0 {
                    break;
                }
                if is_handle_format(fmt) {
                    continue;
                }
                if let Some(data) = read_global(GetClipboardData(fmt)) {
                    total += data.len();
                    if total > MAX_SNAPSHOT_BYTES {
                        log::warn!("clipboard snapshot too large ({total} bytes), not restoring it");
                        return None;
                    }
                    formats.push((fmt, data));
                }
            }
        }
        Some(Snapshot { formats })
    }

    pub fn is_empty(&self) -> bool {
        self.formats.is_empty()
    }

    /// Put the snapshot back (an empty snapshot empties the clipboard).
    pub fn restore(&self) {
        let Some(_open) = Open::new(null_mut()) else { return };
        unsafe {
            EmptyClipboard();
            let mut failed = 0;
            for (fmt, data) in &self.formats {
                if !put(*fmt, data) {
                    failed += 1;
                }
            }
            if !self.formats.is_empty() {
                // The original content is already in Win+V history: do not add a duplicate.
                mark_private();
            }
            if failed > 0 {
                log::warn!("clipboard restore: {failed} of {} formats failed", self.formats.len());
            }
        }
    }
}

/// Unicode text currently on the clipboard.
pub fn text() -> Option<String> {
    let _open = Open::new(null_mut())?;
    unsafe {
        let data = read_global(GetClipboardData(CF_UNICODETEXT))?;
        let units: Vec<u16> = data.as_chunks::<2>().0.iter().map(|&[lo, hi]| u16::from_le_bytes([lo, hi])).collect();
        let len = units.iter().position(|&u| u == 0).unwrap_or(units.len());
        Some(String::from_utf16_lossy(&units[..len]))
    }
}

pub fn sequence_number() -> u32 {
    unsafe { GetClipboardSequenceNumber() }
}
