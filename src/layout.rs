//! Keyboard layout tables: US QWERTY <-> Russian ЙЦУКЕН, keyed by Windows virtual-key codes.
//! Platform independent so it can be unit-tested anywhere.

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

/// Is this virtual key part of a "word" (letters in at least one layout, or word punctuation)?
pub fn is_word_key(vk: u8) -> bool {
    find(vk).is_some()
}

pub fn render_key(ks: KeyStroke, lang: Lang) -> Option<char> {
    let d = find(ks.vk)?;
    let (pair, letter) = match lang {
        Lang::En => (d.en, d.en_letter),
        Lang::Ru => (d.ru, d.ru_letter),
    };
    let upper = if letter { ks.shift ^ ks.caps } else { ks.shift };
    Some(pair[upper as usize])
}

pub fn render(keys: &[KeyStroke], lang: Lang) -> String {
    keys.iter().filter_map(|&k| render_key(k, lang)).collect()
}

/// Find which key + shift produces `c` in `lang` (ignores CapsLock).
fn key_for_char(c: char, lang: Lang) -> Option<(&'static KeyDef, bool)> {
    for d in KEYS {
        let pair = match lang {
            Lang::En => d.en,
            Lang::Ru => d.ru,
        };
        if pair[0] == c {
            return Some((d, false));
        }
        if pair[1] == c {
            return Some((d, true));
        }
    }
    None
}

/// Convert text that was typed in layout `from` as if it had been typed in `to`.
/// Characters with no mapping (digits, spaces, ...) are kept as is.
pub fn convert_text(text: &str, from: Lang, to: Lang) -> String {
    text.chars()
        .map(|c| match key_for_char(c, from) {
            Some((d, shift)) => {
                let pair = match to {
                    Lang::En => d.en,
                    Lang::Ru => d.ru,
                };
                pair[shift as usize]
            }
            None => c,
        })
        .collect()
}

/// Guess which layout a piece of text was typed in (by counting Cyrillic vs Latin letters).
pub fn guess_text_lang(text: &str) -> Lang {
    let ru = text.chars().filter(|c| ('а'..='я').contains(&c.to_lowercase().next().unwrap_or(*c)) || *c == 'ё' || *c == 'Ё').count();
    let en = text.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if ru > en { Lang::Ru } else { Lang::En }
}

/// Convert a selection: detects its layout and flips it to the other one.
pub fn flip_text(text: &str) -> String {
    let from = guess_text_lang(text);
    convert_text(text, from, from.other())
}

/// Turn text into keystrokes in `lang` (used by tests and tools).
pub fn keys_for_text(text: &str, lang: Lang) -> Option<Vec<KeyStroke>> {
    text.chars()
        .map(|c| key_for_char(c, lang).map(|(d, shift)| KeyStroke { vk: d.vk, shift, caps: false }))
        .collect()
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
}
