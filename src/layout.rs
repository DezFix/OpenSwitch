// SPDX-License-Identifier: GPL-3.0-or-later
//! Keyboard layout tables keyed by Windows virtual-key codes: built-in US QWERTY and Russian
//! ЙЦУКЕН, or maps read from the layouts installed in Windows. Platform independent so it can
//! be unit-tested anywhere.

/// Which side of the pair a layout is on. `Ru` means "the Cyrillic layout": Russian, Ukrainian
/// or Belarusian (word detection uses the Russian dictionary for all of them for now).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    En,
    Ru,
}

impl Lang {
    pub fn other(self) -> Lang {
        match self {
            Lang::En => Lang::Ru,
            Lang::Ru => Lang::En,
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "EN",
            Lang::Ru => "RU",
        }
    }
}

/// One physical key press that belongs to a word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyStroke {
    pub vk: u8,
    pub shift: bool,
    pub caps: bool,
}

struct KeyDef {
    vk: u8,
    en: [char; 2], // [normal, shift]
    ru: [char; 2],
    en_letter: bool, // affected by CapsLock in EN
    ru_letter: bool, // affected by CapsLock in RU
}

const fn k(vk: u8, en: [char; 2], ru: [char; 2], en_letter: bool, ru_letter: bool) -> KeyDef {
    KeyDef { vk, en, ru, en_letter, ru_letter }
}

#[rustfmt::skip]
static KEYS: &[KeyDef] = &[
    k(b'Q', ['q','Q'], ['й','Й'], true, true),
    k(b'W', ['w','W'], ['ц','Ц'], true, true),
    k(b'E', ['e','E'], ['у','У'], true, true),
    k(b'R', ['r','R'], ['к','К'], true, true),
    k(b'T', ['t','T'], ['е','Е'], true, true),
    k(b'Y', ['y','Y'], ['н','Н'], true, true),
    k(b'U', ['u','U'], ['г','Г'], true, true),
    k(b'I', ['i','I'], ['ш','Ш'], true, true),
    k(b'O', ['o','O'], ['щ','Щ'], true, true),
    k(b'P', ['p','P'], ['з','З'], true, true),
    k(0xDB, ['[','{'], ['х','Х'], false, true), // VK_OEM_4
    k(0xDD, [']','}'], ['ъ','Ъ'], false, true), // VK_OEM_6
    k(b'A', ['a','A'], ['ф','Ф'], true, true),
    k(b'S', ['s','S'], ['ы','Ы'], true, true),
    k(b'D', ['d','D'], ['в','В'], true, true),
    k(b'F', ['f','F'], ['а','А'], true, true),
    k(b'G', ['g','G'], ['п','П'], true, true),
    k(b'H', ['h','H'], ['р','Р'], true, true),
    k(b'J', ['j','J'], ['о','О'], true, true),
    k(b'K', ['k','K'], ['л','Л'], true, true),
    k(b'L', ['l','L'], ['д','Д'], true, true),
    k(0xBA, [';',':'], ['ж','Ж'], false, true), // VK_OEM_1
    k(0xDE, ['\'','"'], ['э','Э'], false, true), // VK_OEM_7
    k(b'Z', ['z','Z'], ['я','Я'], true, true),
    k(b'X', ['x','X'], ['ч','Ч'], true, true),
    k(b'C', ['c','C'], ['с','С'], true, true),
    k(b'V', ['v','V'], ['м','М'], true, true),
    k(b'B', ['b','B'], ['и','И'], true, true),
    k(b'N', ['n','N'], ['т','Т'], true, true),
    k(b'M', ['m','M'], ['ь','Ь'], true, true),
    k(0xBC, [',','<'], ['б','Б'], false, true), // VK_OEM_COMMA
    k(0xBE, ['.','>'], ['ю','Ю'], false, true), // VK_OEM_PERIOD
    k(0xBF, ['/','?'], ['.',','], false, false), // VK_OEM_2
    k(0xC0, ['`','~'], ['ё','Ё'], false, true), // VK_OEM_3
    k(0xDC, ['\\','|'], ['\\','/'], false, false), // VK_OEM_5
    k(0xBD, ['-','_'], ['-','_'], false, false), // VK_OEM_MINUS
];

fn find(vk: u8) -> Option<&'static KeyDef> {
    KEYS.iter().find(|d| d.vk == vk)
}

