// SPDX-License-Identifier: GPL-3.0-or-later
//! Platform-independent typing engine. The Windows hook feeds it raw key events and
//! performs the returned actions (SendInput, layout switch, ...).

use std::collections::HashSet;

use crate::config::Settings;
use crate::detect::Detector;
use crate::layout::{KeyMap, KeyStroke, Lang};

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
/// Sensitivity factor for the first word after the user switched the layout on purpose.
const CAREFUL: f32 = 0.75;
/// Shortest word for the CapsLock fix (пРИВЕТ -> Привет).
const CAPS_FIX_MIN: usize = 3;
/// Shortest word learned from a manual fix.
const LEARN_MIN: usize = 2;

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
    /// Remember a word that must always be converted (learned from a manual fix).
    Learn(String),
    /// Drop a learned word again (the user put it back).
    Forget(String),
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
    /// Changed automatically (other layout and/or CapsLock fix).
    auto: bool,
    manual: bool,
    /// Keys as typed, before the CapsLock fix changed them.
    original: Option<Vec<KeyStroke>>,
    /// The user switched the layout themselves right before this word.
    after_switch: bool,
    /// Fixed by hand before it was finished: learn it at the word boundary.
    learn_pending: bool,
    /// Learned (added to `always_switch`) from this word.
    learned: bool,
}

/// Word typed with CapsLock on by accident: Shift held for the first letter only, so it came
/// out as пРИВЕТ. All keys must be letters in `map`.
fn caps_accident(keys: &[KeyStroke], map: &KeyMap) -> bool {
    keys.len() >= CAPS_FIX_MIN
        && keys.iter().all(|k| k.caps && map.caps_sensitive(k.vk))
        && keys[0].shift
        && keys[1..].iter().all(|k| !k.shift)
}

