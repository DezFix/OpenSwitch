// SPDX-License-Identifier: GPL-3.0-or-later
//! Platform-independent typing engine. The Windows hook feeds it raw key events and
//! performs the returned actions (SendInput, layout switch, ...).

use std::collections::HashSet;

use crate::config::Settings;
use crate::detect::Detector;
use crate::layout::{KeyStroke, Lang};
use crate::spelling;
use crate::text::{capitalize_keys, caps_accident, ends_sentence, needs_capital, without_caps, SENTENCE_END};

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
    /// Remember a word that must always be converted (learned from a manual fix):
    /// the word as typed and the code of the layout it was converted to.
    Learn(String, String),
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
    /// Layouts a manual fix cycles through: the typed one first, then the others from the
    /// most to the least plausible (filled on the first manual fix).
    cycle: Vec<Lang>,
    /// The word starts a sentence (it came after . ! ? or Enter).
    sentence_start: bool,
}

/// Module "Spelling" for a word shown in layout `lang`: the keys of the fixed word. Only words
/// unknown in every language of the layout are touched (a Ukrainian layout that also stands
/// for Russian keeps Russian words).
fn spell(d: &Detector, text: &str, lang: Lang) -> Option<Vec<KeyStroke>> {
    let languages = d.layouts.languages(lang);
    let (&main, others) = languages.split_first()?;
    if others.iter().any(|&l| d.model(l).clean(text).is_some_and(|w| d.model(l).rank(&w).is_some())) {
        return None;
    }
    let fixed = spelling::correct(d.model(main), Some(d.layouts.map(lang)), text)?;
    d.layouts.keys_for_text(&fixed, lang)
}

/// How much better the fixed word must look in its language than the word as typed looks in
/// the layout it was typed in (character model, average log-probability per letter). Chosen
/// with `examples/eval_typo_layout.rs`.
pub const OTHER_LAYOUT_GAIN: f32 = 1.0;