/// Characters produced by each virtual key in one keyboard layout: `[normal, shift]`, plus
/// whether CapsLock acts on the key like Shift (true for letters).
///
/// The built-in maps are US QWERTY and Russian ЙЦУКЕН. On Windows the maps are read from the
/// layouts actually installed (see `sys::read_keymap`), so customised layouts — e.g. a
/// Ukrainian layout with Russian letters added — are converted correctly.
#[derive(Clone)]
pub struct KeyMap {
    chars: [[Option<char>; 2]; 256],
    caps: [bool; 256],
}

impl KeyMap {
    pub fn empty() -> KeyMap {
        KeyMap { chars: [[None; 2]; 256], caps: [false; 256] }
    }

    /// The built-in table for `lang` (US QWERTY / Russian ЙЦУКЕН).
    pub fn builtin(lang: Lang) -> KeyMap {
        let mut m = KeyMap::empty();
        for d in KEYS {
            let (pair, letter) = match lang {
                Lang::En => (d.en, d.en_letter),
                Lang::Ru => (d.ru, d.ru_letter),
            };
            m.set(d.vk, Some(pair[0]), Some(pair[1]), letter);
        }
        m
    }

    pub fn set(&mut self, vk: u8, normal: Option<char>, shifted: Option<char>, caps_like_shift: bool) {
        self.chars[vk as usize] = [normal, shifted];
        self.caps[vk as usize] = caps_like_shift;
    }

    /// Number of keys that produce a letter (used to sanity-check maps read from the system).
    pub fn letter_count(&self) -> usize {
        self.chars.iter().filter(|p| p[0].is_some_and(char::is_alphabetic)).count()
    }

    pub fn render_key(&self, ks: KeyStroke) -> Option<char> {
        let upper = if self.caps[ks.vk as usize] { ks.shift ^ ks.caps } else { ks.shift };
        self.chars[ks.vk as usize][upper as usize]
    }

    /// Which key + shift produces `c` (ignores CapsLock). Word keys are preferred over others.
    fn key_for_char(&self, c: char) -> Option<(u8, bool)> {
        let mut fallback = None;
        for vk in 0..=255u8 {
            for shift in [false, true] {
                if self.chars[vk as usize][shift as usize] == Some(c) {
                    if find(vk).is_some() {
                        return Some((vk, shift));
                    }
                    fallback.get_or_insert((vk, shift));
                }
            }
        }
        fallback
    }

    fn is_letter_key(&self, vk: u8) -> bool {
        self.chars[vk as usize][0].is_some_and(char::is_alphabetic)
    }

    /// Does CapsLock change this key (i.e. it is a letter in this layout)?
    pub fn caps_sensitive(&self, vk: u8) -> bool {
        self.caps[vk as usize]
    }
}

/// The pair of layouts OpenSwitch converts between.
/// `Lang::Ru` stands for "the Cyrillic layout" (Russian, Ukrainian or Belarusian).
#[derive(Clone)]
pub struct Layouts {
    pub en: KeyMap,
    pub ru: KeyMap,
}

impl Default for Layouts {
    fn default() -> Self {
        Layouts { en: KeyMap::builtin(Lang::En), ru: KeyMap::builtin(Lang::Ru) }
    }
}

impl Layouts {
    pub fn map(&self, lang: Lang) -> &KeyMap {
        match lang {
            Lang::En => &self.en,
            Lang::Ru => &self.ru,
        }
    }

    /// Is this virtual key part of a "word": a letter in one of the two layouts, or one of the
    /// punctuation keys that are letters in ЙЦУКЕН (`[ ] ; ' , . \``) or word punctuation (`-`)?
    pub fn is_word_key(&self, vk: u8) -> bool {
        find(vk).is_some() || self.en.is_letter_key(vk) || self.ru.is_letter_key(vk)
    }

    pub fn render_key(&self, ks: KeyStroke, lang: Lang) -> Option<char> {
        self.map(lang).render_key(ks)
    }

    pub fn render(&self, keys: &[KeyStroke], lang: Lang) -> String {
        let m = self.map(lang);
        keys.iter().filter_map(|&k| m.render_key(k)).collect()
    }

    /// Convert text that was typed in layout `from` as if it had been typed in `to`.
    /// Characters with no mapping (digits, spaces, ...) are kept as is.
    pub fn convert_text(&self, text: &str, from: Lang, to: Lang) -> String {
        let (src, dst) = (self.map(from), self.map(to));
        text.chars()
            .map(|c| match src.key_for_char(c) {
                Some((vk, shift)) => dst.chars[vk as usize][shift as usize].unwrap_or(c),
                None => c,
            })
            .collect()
    }

