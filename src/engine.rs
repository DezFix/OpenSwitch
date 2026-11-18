// SPDX-License-Identifier: GPL-3.0-or-later
//! Platform-independent typing engine. The Windows hook feeds it raw key events and
//! performs the returned actions (SendInput, layout switch, ...).

use crate::config::Settings;
use crate::detect::Detector;
use crate::layout::{is_word_key, render, KeyStroke, Lang};

pub const VK_BACK: u8 = 0x08;
pub const VK_RETURN: u8 = 0x0D;
pub const VK_SHIFT: u8 = 0x10;
pub const VK_CONTROL: u8 = 0x11;
pub const VK_MENU: u8 = 0x12;
pub const VK_CAPITAL: u8 = 0x14;
pub const VK_SPACE: u8 = 0x20;
pub const VK_LWIN: u8 = 0x5B;
pub const VK_RWIN: u8 = 0x5C;
pub const VK_LSHIFT: u8 = 0xA0;
pub const VK_RSHIFT: u8 = 0xA1;
pub const VK_LCONTROL: u8 = 0xA2;
pub const VK_RCONTROL: u8 = 0xA3;
pub const VK_LMENU: u8 = 0xA4;
pub const VK_RMENU: u8 = 0xA5;

const DOUBLE_SHIFT_MS: u64 = 400;
const SHIFT_TAP_MAX_MS: u64 = 300;
const FORCED_LAYOUT_MS: u64 = 1000;

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Press Backspace n times.
    Backspace(usize),
    /// Type text (as Unicode input, independent of the active layout).
    Type(String),
    /// Press and release a virtual key.
    Tap(u8),
    /// Switch the foreground window's layout.
    SwitchLayout(Lang),
    /// Convert the currently selected text (needs clipboard, done asynchronously by the host).
    ConvertSelection,
    /// Toggle automatic switching.
    ToggleAuto,
    /// Remember a word that must never be auto-converted again (persist it).
    AddException(String),
}

#[derive(Debug, Default)]
pub struct Output {
    pub swallow: bool,
    pub actions: Vec<Action>,
}

impl Output {
    fn pass() -> Self {
        Output::default()
    }
}

#[derive(Debug, Clone)]
struct Word {
    keys: Vec<KeyStroke>,
    /// Layout that was active when the word was typed.
    typed_lang: Lang,
    /// Layout the word is currently displayed in.
    shown_lang: Lang,
    /// Spaces typed after the word.
    trailing: usize,
    auto: bool,
    manual: bool,
}

pub struct Engine {
    pub detector: Detector,
    pub settings: Settings,
    word: Option<Word>,
    lshift: bool,
    rshift: bool,
    ctrl: [bool; 2],
    alt: [bool; 2],
    win: [bool; 2],
    pub caps: bool,
    forced: Option<(Lang, u64)>,
    shift_down_at: Option<u64>,
    shift_clean: bool,
    last_shift_tap: Option<u64>,
}

impl Engine {
    pub fn new(detector: Detector, settings: Settings) -> Self {
        Engine {
            detector,
            settings,
            word: None,
            lshift: false,
            rshift: false,
            ctrl: [false; 2],
            alt: [false; 2],
            win: [false; 2],
            caps: false,
            forced: None,
            shift_down_at: None,
            shift_clean: false,
            last_shift_tap: None,
        }
    }

    pub fn reset(&mut self) {
        self.word = None;
        self.last_shift_tap = None;
        self.shift_clean = false;
    }

    /// Clear the cached modifier state (e.g. after the workstation was locked).
    pub fn reset_modifiers(&mut self) {
        self.lshift = false;
        self.rshift = false;
        self.ctrl = [false; 2];
        self.alt = [false; 2];
        self.win = [false; 2];
        self.reset();
    }