/// A typo in a word typed in the wrong layout: `ghdbtn` is «првиет» in Russian, one swap from
/// «привет». Only when the word as typed is no known word of its own layout, it is a clear
/// typo (the spelling module is sure) in exactly one other layout, and the fixed word looks
/// much more like its language than the typed one looks like its own. Only between Latin
/// and Cyrillic layouts. Returns the layout and the keys of the fixed word.
pub fn spell_other_layout(d: &Detector, keys: &[KeyStroke], from: Lang, gain: f32) -> Option<(Lang, Vec<KeyStroke>)> {
    let shown = d.layouts.render(keys, from);
    let from_langs = d.layouts.languages(from);
    let mut from_score = f32::MIN;
    for &l in from_langs {
        let m = d.model(l);
        let w = m.clean(&shown)?;
        if m.rank(&w).is_some() {
            return None; // a word as typed
        }
        from_score = from_score.max(m.score(&w));
    }
    let mut found = None;
    for to in d.layouts.all() {
        let Some(layout) = d.layouts.get(to) else { continue };
        if to == from || !layout.target || d.layouts.is_latin(to) == d.layouts.is_latin(from) {
            continue;
        }
        let Some(fixed) = spell(d, &d.layouts.render(keys, to), to) else { continue };
        let main = d.model(*d.layouts.languages(to).first()?);
        let fixed_score = main.clean(&d.layouts.render(&fixed, to)).map_or(f32::MIN, |w| main.score(&w));
        if fixed_score - from_score < gain {
            continue;
        }
        if found.is_some() {
            return None; // a typo in two layouts: unclear
        }
        found = Some((to, fixed));
    }
    found
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
    /// The next word starts a sentence: the last thing typed was . ! ? (and spaces) or Enter.
    /// False when unknown (after a click, a window change, arrows...).
    sentence_start: bool,
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
            sentence_start: false,
            undone: HashSet::new(),
        }
    }

    pub fn reset(&mut self) {
        self.word = None;
        self.prev_lang = None;
        self.sentence_start = false;
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
                if was_tap && self.settings.module_layout && self.settings.double_shift {
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
        let hotkeys = s.module_layout;
        if hotkeys && s.hotkey_convert_word.matches(vk, ctrl, alt, shift, win) {
            let lang = self.effective_lang(layout, now);
            return Output { swallow: true, actions: self.convert_word(lang, now) };
        }
        if hotkeys && s.hotkey_convert_selection.matches(vk, ctrl, alt, shift, win) {
            self.word = None;
            return Output { swallow: true, actions: vec![Action::ConvertSelection] };
        }
        if hotkeys && s.hotkey_toggle_auto.matches(vk, ctrl, alt, shift, win) {
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
            if self.word.is_none() {
                self.sentence_start = false;
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

        if let Some(out) = self.space_before_punctuation(vk, shift, lang, auto_allowed) {
            return out;
        }

        if self.detector.layouts.is_word_key(vk) {
            let ks = KeyStroke { vk, shift, caps: self.caps };
            let sentence_start = self.sentence_start;
            let ch = self.detector.layouts.render_key(ks, lang);
            self.sentence_start = ch.is_some_and(|c| SENTENCE_END.contains(&c));
            let after_switch = self.settings.careful_after_switch && self.prev_lang.is_some_and(|p| p != lang);
            self.detector.layouts.touch(lang, now);
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
                        cycle: Vec::new(),
                        sentence_start,
                    })
                }
            }
            return Output::pass();
        }

        if vk == VK_SPACE || (vk == VK_RETURN && self.settings.switch_on_enter) {
            let out = self.boundary(vk, now, auto_allowed);
            if vk == VK_RETURN {
                self.word = None;
                self.sentence_start = true;
            }
            return out;
        }

        // Any other key (digits, arrows, Tab, Delete, Esc, F-keys, numpad...) ends tracking.
        self.word = None;
        self.sentence_start =
            vk == VK_RETURN || self.punctuation(vk, shift, lang).is_some_and(|c| c == '!' || c == '?');
        Output::pass()
    }

    fn boundary(&mut self, vk: u8, now: u64, auto_allowed: bool) -> Output {
        let s = &self.settings;
        let auto_on = s.module_layout && s.auto_switch && auto_allowed;
        let caps_on = s.module_text && s.fix_caps && auto_allowed;
        let capital_on = s.module_text && s.capitalize && auto_allowed;
        let spell_on = s.module_spelling && auto_allowed;
        let Some(w) = &mut self.word else {
            return Output::pass();
        };
        if w.trailing == 0 && !w.manual && !w.keys.is_empty() {
            let from = w.shown_lang;
            let care = if w.after_switch { CAREFUL } else { 1.0 };
            let verdict = if auto_on { Some(self.detector.check_careful(&w.keys, from, care)) } else { None };
            let mut switch = verdict.as_ref().is_some_and(|v| v.switch);
            let mut to = verdict.filter(|v| v.switch).map_or(from, |v| v.to);
            // A typo in a word the detector did not know because of it: `ghdbtn` -> «привет».
            let mut other_fix = None;
            if !switch
                && auto_on
                && spell_on
                && !w.after_switch
                && !self.detector.never_switch.contains(&self.detector.layouts.render(&w.keys, from).to_lowercase())
            {
                if let Some((l, k)) = spell_other_layout(&self.detector, &w.keys, from, OTHER_LAYOUT_GAIN) {
                    switch = true;
                    to = l;
                    other_fix = Some(k);
                }
            }
            let caps_fix = caps_on && caps_accident(&w.keys, self.detector.layouts.map(to));
            let n = self.detector.layouts.render(&w.keys, from).chars().count();
            let mut keys = if caps_fix { without_caps(&w.keys) } else { w.keys.clone() };
            let typed = self.detector.layouts.render(&w.keys, w.typed_lang).to_lowercase();
            let spelled = if let Some(k) = other_fix {
                keys = k;
                true
            } else {
                spell_on
                    && !self.detector.never_switch.contains(&typed)
                    && match spell(&self.detector, &self.detector.layouts.render(&keys, to), to) {
                        Some(fixed) => {
                            keys = fixed;
                            true
                        }
                        None => false,
                    }
            };
            let capital = capital_on
                && needs_capital(
                    &self.detector.layouts.render(&keys, to),
                    w.sentence_start,
                    self.detector.layouts.is_latin(to),
                )
                && capitalize_keys(&mut keys, self.detector.layouts.map(to));
            if switch || caps_fix || capital || spelled {
                if caps_fix || capital || spelled {
                    w.original = Some(std::mem::replace(&mut w.keys, keys));
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
                self.sentence_start = ends_sentence(&self.detector.layouts.render(&w.keys, to));
                return Output { swallow: true, actions };
            }
        }
        let mut out = Output::pass();
        if w.trailing == 0 {
            self.prev_lang = Some(w.shown_lang);
            self.sentence_start = ends_sentence(&self.detector.layouts.render(&w.keys, w.shown_lang));
            if w.learn_pending {
                w.learn_pending = false;
                w.learned = true;
                let word = self.detector.layouts.render(&w.keys, w.typed_lang).to_lowercase();
                let code = self.detector.layouts.code(w.shown_lang).to_string();
                self.detector.always_switch.insert(word.clone(), Some(code.clone()));
                out.actions.push(Action::Learn(word, code));
            }
        }
        if vk == VK_SPACE {
            w.trailing += 1;
        }
        out
    }

    /// The punctuation mark a key types in `lang`, if any. Word keys are read from the layout;
    /// on the digit row only the marks that are the same in all supported layouts are known.
    fn punctuation(&self, vk: u8, shift: bool, lang: Lang) -> Option<char> {
        let c = if self.detector.layouts.is_word_key(vk) {
            self.detector.layouts.render_key(KeyStroke { vk, shift, caps: false }, lang)?
        } else {
            match (vk, shift, self.detector.layouts.is_latin(lang)) {
                (b'1', true, _) => '!',
                (b'7', true, false) => '?',
                _ => return None,
            }
        };
        matches!(c, ',' | '.' | '!' | '?').then_some(c)
    }

    /// "привет ," -> "привет,": a mark typed right after the space(s) that followed a word.
    /// Only when the key is no letter in any layout (in English «,» and «.» are Russian «б»
    /// and «ю», the start of the next word typed in the wrong layout).
    fn space_before_punctuation(&mut self, vk: u8, shift: bool, lang: Lang, auto_allowed: bool) -> Option<Output> {
        if !self.settings.fix_space || !auto_allowed {
            return None;
        }
        let spaces = self.word.as_ref().filter(|w| w.trailing > 0)?.trailing;
        let c = self.punctuation(vk, shift, lang)?;
        let layouts = &self.detector.layouts;
        let letter_somewhere = layouts.is_word_key(vk)
            && layouts
                .all()
                .any(|l| layouts.render_key(KeyStroke { vk, shift, caps: false }, l).is_some_and(char::is_alphabetic));
        if letter_somewhere {
            return None;
        }
        self.word = None;
        self.sentence_start = SENTENCE_END.contains(&c);
        Some(Output { swallow: true, actions: vec![Action::Backspace(spaces), Action::Type(c.to_string())] })
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
        }
        let word = layouts.render(&keys, w.typed_lang).to_lowercase();
        if !self.undone.insert(word.clone()) {
            self.detector.never_switch.insert(word.clone());
            actions.push(Action::AddException(word));
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
        let to = if w.auto {
            // Undo an automatic fix.
            w.typed_lang
        } else {
            if w.cycle.is_empty() {
                w.cycle.push(w.typed_lang);
                w.cycle.extend(self.detector.ranked_alternatives(&w.keys, w.typed_lang));
            }
            let i = w.cycle.iter().position(|&l| l == from).unwrap_or(0);
            w.cycle[(i + 1) % w.cycle.len()]
        };
        if to == from {
            return vec![];
        }
        let n = self.detector.layouts.render(&w.keys, from).chars().count() + w.trailing;
        let mut text = self.detector.layouts.render(&w.keys, to);
        text.push_str(&" ".repeat(w.trailing));
        let mut actions = vec![Action::Backspace(n), Action::Type(text), Action::SwitchLayout(to)];
        let reverting = to == w.typed_lang;
        let word = self.detector.layouts.render(&w.keys, w.typed_lang).to_lowercase();
        if w.auto && reverting {
            // The user reverted an automatic correction: never touch this word again.
            if self.detector.always_switch.remove(&word).is_some() {
                actions.push(Action::Forget(word.clone()));
            }
            self.detector.never_switch.insert(word.clone());
            actions.push(Action::AddException(word));
            w.auto = false;
        } else if reverting {
            // Fixed by hand and put back: nothing to learn.
            w.learn_pending = false;
            if w.learned && self.detector.always_switch.remove(&word).is_some() {
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
                let code = self.detector.layouts.code(to).to_string();
                self.detector.always_switch.insert(word.clone(), Some(code.clone()));
                actions.push(Action::Learn(word, code));
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
        assert_eq!(o.actions.last(), Some(&Action::Learn("here".into(), "RU".into())));
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
        assert!(!o.actions.iter().any(|a| matches!(a, Action::Learn(..))));
        let o = space(&mut e, Lang::Ru, &mut t);
        assert!(!o.swallow);
        assert_eq!(o.actions, vec![Action::Learn("here".into(), "RU".into())]);

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
        assert!(!o.actions.iter().any(|a| matches!(a, Action::Learn(..))));
    }

    #[test]
    fn three_layouts_manual_cycle() {
        let mut d = Detector::new();
        d.layouts = crate::layout::en_ru_ua();
        let mut e = Engine::new(d, Settings { learn_manual: false, ..Settings::default() });
        let mut t = 0;
        // "here": a real English word, kept; Pause cycles RU «руку» (UA gives the same text,
        // so it is skipped) and back to English
        typ(&mut e, "here", Lang::En, &mut t);
        assert!(!space(&mut e, Lang::En, &mut t).swallow);
        let o = e.on_key(crate::config::VK_PAUSE, true, t + 10, Some(Lang::En), true);
        assert_eq!(o.actions[1], Action::Type("руку ".into()));
        let o = e.on_key(crate::config::VK_PAUSE, true, t + 20, Some(Lang::Ru), true);
        assert_eq!(o.actions[1], Action::Type("here ".into()));
        // "ghbdsn" is fixed to Ukrainian, and Pause undoes it
        t += 2000;
        typ(&mut e, "ghbdsn", Lang::En, &mut t);
        let o = space(&mut e, Lang::En, &mut t);
        assert_eq!(o.actions[1], Action::Type("привіт".into()));
        assert_eq!(o.actions[3], Action::SwitchLayout(Lang(2)));
    }

    fn enter(e: &mut Engine, lang: Lang, t: &mut u64) -> Output {
        *t += 50;
        e.on_key(VK_RETURN, true, *t, Some(lang), true)
    }

    #[test]
    fn capital_letter_at_sentence_start() {
        let mut e = Engine::new(Detector::new(), Settings::default());
        let mut t = 0;
        // nothing known before the first word: left alone
        typ(&mut e, "привет.", Lang::Ru, &mut t);
        assert!(!space(&mut e, Lang::Ru, &mut t).swallow);
        typ(&mut e, "как", Lang::Ru, &mut t);
        let o = space(&mut e, Lang::Ru, &mut t);
        assert_eq!(o.actions, vec![Action::Backspace(3), Action::Type("Как".into()), Action::Tap(VK_SPACE)]);
        // the middle of the sentence stays small
        typ(&mut e, "дела", Lang::Ru, &mut t);
        assert!(!space(&mut e, Lang::Ru, &mut t).swallow);
        // Backspace right away undoes the capital
        typ(&mut e, "дела.", Lang::Ru, &mut t);
        space(&mut e, Lang::Ru, &mut t);
        typ(&mut e, "ну", Lang::Ru, &mut t);
        assert_eq!(space(&mut e, Lang::Ru, &mut t).actions[1], Action::Type("Ну".into()));
        let o = e.on_key(VK_BACK, true, t + 10, Some(Lang::Ru), true);
        assert_eq!(o.actions[1], Action::Type("ну".into()));
        // after Enter, together with a layout fix
        e.reset();
        enter(&mut e, Lang::En, &mut t);
        typ(&mut e, "ghbdtn", Lang::En, &mut t);
        assert_eq!(space(&mut e, Lang::En, &mut t).actions[1], Action::Type("Привет".into()));
        // iPhone, links and words that are capitalised already are left alone
        // (after a pause: right after a fix the engine waits for the layout switch)
        t += 2000;
        for w in ["iPhone", "github.com", "Hello"] {
            enter(&mut e, Lang::En, &mut t);
            typ(&mut e, w, Lang::En, &mut t);
            let o = space(&mut e, Lang::En, &mut t);
            assert!(!o.swallow, "{w} {:?}", o.actions);
        }
        // option off
        let mut e = Engine::new(Detector::new(), Settings { capitalize: false, ..Settings::default() });
        enter(&mut e, Lang::Ru, &mut t);
        typ(&mut e, "как", Lang::Ru, &mut t);
        assert!(!space(&mut e, Lang::Ru, &mut t).swallow);
    }

    #[test]
    fn abbreviations_do_not_end_sentences() {
        for w in ["дела.", "ура!", "правда?", "ну...", "Hello."] {
            assert!(ends_sentence(w), "{w}");
        }
        for w in ["т.", "д.", "г.", "т.е.", "ул.", "стр.", "e.g.", "etc.", "Mr.", "привет", "так,"] {
            assert!(!ends_sentence(w), "{w}");
        }
        let mut e = Engine::new(Detector::new(), Settings::default());
        let mut t = 0;
        typ(&mut e, "т.е.", Lang::Ru, &mut t);
        space(&mut e, Lang::Ru, &mut t);
        typ(&mut e, "дом", Lang::Ru, &mut t);
        assert!(!space(&mut e, Lang::Ru, &mut t).swallow);
    }

    #[test]
    fn english_i() {
        let mut e = Engine::new(Detector::new(), Settings::default());
        let mut t = 0;
        typ(&mut e, "and", Lang::En, &mut t);
        space(&mut e, Lang::En, &mut t);
        for (w, want) in [("i", "I"), ("i'm", "I'm")] {
            typ(&mut e, w, Lang::En, &mut t);
            assert_eq!(space(&mut e, Lang::En, &mut t).actions[1], Action::Type(want.into()));
        }
    }

    #[test]
    fn space_before_punctuation() {
        let mut e = Engine::new(Detector::new(), Settings::default());
        let mut t = 0;
        typ(&mut e, "привет", Lang::Ru, &mut t);
        space(&mut e, Lang::Ru, &mut t);
        let comma = keys_for_text(",", Lang::Ru).unwrap()[0];
        e.on_key(VK_LSHIFT, true, t, Some(Lang::Ru), true);
        let o = e.on_key(comma.vk, true, t, Some(Lang::Ru), true);
        e.on_key(VK_LSHIFT, false, t, Some(Lang::Ru), true);
        assert!(o.swallow);
        assert_eq!(o.actions, vec![Action::Backspace(1), Action::Type(",".into())]);
        // "!" on the digit row
        typ(&mut e, "ура", Lang::Ru, &mut t);
        space(&mut e, Lang::Ru, &mut t);
        e.on_key(VK_LSHIFT, true, t, Some(Lang::Ru), true);
        let o = e.on_key(b'1', true, t, Some(Lang::Ru), true);
        assert_eq!(o.actions, vec![Action::Backspace(1), Action::Type("!".into())]);
        e.on_key(VK_LSHIFT, false, t, Some(Lang::Ru), true);
        // English "," is the Russian «б»: may start the next word, left alone
        typ(&mut e, "hello", Lang::En, &mut t);
        space(&mut e, Lang::En, &mut t);
        assert!(!e.on_key(0xBC, true, t, Some(Lang::En), true).swallow);
        // smileys: " :)" stays
        typ(&mut e, "ок", Lang::Ru, &mut t);
        space(&mut e, Lang::Ru, &mut t);
        e.on_key(VK_LSHIFT, true, t, Some(Lang::Ru), true);
        assert!(!e.on_key(b'6', true, t, Some(Lang::Ru), true).swallow);
        e.on_key(VK_LSHIFT, false, t, Some(Lang::Ru), true);
    }

    #[test]
    fn modules_turn_off() {
        let mut t = 0;
        // Layout module off: no automatic fix, hotkeys do nothing
        let mut e = Engine::new(Detector::new(), Settings { module_layout: false, ..Settings::default() });
        typ(&mut e, "ghbdtn", Lang::En, &mut t);
        assert!(!space(&mut e, Lang::En, &mut t).swallow);
        let o = e.on_key(crate::config::VK_PAUSE, true, t + 10, Some(Lang::En), true);
        assert!(!o.swallow && o.actions.is_empty());
        // Text module off: no capital letters
        let mut e = Engine::new(Detector::new(), Settings { module_text: false, ..Settings::default() });
        enter(&mut e, Lang::Ru, &mut t);
        typ(&mut e, "как", Lang::Ru, &mut t);
        assert!(!space(&mut e, Lang::Ru, &mut t).swallow);
        // Spelling module is off by default
        let mut e = Engine::new(Detector::new(), Settings::default());
        typ(&mut e, "првиет", Lang::Ru, &mut t);
        assert!(!space(&mut e, Lang::Ru, &mut t).swallow);
    }

    #[test]
    fn spelling_module() {
        let mut e = Engine::new(Detector::new(), Settings { module_spelling: true, ..Settings::default() });
        let mut t = 0;
        typ(&mut e, "првиет", Lang::Ru, &mut t);
        let o = space(&mut e, Lang::Ru, &mut t);
        assert_eq!(o.actions, vec![Action::Backspace(6), Action::Type("привет".into()), Action::Tap(VK_SPACE)]);
        // undo with Backspace; twice -> left alone for good
        let o = e.on_key(VK_BACK, true, t + 10, Some(Lang::Ru), true);
        assert_eq!(o.actions[1], Action::Type("првиет".into()));
        t += 100;
        e.reset();
        typ(&mut e, "првиет", Lang::Ru, &mut t);
        assert!(space(&mut e, Lang::Ru, &mut t).swallow);
        let o = e.on_key(VK_BACK, true, t + 10, Some(Lang::Ru), true);
        assert_eq!(o.actions.last(), Some(&Action::AddException("првиет".into())));
        e.reset();
        typ(&mut e, "првиет", Lang::Ru, &mut t);
        assert!(!space(&mut e, Lang::Ru, &mut t).swallow);
    }

    #[test]
    fn typo_in_wrong_layout() {
        // «првиет» typed with English keys: the detector does not know it, spelling does.
        let mut t = 0;
        let mut e = Engine::new(Detector::new(), Settings { module_spelling: true, ..Settings::default() });
        typ(&mut e, "ghdbtn", Lang::En, &mut t);
        let o = space(&mut e, Lang::En, &mut t);
        assert_eq!(o.actions[..3], [Action::Backspace(6), Action::Type("привет".into()), Action::Tap(VK_SPACE)]);
        assert!(o.actions.contains(&Action::SwitchLayout(Lang::Ru)));
        // Backspace puts it back as typed
        let o = e.on_key(VK_BACK, true, t + 10, Some(Lang::Ru), true);
        assert_eq!(o.actions[1], Action::Type("ghdbtn".into()));
        // An English typo stays English (the spelling module fixes it there)
        e.reset();
        typ(&mut e, "freind", Lang::En, &mut t);
        let o = space(&mut e, Lang::En, &mut t);
        assert_eq!(o.actions[1], Action::Type("friend".into()));
        assert!(!o.actions.iter().any(|a| matches!(a, Action::SwitchLayout(_))));
        // and the other way round: «freind» typed with Russian keys
        e.reset();
        typ(&mut e, "акуштв", Lang::Ru, &mut t);
        let o = space(&mut e, Lang::Ru, &mut t);
        assert_eq!(o.actions[1], Action::Type("friend".into()));
        assert!(o.actions.contains(&Action::SwitchLayout(Lang::En)));
        // Without the spelling module: left as typed
        let mut e = Engine::new(Detector::new(), Settings::default());
        typ(&mut e, "ghdbtn", Lang::En, &mut t);
        assert!(!space(&mut e, Lang::En, &mut t).swallow);
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
        use crate::layout::KeyMap;
        // A Ukrainian layout with і on S: typing "love" there must still become English.
        let mut d = Detector::new();
        let mut ru = KeyMap::builtin(Lang::Ru);
        ru.set(b'S', Some('і'), Some('І'), true);
        d.layouts = crate::layout::Layouts::new(vec![
            crate::layout::Layout::new(KeyMap::builtin(Lang::En), vec![crate::layout::Language::En], "EN"),
            crate::layout::Layout::new(ru, vec![crate::layout::Language::Uk, crate::layout::Language::Ru], "UA"),
        ]);
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
