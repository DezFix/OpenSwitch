// SPDX-License-Identifier: GPL-3.0-or-later
//! Converting the selected text through the clipboard.

use super::*;

/// Runs on a worker thread: copy selection, convert, type it back, re-select it, and put the
/// user's clipboard back exactly as it was (all formats, not only text).
pub(super) fn convert_selection(layouts: &Layouts) {
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
        // If the snapshot fails (clipboard busy or huge), do not touch the clipboard at all.
        let Some(saved) = clipboard::Snapshot::take() else {
            log::warn!("convert selection: could not save the clipboard, skipped");
            return;
        };
        let seq = clipboard::sequence_number();
        inputs.push(key_input(VK_CONTROL, false));
        push_tap(&mut inputs, b'C' as u16);
        inputs.push(key_input(VK_CONTROL, true));
        send(&inputs);

        let mut changed = false;
        for _ in 0..40 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            if clipboard::sequence_number() != seq {
                changed = true;
                break;
            }
        }
        if !changed {
            // Nothing selected (or the program ignores Ctrl+C): the clipboard is untouched.
            log::debug!("convert selection: nothing was copied");
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
        let Some(text) = clipboard::text() else {
            saved.restore();
            return;
        };
        // Some editors (VS Code...) copy the whole line when nothing is selected.
        let single_line_with_newline = text.ends_with('\n') && text.trim_end().lines().count() <= 1;
        if text.is_empty() || single_line_with_newline || text.chars().count() > 5000 {
            saved.restore();
            return;
        }
        let target = layouts.flip_target(layouts.guess_text_lang(&text));
        let converted = layouts.flip_text(&text);
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
        PostMessageW(main, WM_SWITCH_LAYOUT, lang_to_wparam(target), 0);
        std::thread::sleep(std::time::Duration::from_millis(100));
        saved.restore();
        log::debug!("convert selection: done, clipboard restored (empty before: {})", saved.is_empty());
    }
}
