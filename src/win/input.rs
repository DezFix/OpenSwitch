// SPDX-License-Identifier: GPL-3.0-or-later
//! Input injection: key taps and Unicode text through SendInput.

use super::*;

pub(super) fn key_input(vk: u16, up: bool) -> INPUT {
    let mut i: INPUT = unsafe { std::mem::zeroed() };
    i.r#type = INPUT_KEYBOARD;
    i.Anonymous.ki =
        KEYBDINPUT { wVk: vk, wScan: 0, dwFlags: if up { KEYEVENTF_KEYUP } else { 0 }, time: 0, dwExtraInfo: OUR_MARK };
    i
}

pub(super) fn unicode_input(unit: u16, up: bool) -> INPUT {
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

pub(super) fn push_tap(v: &mut Vec<INPUT>, vk: u16) {
    v.push(key_input(vk, false));
    v.push(key_input(vk, true));
}

pub(super) fn push_text(v: &mut Vec<INPUT>, text: &str) {
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

pub(super) fn send(inputs: &[INPUT]) {
    if inputs.is_empty() {
        return;
    }
    unsafe {
        SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}