/// The same keys as if CapsLock had been off: capital first letter, the rest small.
fn without_caps(keys: &[KeyStroke]) -> Vec<KeyStroke> {
    keys.iter().enumerate().map(|(i, k)| KeyStroke { vk: k.vk, shift: i == 0, caps: false }).collect()
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
    /// Layout of the last finished word (None after a click or a window change).
    prev_lang: Option<Lang>,
    /// Words whose automatic fix was undone with Backspace once in this session; a second
    /// undo makes the word an exception.
    undone: HashSet<String>,
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
            prev_lang: None,
            undone: HashSet::new(),
        }
    }

    pub fn reset(&mut self) {
        self.word = None;
        self.prev_lang = None;
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
        self.word.as_ref().map(|w| self.detector.layouts.render(&w.keys, w.shown_lang))
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
            if self.settings.undo_backspace {
                if let Some(actions) = self.undo_auto(now) {
                    return Output { swallow: true, actions };
                }
            }
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

        if self.detector.layouts.is_word_key(vk) {
            let ks = KeyStroke { vk, shift, caps: self.caps };
            let after_switch = self.settings.careful_after_switch && self.prev_lang.is_some_and(|p| p != lang);
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
                        original: None,
                        after_switch,
                        learn_pending: false,
                        learned: false,
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
        let caps_on = self.settings.fix_caps && auto_allowed;
        let Some(w) = &mut self.word else {
            return Output::pass();
        };
        if w.trailing == 0 && !w.manual && !w.keys.is_empty() {
            let from = w.shown_lang;
            let care = if w.after_switch { CAREFUL } else { 1.0 };
            let switch = auto_on && self.detector.check_careful(&w.keys, from, care).switch;
            let to = if switch { from.other() } else { from };
            let caps_fix = caps_on && caps_accident(&w.keys, self.detector.layouts.map(to));
            if switch || caps_fix {
                let n = self.detector.layouts.render(&w.keys, from).chars().count();
                if caps_fix {
                    let fixed = without_caps(&w.keys);
                    w.original = Some(std::mem::replace(&mut w.keys, fixed));
                }
                let mut actions = vec![
                    Action::Backspace(n),
                    Action::Type(self.detector.layouts.render(&w.keys, to)),
                    Action::Tap(vk),
                ];
                if switch {
                    actions.push(Action::SwitchLayout(to));
                    self.forced = Some((to, now));
                }
                if caps_fix {
                    // Injected keys are not seen by the hook: track the new state here.
                    actions.push(Action::Tap(VK_CAPITAL));
                    self.caps = false;
                }
                w.shown_lang = to;
                w.auto = true;
                w.trailing = if vk == VK_SPACE { 1 } else { 0 };
                self.prev_lang = Some(to);
                return Output { swallow: true, actions };
            }
        }
        let mut out = Output::pass();
        if w.trailing == 0 {
            self.prev_lang = Some(w.shown_lang);
            if w.learn_pending {
                w.learn_pending = false;
                w.learned = true;
                let word = self.detector.layouts.render(&w.keys, w.typed_lang).to_lowercase();
                self.detector.always_switch.insert(word.clone());
                out.actions.push(Action::Learn(word));
            }
        }
        if vk == VK_SPACE {
            w.trailing += 1;
        }
        out
    }

    /// Backspace right after an automatic fix (word + the space that triggered it): put the
    /// word back as typed, in the layout it was typed in. Undoing the same word twice in a
    /// session makes it an exception.
    fn undo_auto(&mut self, now: u64) -> Option<Vec<Action>> {
        let w = self.word.as_mut()?;
        if !w.auto || w.manual || w.trailing != 1 {
            return None;
        }
        let layouts = &self.detector.layouts;
        let n = layouts.render(&w.keys, w.shown_lang).chars().count() + 1;
        let keys = w.original.take().unwrap_or_else(|| w.keys.clone());
        let mut actions = vec![Action::Backspace(n), Action::Type(layouts.render(&keys, w.typed_lang))];
        if w.shown_lang != w.typed_lang {
            actions.push(Action::SwitchLayout(w.typed_lang));
            self.forced = Some((w.typed_lang, now));
            let word = layouts.render(&keys, w.typed_lang).to_lowercase();
            if !self.undone.insert(word.clone()) {
                self.detector.never_switch.insert(word.clone());
                actions.push(Action::AddException(word));
            }
        }
        w.keys = keys;
        w.shown_lang = w.typed_lang;
        w.trailing = 0;
        w.auto = false;
        w.manual = true;
        self.prev_lang = Some(w.typed_lang);
        Some(actions)
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
        let n = self.detector.layouts.render(&w.keys, from).chars().count() + w.trailing;
        let mut text = self.detector.layouts.render(&w.keys, to);
        text.push_str(&" ".repeat(w.trailing));
        let mut actions = vec![Action::Backspace(n), Action::Type(text), Action::SwitchLayout(to)];
        let reverting = to == w.typed_lang;
        let word = self.detector.layouts.render(&w.keys, w.typed_lang).to_lowercase();
        if w.auto && reverting {
            // The user reverted an automatic correction: never touch this word again.
            if self.detector.always_switch.remove(&word) {
                actions.push(Action::Forget(word.clone()));
            }
            self.detector.never_switch.insert(word.clone());
            actions.push(Action::AddException(word));
            w.auto = false;
        } else if reverting {
            // Fixed by hand and put back: nothing to learn.
            w.learn_pending = false;
            if w.learned && self.detector.always_switch.remove(&word) {
                actions.push(Action::Forget(word));
            }
            w.learned = false;
        } else if self.settings.learn_manual
            && !w.auto
            && w.keys.len() >= LEARN_MIN
            && !self.detector.never_switch.contains(&word)
        {
            // Fixed by hand: the detector missed it. Learn it once the word is finished.
            if w.trailing > 0 {
                w.learned = true;
                self.detector.always_switch.insert(word.clone());
                actions.push(Action::Learn(word));
            } else {
                w.learn_pending = true;
            }
        }
        w.shown_lang = to;
        w.manual = true;
        w.original = None;
        if w.trailing > 0 {
            self.prev_lang = Some(to);
        }
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

    fn space(e: &mut Engine, lang: Lang, t: &mut u64) -> Output {
        *t += 50;
        e.on_key(VK_SPACE, true, *t, Some(lang), true)
    }

    #[test]
    fn backspace_undo() {
        let mut e = Engine::new(Detector::new(), Settings::default());
        let mut t = 0;
        typ(&mut e, "ghbdtn", Lang::En, &mut t);
        assert!(space(&mut e, Lang::En, &mut t).swallow);
        // Backspace right away: the word comes back as typed, layout back to English
        let o = e.on_key(VK_BACK, true, t + 50, Some(Lang::Ru), true);
        assert!(o.swallow);
        assert_eq!(
            o.actions,
            vec![Action::Backspace(7), Action::Type("ghbdtn".into()), Action::SwitchLayout(Lang::En)]
        );
        // a space now does not fix it again
        assert!(!space(&mut e, Lang::En, &mut t).swallow);
        // second undo of the same word in a session -> exception
        t += 2000;
        typ(&mut e, "ghbdtn", Lang::En, &mut t);
        assert!(space(&mut e, Lang::En, &mut t).swallow);
        let o = e.on_key(VK_BACK, true, t + 50, Some(Lang::Ru), true);
        assert_eq!(o.actions.last(), Some(&Action::AddException("ghbdtn".into())));
    }

    #[test]
    fn backspace_later_is_plain_backspace() {
        let mut e = Engine::new(Detector::new(), Settings::default());
        let mut t = 0;
        typ(&mut e, "ghbdtn", Lang::En, &mut t);
        space(&mut e, Lang::En, &mut t);
        space(&mut e, Lang::Ru, &mut t); // two spaces: Backspace only deletes a space
        assert!(!e.on_key(VK_BACK, true, t + 50, Some(Lang::Ru), true).swallow);
        // and with the option off nothing is undone
        let mut e = Engine::new(Detector::new(), Settings { undo_backspace: false, ..Settings::default() });
        typ(&mut e, "ghbdtn", Lang::En, &mut t);
        space(&mut e, Lang::En, &mut t);
        assert!(!e.on_key(VK_BACK, true, t + 50, Some(Lang::Ru), true).swallow);
    }

    fn typ_caps(e: &mut Engine, text: &str, lang: Lang, t: &mut u64) {
        // CapsLock on, Shift for the first letter: the user meant "Text"
        e.on_key(VK_CAPITAL, true, *t, Some(lang), true);
        e.on_key(VK_CAPITAL, false, *t, Some(lang), true);
        for (i, k) in keys_for_text(text, lang).unwrap().into_iter().enumerate() {
            if i == 0 {
                e.on_key(VK_LSHIFT, true, *t, Some(lang), true);
            }
            e.on_key(k.vk, true, *t, Some(lang), true);
            e.on_key(k.vk, false, *t, Some(lang), true);
            if i == 0 {
                e.on_key(VK_LSHIFT, false, *t, Some(lang), true);
            }
            *t += 50;
        }
    }

    #[test]
    fn caps_lock_fix() {
        let mut e = Engine::new(Detector::new(), Settings::default());
        let mut t = 0;
        typ_caps(&mut e, "привет", Lang::Ru, &mut t);
        let o = space(&mut e, Lang::Ru, &mut t);
        assert!(o.swallow);
        assert_eq!(
            o.actions,
            vec![Action::Backspace(6), Action::Type("Привет".into()), Action::Tap(VK_SPACE), Action::Tap(VK_CAPITAL)]
        );
        assert!(!e.caps);
        // Backspace undo brings back what was typed
        let o = e.on_key(VK_BACK, true, t + 50, Some(Lang::Ru), true);
        assert_eq!(o.actions[1], Action::Type("пРИВЕТ".into()));

        // CapsLock accident and wrong layout at once: gHBDTN -> Привет
        let mut e = Engine::new(Detector::new(), Settings::default());
        typ_caps(&mut e, "ghbdtn", Lang::En, &mut t);
        let o = space(&mut e, Lang::En, &mut t);
        assert_eq!(o.actions[1], Action::Type("Привет".into()));
        assert!(o.actions.contains(&Action::SwitchLayout(Lang::Ru)));
        assert!(o.actions.contains(&Action::Tap(VK_CAPITAL)));

        // Real all-caps words are left alone
        let mut e = Engine::new(Detector::new(), Settings::default());
        e.on_key(VK_CAPITAL, true, t, Some(Lang::Ru), true);
        typ(&mut e, "привет", Lang::Ru, &mut t);
        assert!(!space(&mut e, Lang::Ru, &mut t).swallow);
    }

    #[test]
    fn careful_after_manual_switch() {
        let mut e = Engine::new(Detector::new(), Settings::default());
        let mut t = 0;
        typ(&mut e, "привет", Lang::Ru, &mut t);
        space(&mut e, Lang::Ru, &mut t);
        // the user switches to English and types a word
        t += 1500;
        typ(&mut e, "rust", Lang::En, &mut t);
        assert!(e.word.as_ref().unwrap().after_switch);
        assert!(!space(&mut e, Lang::En, &mut t).swallow);
        // obvious mistakes are still fixed right after a switch
        typ(&mut e, "привет", Lang::Ru, &mut t);
        space(&mut e, Lang::Ru, &mut t);
        t += 1500;
        typ(&mut e, "ghbdtn", Lang::En, &mut t);
        assert!(space(&mut e, Lang::En, &mut t).swallow);
        // a click (reset) forgets the previous word
        e.reset();
        typ(&mut e, "hello", Lang::En, &mut t);
        assert!(!e.word.as_ref().unwrap().after_switch);
    }

    #[test]
    fn learn_from_manual_fix() {
        let mut e = Engine::new(Detector::new(), Settings { auto_switch: true, ..Settings::default() });
        let mut t = 0;
        // "ghbdtn" is fixed automatically anyway; take a word the detector keeps: "here"
        typ(&mut e, "here", Lang::En, &mut t);
        assert!(!space(&mut e, Lang::En, &mut t).swallow);
        // fixed by hand after the space -> learned right away
        let o = e.on_key(crate::config::VK_PAUSE, true, t + 50, Some(Lang::En), true);
        assert_eq!(o.actions[1], Action::Type("руку ".into()));
        assert_eq!(o.actions.last(), Some(&Action::Learn("here".into())));
        // next time it is fixed automatically
        t += 2000;
        e.reset();
        typ(&mut e, "here", Lang::En, &mut t);
        let o = space(&mut e, Lang::En, &mut t);
        assert!(o.swallow);
        assert_eq!(o.actions[1], Action::Type("руку".into()));
        // reverting that automatic fix forgets it and makes it an exception
        let o = e.on_key(crate::config::VK_PAUSE, true, t + 50, Some(Lang::Ru), true);
        assert!(o.actions.contains(&Action::Forget("here".into())));
        assert!(o.actions.contains(&Action::AddException("here".into())));
    }

    #[test]
    fn learn_waits_for_the_end_of_the_word() {
        let mut e = Engine::new(Detector::new(), Settings::default());
        let mut t = 0;
        typ(&mut e, "here", Lang::En, &mut t);
        // Pause before the space: nothing learned yet (the word may go on)
        let o = e.on_key(crate::config::VK_PAUSE, true, t, Some(Lang::En), true);
        assert!(!o.actions.iter().any(|a| matches!(a, Action::Learn(_))));
        let o = space(&mut e, Lang::Ru, &mut t);
        assert!(!o.swallow);
        assert_eq!(o.actions, vec![Action::Learn("here".into())]);

        // converted and converted back: nothing learned
        let mut e = Engine::new(Detector::new(), Settings::default());
        typ(&mut e, "here", Lang::En, &mut t);
        e.on_key(crate::config::VK_PAUSE, true, t, Some(Lang::En), true);
        e.on_key(crate::config::VK_PAUSE, true, t + 10, Some(Lang::Ru), true);
        assert!(space(&mut e, Lang::En, &mut t).actions.is_empty());

        // and with the option off nothing is learned
        let mut e = Engine::new(Detector::new(), Settings { learn_manual: false, ..Settings::default() });
        typ(&mut e, "here", Lang::En, &mut t);
        space(&mut e, Lang::En, &mut t);
        let o = e.on_key(crate::config::VK_PAUSE, true, t + 50, Some(Lang::En), true);
        assert!(!o.actions.iter().any(|a| matches!(a, Action::Learn(_))));
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

    #[test]
    fn cyrillic_side_custom_layout() {
        use crate::layout::{KeyMap, Layouts};
        // A Ukrainian layout with і on S: typing "love" there must still become English.
        let mut d = Detector::new();
        d.set_ukrainian(true);
        let mut ru = KeyMap::builtin(Lang::Ru);
        ru.set(b'S', Some('і'), Some('І'), true);
        d.layouts = Layouts { en: KeyMap::builtin(Lang::En), ru };
        let mut e = Engine::new(d, Settings::default());
        let mut t = 0;
        for k in e.detector.layouts.keys_for_text("дщму", Lang::Ru).unwrap() {
            e.on_key(k.vk, true, t, Some(Lang::Ru), true);
            e.on_key(k.vk, false, t, Some(Lang::Ru), true);
            t += 50;
        }
        let o = e.on_key(VK_SPACE, true, t, Some(Lang::Ru), true);
        assert_eq!(o.actions[1], Action::Type("love".into()));
        assert_eq!(o.actions[3], Action::SwitchLayout(Lang::En));
        // and "ghbdsn" typed in English becomes Ukrainian "привіт"
        t += 1000;
        for k in e.detector.layouts.keys_for_text("ghbdsn", Lang::En).unwrap() {
            e.on_key(k.vk, true, t, Some(Lang::En), true);
            e.on_key(k.vk, false, t, Some(Lang::En), true);
            t += 50;
        }
        let o = e.on_key(VK_SPACE, true, t, Some(Lang::En), true);
        assert_eq!(o.actions[1], Action::Type("привіт".into()));
    }
}