    /// Convert a selection: detects its layout and flips it to the other one.
    pub fn flip_text(&self, text: &str) -> String {
        let from = guess_text_lang(text);
        self.convert_text(text, from, from.other())
    }

    /// Turn text into keystrokes in `lang` (used by tests and tools).
    pub fn keys_for_text(&self, text: &str, lang: Lang) -> Option<Vec<KeyStroke>> {
        let m = self.map(lang);
        text.chars().map(|c| m.key_for_char(c).map(|(vk, shift)| KeyStroke { vk, shift, caps: false })).collect()
    }
}

fn default_layouts() -> &'static Layouts {
    static L: std::sync::OnceLock<Layouts> = std::sync::OnceLock::new();
    L.get_or_init(Layouts::default)
}

/// Built-in layouts (US / ЙЦУКЕН): is this key part of a word?
pub fn is_word_key(vk: u8) -> bool {
    default_layouts().is_word_key(vk)
}

/// Built-in layouts: character of one key.
pub fn render_key(ks: KeyStroke, lang: Lang) -> Option<char> {
    default_layouts().render_key(ks, lang)
}

/// Built-in layouts: render a word.
pub fn render(keys: &[KeyStroke], lang: Lang) -> String {
    default_layouts().render(keys, lang)
}

/// Built-in layouts: convert text typed in `from` as if typed in `to`.
pub fn convert_text(text: &str, from: Lang, to: Lang) -> String {
    default_layouts().convert_text(text, from, to)
}

/// Guess which layout a piece of text was typed in (by counting Cyrillic vs Latin letters).
pub fn guess_text_lang(text: &str) -> Lang {
    let cyr = text.chars().filter(|c| ('\u{0400}'..='\u{04FF}').contains(c) && c.is_alphabetic()).count();
    let lat = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if cyr > lat {
        Lang::Ru
    } else {
        Lang::En
    }
}

/// Built-in layouts: flip a selection to the other layout.
pub fn flip_text(text: &str) -> String {
    default_layouts().flip_text(text)
}

/// Built-in layouts: keystrokes for a text (tests and tools).
pub fn keys_for_text(text: &str, lang: Lang) -> Option<Vec<KeyStroke>> {
    default_layouts().keys_for_text(text, lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        assert_eq!(convert_text("ghbdtn", Lang::En, Lang::Ru), "привет");
        assert_eq!(convert_text("руддщ", Lang::Ru, Lang::En), "hello");
        assert_eq!(convert_text("Ghbdtn? rfr ltkf", Lang::En, Lang::Ru), "Привет, как дела");
        assert_eq!(flip_text("j;bl'v"), "ожидэм");
        assert_eq!(flip_text("Yfcnz"), "Настя");
    }

    #[test]
    fn caps_lock() {
        let ks = KeyStroke { vk: 0xBA, shift: false, caps: true };
        assert_eq!(render_key(ks, Lang::En), Some(';'));
        assert_eq!(render_key(ks, Lang::Ru), Some('Ж'));
        let ks = KeyStroke { vk: b'A', shift: true, caps: true };
        assert_eq!(render_key(ks, Lang::En), Some('a'));
    }

    /// A Ukrainian layout with Russian letters added (like the one reported by a user):
    /// і on S, ї on ], є on ', ґ on `; ы, э, ъ, ё reachable elsewhere.
    fn ukrainian_hybrid() -> Layouts {
        let mut ru = KeyMap::builtin(Lang::Ru);
        ru.set(b'S', Some('і'), Some('І'), true);
        ru.set(0xDD, Some('ї'), Some('Ї'), true);
        ru.set(0xDE, Some('є'), Some('Є'), true);
        ru.set(0xC0, Some('ґ'), Some('Ґ'), true);
        ru.set(0xE2, Some('ы'), Some('Ы'), true); // VK_OEM_102
        Layouts { en: KeyMap::builtin(Lang::En), ru }
    }

    #[test]
    fn custom_layout() {
        let l = ukrainian_hybrid();
        assert_eq!(l.convert_text("ghbdsn", Lang::En, Lang::Ru), "привіт");
        assert_eq!(l.convert_text("дщму", Lang::Ru, Lang::En), "love");
        assert_eq!(l.flip_text("Привіт"), "Ghbdsn");
        assert!(l.is_word_key(0xE2));
        assert!(!is_word_key(0xE2));
        let keys = l.keys_for_text("їжак", Lang::Ru).unwrap();
        assert_eq!(l.render(&keys, Lang::En), "];fr");
        assert_eq!(guess_text_lang("Їжак і ґанок"), Lang::Ru);
    }
}