    /// Overwrite the modifier state with the real one (fixes keys released on the secure
    /// desktop, e.g. after Win+L, that the hook never saw).
    pub fn sync_modifiers(&mut self, lshift: bool, rshift: bool, ctrl: [bool; 2], alt: [bool; 2], win: [bool; 2]) {
        self.lshift = lshift;
        self.rshift = rshift;
        self.ctrl = ctrl;
        self.alt = alt;
        self.win = win;
    }

    /// The requested layout switch did not happen: trust the real layout again.
    pub fn cancel_forced(&mut self) {
        self.forced = None;
    }

    pub fn shift(&self) -> bool {
        self.lshift || self.rshift
    }
    fn ctrl(&self) -> bool {
        self.ctrl[0] || self.ctrl[1]
    }
    fn alt(&self) -> bool {
        self.alt[0] || self.alt[1]
    }
    fn win(&self) -> bool {
        self.win[0] || self.win[1]
    }

    /// Text of the current word as it is displayed (for UI / debugging).
    pub fn current_word(&self) -> Option<String> {
        self.word.as_ref().map(|w| render(&w.keys, w.shown_lang))
    }

    fn effective_lang(&mut self, layout: Option<Lang>, now: u64) -> Option<Lang> {
        if let Some((f, t)) = self.forced {
            if layout == Some(f) || now.saturating_sub(t) > FORCED_LAYOUT_MS {
                self.forced = None;
            } else {
                return Some(f);
            }
        }
        layout
    }

    /// Handle one physical (non-injected) key event.
    /// `layout` is the active layout of the focused window (None if it is neither EN nor RU),
    /// `auto_allowed` is false for excluded applications.
    pub fn on_key(&mut self, vk: u8, down: bool, now: u64, layout: Option<Lang>, auto_allowed: bool) -> Output {
        let is_shift = matches!(vk, VK_SHIFT | VK_LSHIFT | VK_RSHIFT);

        if !down {
            match vk {
                VK_LSHIFT | VK_SHIFT => self.lshift = false,
                VK_RSHIFT => self.rshift = false,
                VK_LCONTROL | VK_CONTROL => self.ctrl[0] = false,
                VK_RCONTROL => self.ctrl[1] = false,
                VK_LMENU | VK_MENU => self.alt[0] = false,
                VK_RMENU => self.alt[1] = false,
                VK_LWIN => self.win[0] = false,
                VK_RWIN => self.win[1] = false,
                _ => {}
            }
            if is_shift && !self.shift() {
                let was_tap =
                    self.shift_clean && self.shift_down_at.is_some_and(|t| now.saturating_sub(t) <= SHIFT_TAP_MAX_MS);
                self.shift_down_at = None;
                self.shift_clean = false;
                if was_tap && self.settings.double_shift {
                    if self.last_shift_tap.is_some_and(|t| now.saturating_sub(t) <= DOUBLE_SHIFT_MS) {
                        self.last_shift_tap = None;
                        let lang = self.effective_lang(layout, now);
                        let mut out = Output::pass();
                        out.actions = self.convert_word(lang, now);
                        return out;
                    }
                    self.last_shift_tap = Some(now);
                }
            }
            return Output::pass();
        }

        // ---- key down ----
        if is_shift {
            let first = !self.shift();
            match vk {
                VK_RSHIFT => self.rshift = true,
                _ => self.lshift = true,
            }
            if first {
                self.shift_down_at = Some(now);
                self.shift_clean = !self.ctrl() && !self.alt() && !self.win();
            }
            return Output::pass();
        }
        let modifier = match vk {
            VK_LCONTROL | VK_CONTROL => {
                self.ctrl[0] = true;
                true
            }
            VK_RCONTROL => {
                self.ctrl[1] = true;
                true
            }
            VK_LMENU | VK_MENU => {
                self.alt[0] = true;
                true
            }
            VK_RMENU => {
                self.alt[1] = true;
                true
            }
            VK_LWIN => {
                self.win[0] = true;
                true
            }
            VK_RWIN => {
                self.win[1] = true;
                true
            }
            _ => false,
        };
        self.shift_clean = false;
        self.last_shift_tap = None;
        if modifier {
            return Output::pass();
        }
        if vk == VK_CAPITAL {
            self.caps = !self.caps;
            return Output::pass();
        }

        let (ctrl, alt, shift, win) = (self.ctrl(), self.alt(), self.shift(), self.win());
        let s = &self.settings;
        if s.hotkey_convert_word.matches(vk, ctrl, alt, shift, win) {
            let lang = self.effective_lang(layout, now);
            return Output { swallow: true, actions: self.convert_word(lang, now) };
        }
        if s.hotkey_convert_selection.matches(vk, ctrl, alt, shift, win) {
            self.word = None;
            return Output { swallow: true, actions: vec![Action::ConvertSelection] };
        }
        if s.hotkey_toggle_auto.matches(vk, ctrl, alt, shift, win) {
            return Output { swallow: true, actions: vec![Action::ToggleAuto] };
        }
        if ctrl || alt || win {
            self.word = None;
            return Output::pass();
        }
        let Some(lang) = self.effective_lang(layout, now) else {
            self.word = None;
            return Output::pass();
        };

        if vk == VK_BACK {
            if let Some(w) = &mut self.word {
                if w.trailing > 0 {
                    w.trailing -= 1;
                } else {
                    w.keys.pop();
                    if w.keys.is_empty() {
                        self.word = None;
                    }
                }
            }
            return Output::pass();
        }

        if is_word_key(vk) {
            let ks = KeyStroke { vk, shift, caps: self.caps };
            match &mut self.word {
                Some(w) if w.trailing == 0 && w.shown_lang == lang => w.keys.push(ks),
                _ => {
                    self.word = Some(Word {
                        keys: vec![ks],
                        typed_lang: lang,
                        shown_lang: lang,
                        trailing: 0,
                        auto: false,
                        manual: false,
                    })
                }
            }
            return Output::pass();
        }

        if vk == VK_SPACE || (vk == VK_RETURN && self.settings.switch_on_enter) {
            let out = self.boundary(vk, now, auto_allowed);
            if vk == VK_RETURN {
                self.word = None;
            }
            return out;
        }

        // Any other key (digits, arrows, Tab, Delete, Esc, F-keys, numpad...) ends tracking.
        self.word = None;
        Output::pass()
    }

