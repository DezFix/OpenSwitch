// SPDX-License-Identifier: GPL-3.0-or-later
//! The last automatic fixes, for "Recent fixes" in the settings window.
//!
//! Typed text never goes to disk: the tray app keeps the list in memory and shares it with
//! the settings window through a small block of shared memory (a pagefile-backed file
//! mapping that disappears when OpenSwitch exits).
//!
//! Layout of the block: a change counter (odd while being written), the length of the text,
//! then UTF-8 lines `typed \t before \t after`, newest first.

use std::collections::VecDeque;
use std::ptr::null;
use std::sync::atomic::{AtomicU32, Ordering};

use openswitch::engine::Fix;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Memory::*;

use crate::sys::w;

const NAME: &str = "Local\\OpenSwitch.RecentFixes";
const SIZE: usize = 16 * 1024;
const HEAD: usize = 8;
/// How many fixes are kept.
pub const KEEP: usize = 30;

/// The text stored for `fixes` (whole lines only, at most `max` bytes).
fn encode(fixes: &VecDeque<Fix>, max: usize) -> String {
    let mut text = String::new();
    for f in fixes {
        let line = format!("{}\t{}\t{}\n", f.typed, f.before, f.after);
        if text.len() + line.len() > max {
            break;
        }
        text.push_str(&line);
    }
    text
}

fn decode(text: &str) -> Vec<Fix> {
    text.lines()
        .filter_map(|l| {
            let mut p = l.split('\t');
            Some(Fix { typed: p.next()?.into(), before: p.next()?.into(), after: p.next()?.into() })
        })
        .collect()
}

/// The tray app's side: owns the block.
pub struct Writer {
    handle: HANDLE,
    view: *mut u8,
}

impl Writer {
    pub fn create() -> Option<Writer> {
        unsafe {
            let handle =
                CreateFileMappingW(INVALID_HANDLE_VALUE, null(), PAGE_READWRITE, 0, SIZE as u32, w(NAME).as_ptr());
            if handle.is_null() {
                return None;
            }
            let view = MapViewOfFile(handle, FILE_MAP_WRITE | FILE_MAP_READ, 0, 0, SIZE).Value as *mut u8;
            if view.is_null() {
                CloseHandle(handle);
                return None;
            }
            Some(Writer { handle, view })
        }
    }

    pub fn write(&self, fixes: &VecDeque<Fix>) {
        let text = encode(fixes, SIZE - HEAD);
        unsafe {
            let seq = &*(self.view as *const AtomicU32);
            seq.fetch_add(1, Ordering::AcqRel); // odd: being written
            (self.view.add(4) as *mut u32).write_volatile(text.len() as u32);
            std::ptr::copy_nonoverlapping(text.as_ptr(), self.view.add(HEAD), text.len());
            seq.fetch_add(1, Ordering::AcqRel);
        }
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        unsafe {
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: self.view as *mut _ });
            CloseHandle(self.handle);
        }
    }
}

/// The settings window's side: the fixes, newest first (empty if OpenSwitch is not running).
pub fn read() -> Vec<Fix> {
    unsafe {
        let handle = OpenFileMappingW(FILE_MAP_READ, 0, w(NAME).as_ptr());
        if handle.is_null() {
            return Vec::new();
        }
        let view = MapViewOfFile(handle, FILE_MAP_READ, 0, 0, SIZE).Value as *const u8;
        let mut out = Vec::new();
        if !view.is_null() {
            let seq = &*(view as *const AtomicU32);
            for _ in 0..10 {
                let before = seq.load(Ordering::Acquire);
                if before % 2 == 1 {
                    std::thread::yield_now();
                    continue;
                }
                let len = ((view.add(4) as *const u32).read_volatile() as usize).min(SIZE - HEAD);
                let bytes = std::slice::from_raw_parts(view.add(HEAD), len).to_vec();
                if seq.load(Ordering::Acquire) == before {
                    out = decode(&String::from_utf8_lossy(&bytes));
                    break;
                }
            }
            UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: view as *mut _ });
        }
        CloseHandle(handle);
        out
    }
}

/// Add a fix to the list kept by the tray app.
pub fn push(list: &mut VecDeque<Fix>, fix: Fix) {
    list.push_front(fix);
    list.truncate(KEEP);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fix(a: &str, b: &str) -> Fix {
        Fix { typed: a.into(), before: a.into(), after: b.into() }
    }

    #[test]
    fn shared_block() {
        let mut list = VecDeque::new();
        push(&mut list, fix("ghbdtn", "привет"));
        push(&mut list, fix("руддщ", "hello"));
        let w = Writer::create().expect("mapping");
        w.write(&list);
        assert_eq!(read(), vec![fix("руддщ", "hello"), fix("ghbdtn", "привет")]);
        w.write(&VecDeque::new());
        assert!(read().is_empty());
        drop(w);
        assert!(read().is_empty()); // gone with the tray app
    }

    #[test]
    fn limits() {
        let mut list = VecDeque::new();
        for i in 0..100 {
            push(&mut list, fix(&format!("w{i}"), "x"));
        }
        assert_eq!(list.len(), KEEP);
        assert_eq!(list[0].typed, "w99");
        let text = encode(&list, 20);
        assert!(text.len() <= 20 && text.ends_with('\n'));
    }
}
