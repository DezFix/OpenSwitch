// SPDX-License-Identifier: GPL-3.0-or-later
//! Keyboard layout tables keyed by Windows virtual-key codes: built-in US QWERTY and Russian
//! ЙЦУКЕН, or maps read from the layouts installed in Windows. Platform independent so it can
//! be unit-tested anywhere.

/// A keyboard layout: index into `Layouts::list`.
///
/// With the built-in layouts (and in tests) `Lang::En` is US QWERTY and `Lang::Ru` is Russian
/// ЙЦУКЕН. On Windows the list holds every installed layout OpenSwitch has a dictionary for
/// (English, Russian, Ukrainian, Belarusian), so any number of layouts can take part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Lang(pub u8);

#[allow(non_upper_case_globals)]
impl Lang {
    /// First built-in layout: US QWERTY.
    pub const En: Lang = Lang(0);
    /// Second built-in layout: Russian ЙЦУКЕН.
    pub const Ru: Lang = Lang(1);

    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// A language with a dictionary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Language {
    En,
    Ru,
    Uk,
}

impl Language {
    pub fn is_latin(self) -> bool {
        self == Language::En
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

    /// The built-in table: US QWERTY for `Lang::En`, Russian ЙЦУКЕН for anything else.
    pub fn builtin(lang: Lang) -> KeyMap {
        let mut m = KeyMap::empty();
        for d in KEYS {
            let (pair, letter) = if lang == Lang::En { (d.en, d.en_letter) } else { (d.ru, d.ru_letter) };
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

/// One layout taking part in switching.
#[derive(Clone)]
pub struct Layout {
    pub map: KeyMap,
    /// Languages a word typed in this layout can be in; the first is the main one.
    /// A Ukrainian layout also counts as Russian when no Russian layout is installed.
    pub languages: Vec<Language>,
    /// Short name for the tray icon and the settings (EN, RU, UA, BY).
    pub code: String,
    /// May words be switched *to* this layout automatically?
    pub target: bool,
}

impl Layout {
    pub fn new(map: KeyMap, languages: Vec<Language>, code: &str) -> Layout {
        Layout { map, languages, code: code.to_string(), target: true }
    }
}

/// The layouts OpenSwitch converts between.
#[derive(Clone)]
pub struct Layouts {
    pub list: Vec<Layout>,
    /// When each layout was last typed in (any clock; larger = more recent). Used to choose
    /// between layouts that give the same text, e.g. Russian and Ukrainian for «привет».
    pub recency: Vec<u64>,
}

impl Default for Layouts {
    fn default() -> Self {
        Layouts::pair(KeyMap::builtin(Lang::En), KeyMap::builtin(Lang::Ru))
    }
}

impl Layouts {
    pub fn new(list: Vec<Layout>) -> Layouts {
        let recency = vec![0; list.len()];
        Layouts { list, recency }
    }

    /// English + Russian with the given maps.
    pub fn pair(en: KeyMap, ru: KeyMap) -> Layouts {
        Layouts::new(vec![Layout::new(en, vec![Language::En], "EN"), Layout::new(ru, vec![Language::Ru], "RU")])
    }

    pub fn len(&self) -> usize {
        self.list.len()
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn all(&self) -> impl Iterator<Item = Lang> {
        (0..self.list.len() as u8).map(Lang)
    }

    pub fn get(&self, lang: Lang) -> Option<&Layout> {
        self.list.get(lang.index())
    }

    pub fn map(&self, lang: Lang) -> &KeyMap {
        &self.list[lang.index()].map
    }

    pub fn languages(&self, lang: Lang) -> &[Language] {
        self.get(lang).map_or(&[], |l| &l.languages)
    }

    pub fn is_latin(&self, lang: Lang) -> bool {
        self.languages(lang).first().is_some_and(|l| l.is_latin())
    }

    pub fn code(&self, lang: Lang) -> &str {
        self.get(lang).map_or("??", |l| &l.code)
    }

    /// Remember that the user is typing in `lang` now.
    pub fn touch(&mut self, lang: Lang, now: u64) {
        if let Some(r) = self.recency.get_mut(lang.index()) {
            *r = now;
        }
    }

    /// The most recently used of `candidates` (the first one on a tie).
    pub fn preferred(&self, candidates: impl Iterator<Item = Lang>) -> Option<Lang> {
        let mut best: Option<Lang> = None;
        for c in candidates {
            let r = self.recency.get(c.index()).copied().unwrap_or(0);
            if best.is_none_or(|b| r > self.recency.get(b.index()).copied().unwrap_or(0)) {
                best = Some(c);
            }
        }
        best
    }

    /// Is this virtual key part of a "word": a letter in one of the layouts, or one of the
    /// punctuation keys that are letters in ЙЦУКЕН (`[ ] ; ' , . \``) or word punctuation (`-`)?
    pub fn is_word_key(&self, vk: u8) -> bool {
        find(vk).is_some() || self.list.iter().any(|l| l.map.is_letter_key(vk))
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

    /// Which layout a piece of text was typed in: Latin text -> an English layout, Cyrillic
    /// text -> the Cyrillic layout that has all its letters (the most recently used one).
    pub fn guess_text_lang(&self, text: &str) -> Lang {
        let cyr = text.chars().filter(|c| ('\u{0400}'..='\u{04FF}').contains(c) && c.is_alphabetic()).count();
        let lat = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
        let latin = cyr <= lat;
        let fits = |l: Lang| text.chars().all(|c| !c.is_alphabetic() || self.map(l).key_for_char(c).is_some());
        let side = || self.all().filter(move |&l| self.is_latin(l) == latin);
        self.preferred(side().filter(|&l| fits(l))).or_else(|| self.preferred(side())).unwrap_or(Lang::En)
    }

    /// The layout a selection typed in `from` is converted to: Latin -> the most recently used
    /// Cyrillic layout, Cyrillic -> the most recently used English one.
    pub fn flip_target(&self, from: Lang) -> Lang {
        let latin = self.is_latin(from);
        self.preferred(self.all().filter(|&l| l != from && self.is_latin(l) != latin))
            .or_else(|| self.all().find(|&l| l != from))
            .unwrap_or(from)
    }

    /// Convert a selection: detects its layout and converts it to the other side.
    pub fn flip_text(&self, text: &str) -> String {
        let from = self.guess_text_lang(text);
        self.convert_text(text, from, self.flip_target(from))
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

/// Built-in layouts: guess which layout a piece of text was typed in.
pub fn guess_text_lang(text: &str) -> Lang {
    default_layouts().guess_text_lang(text)
}

/// Built-in layouts: flip a selection to the other layout.
pub fn flip_text(text: &str) -> String {
    default_layouts().flip_text(text)
}

/// Standard Ukrainian layout: ЙЦУКЕН with і, ї, є on the keys of ы, ъ, э, ґ on `\` and the
/// apostrophe on the key of ё.
pub fn ukrainian_map() -> KeyMap {
    let mut m = KeyMap::builtin(Lang::Ru);
    m.set(b'S', Some('і'), Some('І'), true);
    m.set(0xDD, Some('ї'), Some('Ї'), true);
    m.set(0xDE, Some('є'), Some('Є'), true);
    m.set(0xC0, Some('\''), Some('₴'), false);
    m.set(0xDC, Some('ґ'), Some('Ґ'), true);
    m
}

/// English, Russian and Ukrainian (as `Lang(0)`, `Lang(1)`, `Lang(2)`): for tests and tools.
pub fn en_ru_ua() -> Layouts {
    Layouts::new(vec![
        Layout::new(KeyMap::builtin(Lang::En), vec![Language::En], "EN"),
        Layout::new(KeyMap::builtin(Lang::Ru), vec![Language::Ru], "RU"),
        Layout::new(ukrainian_map(), vec![Language::Uk], "UA"),
    ])
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
        Layouts::pair(KeyMap::builtin(Lang::En), ru)
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