    fn boundary(&mut self, vk: u8, now: u64, auto_allowed: bool) -> Output {
        let auto_on = self.settings.auto_switch && auto_allowed;
        let Some(w) = &mut self.word else {
            return Output::pass();
        };
        if w.trailing == 0 && !w.manual && auto_on && !w.keys.is_empty() {
            let v = self.detector.check(&w.keys, w.shown_lang);
            if v.switch {
                let from = w.shown_lang;
                let to = from.other();
                let n = render(&w.keys, from).chars().count();
                w.shown_lang = to;
                w.auto = true;
                w.trailing = if vk == VK_SPACE { 1 } else { 0 };
                self.forced = Some((to, now));
                return Output {
                    swallow: true,
                    actions: vec![
                        Action::Backspace(n),
                        Action::Type(render(&w.keys, to)),
                        Action::Tap(vk),
                        Action::SwitchLayout(to),
                    ],
                };
            }
        }
        if vk == VK_SPACE {
            w.trailing += 1;
        }
        Output::pass()
    }

    fn convert_word(&mut self, _lang: Option<Lang>, now: u64) -> Vec<Action> {
        let Some(w) = &mut self.word else {
            return vec![];
        };
        if w.keys.is_empty() {
            return vec![];
        }
        let from = w.shown_lang;
        let to = from.other();
        let n = render(&w.keys, from).chars().count() + w.trailing;
        let mut text = render(&w.keys, to);
        text.push_str(&" ".repeat(w.trailing));
        let mut actions = vec![Action::Backspace(n), Action::Type(text), Action::SwitchLayout(to)];
        if w.auto && to == w.typed_lang {
            // The user reverted an automatic correction: never touch this word again.
            let word = render(&w.keys, w.typed_lang).to_lowercase();
            self.detector.never_switch.insert(word.clone());
            actions.push(Action::AddException(word));
            w.auto = false;
        }
        w.shown_lang = to;
        w.manual = true;
        self.forced = Some((to, now));
        actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::keys_for_text;

    fn typ(e: &mut Engine, text: &str, lang: Lang, t: &mut u64) -> Vec<Action> {
        let mut acts = vec![];
        for k in keys_for_text(text, lang).unwrap() {
            if k.shift {
                e.on_key(VK_LSHIFT, true, *t, Some(lang), true);
            }
            let o = e.on_key(k.vk, true, *t, Some(lang), true);
            acts.extend(o.actions);
            e.on_key(k.vk, false, *t, Some(lang), true);
            if k.shift {
                e.on_key(VK_LSHIFT, false, *t, Some(lang), true);
            }
            *t += 50;
        }
        acts
    }

    #[test]
    fn auto_and_revert() {
        let mut e = Engine::new(Detector::new(), Settings::default());
        let mut t = 1000;
        typ(&mut e, "ghbdtn", Lang::En, &mut t);
        let o = e.on_key(VK_SPACE, true, t, Some(Lang::En), true);
        assert!(o.swallow);
        assert_eq!(
            o.actions,
            vec![
                Action::Backspace(6),
                Action::Type("привет".into()),
                Action::Tap(VK_SPACE),
                Action::SwitchLayout(Lang::Ru)
            ]
        );
        // layout not yet switched by the app: engine must still assume RU
        t += 10;
        let o = e.on_key(crate::config::VK_PAUSE, true, t, Some(Lang::En), true);
        assert!(o.swallow);
        assert_eq!(o.actions[0], Action::Backspace(7));
        assert_eq!(o.actions[1], Action::Type("ghbdtn ".into()));
        assert_eq!(o.actions[3], Action::AddException("ghbdtn".into()));
        // now it is an exception
        t += 2000;
        typ(&mut e, "ghbdtn", Lang::En, &mut t);
        assert!(!e.on_key(VK_SPACE, true, t, Some(Lang::En), true).swallow);
    }

    #[test]
    fn excluded_and_backspace() {
        let mut e = Engine::new(Detector::new(), Settings::default());
        let mut t = 0;
        typ(&mut e, "ghbdtn", Lang::En, &mut t);
        assert!(!e.on_key(VK_SPACE, true, t, Some(Lang::En), false).swallow);
        typ(&mut e, "ghbdtnx", Lang::En, &mut t);
        e.on_key(VK_BACK, true, t, Some(Lang::En), true);
        assert_eq!(e.current_word().unwrap(), "ghbdtn");
        assert!(e.on_key(VK_SPACE, true, t, Some(Lang::En), true).swallow);
    }

    #[test]
    fn double_shift() {
        let mut e = Engine::new(Detector::new(), Settings::default());
        let mut t = 0;
        e.settings.auto_switch = false;
        typ(&mut e, "руддщ", Lang::Ru, &mut t);
        e.on_key(VK_SPACE, true, t, Some(Lang::Ru), true);
        t += 500;
        e.on_key(VK_LSHIFT, true, t, Some(Lang::Ru), true);
        assert!(e.on_key(VK_LSHIFT, false, t + 80, Some(Lang::Ru), true).actions.is_empty());
        e.on_key(VK_LSHIFT, true, t + 200, Some(Lang::Ru), true);
        let o = e.on_key(VK_LSHIFT, false, t + 260, Some(Lang::Ru), true);
        assert_eq!(o.actions[1], Action::Type("hello ".into()));
    }
}
