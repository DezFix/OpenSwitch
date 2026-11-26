// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings file (simple `key = value` INI-like format) and hotkey parsing.

use std::collections::HashSet;

use crate::i18n::{self, t};

pub const VK_PAUSE: u8 = 0x13;
pub const VK_CANCEL: u8 = 0x03; // what Ctrl+Pause produces

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    pub vk: u8,
}

impl Hotkey {
    pub const NONE: Hotkey = Hotkey { ctrl: false, alt: false, shift: false, win: false, vk: 0 };

    pub fn parse(s: &str) -> Option<Hotkey> {
        let s = s.trim();
        if s.is_empty() || s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("off") {
            return Some(Hotkey::NONE);
        }
        let mut hk = Hotkey::NONE;
        for part in s.split('+').map(|p| p.trim().to_ascii_lowercase()) {
            match part.as_str() {
                "ctrl" | "control" => hk.ctrl = true,
                "alt" => hk.alt = true,
                "shift" => hk.shift = true,
                "win" => hk.win = true,
                key => {
                    if hk.vk != 0 {
                        return None;
                    }
                    hk.vk = key_vk(key)?;
                }
            }
        }
        if hk.vk == 0 {
            None
        } else {
            Some(hk)
        }
    }

    pub fn matches(&self, vk: u8, ctrl: bool, alt: bool, shift: bool, win: bool) -> bool {
        if self.vk == 0 {
            return false;
        }
        let vk_ok = self.vk == vk || (self.vk == VK_PAUSE && self.ctrl && vk == VK_CANCEL);
        vk_ok && self.ctrl == ctrl && self.alt == alt && self.shift == shift && self.win == win
    }
}

impl std::fmt::Display for Hotkey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.vk == 0 {
            return f.write_str("none");
        }
        if self.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.alt {
            f.write_str("Alt+")?;
        }
        if self.shift {
            f.write_str("Shift+")?;
        }
        if self.win {
            f.write_str("Win+")?;
        }
        f.write_str(&key_name(self.vk))
    }
}

/// Keys offered in the settings UI (name, vk). vk 0 = no hotkey.
pub fn key_choices() -> Vec<(String, u8)> {
    let mut v = vec![
        (t("None").to_string(), 0),
        ("Pause".to_string(), VK_PAUSE),
        ("ScrollLock".to_string(), 0x91),
        ("Insert".to_string(), 0x2D),
        ("Menu".to_string(), 0x5D),
        ("PrintScreen".to_string(), 0x2C),
        ("Space".to_string(), 0x20),
    ];
    for n in 1..=12u8 {
        v.push((format!("F{n}"), 0x6F + n));
    }
    for c in b'A'..=b'Z' {
        v.push(((c as char).to_string(), c));
    }
    v
}

pub fn key_name(vk: u8) -> String {
    match vk {
        0 => "none".into(),
        VK_PAUSE => "Pause".into(),
        0x91 => "ScrollLock".into(),
        0x14 => "CapsLock".into(),
        0x2D => "Insert".into(),
        0x20 => "Space".into(),
        0x5D => "Menu".into(),
        0x2C => "PrintScreen".into(),
        v @ 0x70..=0x87 => format!("F{}", v - 0x6F),
        v => (v as char).to_string(),
    }
}

/// Can this key be the main key of a hotkey (i.e. saved to settings.ini and read back)?
pub fn is_hotkey_key(vk: u8) -> bool {
    vk != 0 && key_vk(&key_name(vk).to_ascii_lowercase()) == Some(vk)
}

/// Sensitivity presets shown in the settings window: (value, label key).
pub const SENSITIVITY_PRESETS: [(f32, &str); 3] = [(0.7, "Cautious"), (1.0, "Normal"), (1.4, "Bold")];

fn key_vk(name: &str) -> Option<u8> {
    let vk = match name {
        "pause" | "break" => VK_PAUSE,
        "scrolllock" | "scroll" => 0x91,
        "capslock" | "caps" => 0x14,
        "insert" | "ins" => 0x2D,
        "space" => 0x20,
        "apps" | "menu" => 0x5D,
        "printscreen" | "prtsc" => 0x2C,
        _ => {
            if let Some(n) = name.strip_prefix('f').and_then(|n| n.parse::<u8>().ok()) {
                if (1..=24).contains(&n) {
                    return Some(0x6F + n);
                }
                return None;
            }
            let b = name.as_bytes();
            if b.len() == 1 && (b[0].is_ascii_alphanumeric()) {
                return Some(b[0].to_ascii_uppercase());
            }
            return None;
        }
    };
    Some(vk)
}

#[derive(Clone, Debug)]
pub struct Settings {
    /// Module "Layout": automatic and manual layout fixes, hotkeys, double Shift.
    pub module_layout: bool,
    /// Module "Text": capital letters, CapsLock fix, space before punctuation.
    pub module_text: bool,
    /// Module "Spelling": typo fixes with the built-in dictionaries.
    pub module_spelling: bool,
    /// Start with administrator rights through the Task Scheduler task (set by the settings
    /// window together with the task itself).
    pub run_as_admin: bool,
    /// The first-run window was shown (or skipped).
    pub setup_done: bool,
    pub auto_switch: bool,
    pub single_letters: bool,
    pub switch_on_enter: bool,
    pub double_shift: bool,
    /// Backspace right after an automatic fix puts the word back as it was typed.
    pub undo_backspace: bool,
    /// Fix words typed with CapsLock on by accident (пРИВЕТ -> Привет) and turn CapsLock off.
    pub fix_caps: bool,
    /// Leave links, e-mail addresses, file names and camelCase identifiers alone.
    pub skip_code: bool,
    /// Do nothing while the user types in a password field.
    pub skip_passwords: bool,
    /// Be more careful with the first word after the user switched the layout themselves.
    pub careful_after_switch: bool,
    /// Remember words fixed by hand (Pause / Shift Shift) and fix them automatically next time.
    pub learn_manual: bool,
    /// Capital letter at the start of a sentence, English "i" -> "I".
    pub capitalize: bool,
    /// Remove the space typed before , . ! ? ("привет , как" -> "привет, как").
    pub fix_space: bool,
    pub sensitivity: f32,
    pub hotkey_convert_word: Hotkey,
    pub hotkey_convert_selection: Hotkey,
    pub hotkey_toggle_auto: Hotkey,
    /// UI language: `system` (follow Windows), `en`, `ru` or `uk`.
    pub language: String,
    /// Colours of the windows: `system` (follow Windows), `light` or `dark`.
    pub theme: String,
    /// Executables (lowercase file names) where auto switching is disabled.
    pub exclude: HashSet<String>,
    /// Layouts (codes like RU, UA) words are never switched *to* automatically.
    pub disabled_layouts: HashSet<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            module_layout: true,
            module_text: true,
            module_spelling: false,
            setup_done: false,
            run_as_admin: false,
            auto_switch: true,
            single_letters: true,
            switch_on_enter: true,
            double_shift: true,
            undo_backspace: true,
            fix_caps: true,
            skip_code: true,
            skip_passwords: true,
            careful_after_switch: true,
            learn_manual: true,
            capitalize: true,
            fix_space: true,
            sensitivity: 1.0,
            hotkey_convert_word: Hotkey { vk: VK_PAUSE, ..Hotkey::NONE },
            hotkey_convert_selection: Hotkey { vk: VK_PAUSE, shift: true, ..Hotkey::NONE },
            hotkey_toggle_auto: Hotkey { vk: VK_PAUSE, alt: true, ..Hotkey::NONE },
            language: "system".into(),
            theme: "system".into(),
            exclude: DEFAULT_EXCLUDE.iter().map(|s| s.to_string()).collect(),
            disabled_layouts: HashSet::new(),
        }
    }
}

const DEFAULT_EXCLUDE: &[&str] = &[
    "cmd.exe",
    "powershell.exe",
    "pwsh.exe",
    "windowsterminal.exe",
    "conhost.exe",
    "putty.exe",
    "mintty.exe",
    "keepass.exe",
    "keepassxc.exe",
];

/// Values of `theme`.
pub const THEMES: [&str; 3] = ["system", "light", "dark"];

pub const DEFAULT_FILE: &str = "\
# OpenSwitch - настройки. Удобнее менять в окне «Настройки» (меню значка в трее).\n# После ручной правки файла: меню в трее -> «Перечитать настройки».
# Значения true/false. Горячие клавиши: Pause, Shift+Pause, Alt+Pause, Ctrl+Shift+F12, ScrollLock, none ...

# Модули: что OpenSwitch делает. Каждый включается отдельно.
# Раскладка: исправление слов, набранных не в той раскладке, горячие клавиши
module_layout = true
# Текст: заглавные буквы, случайный CapsLock, пробел перед знаками
module_text = true
# Орфография: исправление опечаток по встроенным словарям
module_spelling = false

# Окно первого запуска уже показано
setup_done = false

# Запускаться с правами администратора (через задачу Планировщика «OpenSwitch»).
# Меняйте в окне настроек: там же создаётся и удаляется сама задача.
run_as_admin = false

# Автоматически исправлять слова, набранные не в той раскладке
auto_switch = true

# Исправлять однобуквенные слова (d -> в, z -> я, ш -> i)
single_letters = true

# Проверять слово также при нажатии Enter (иначе только пробел)
switch_on_enter = true

# Двойное нажатие Shift = исправить последнее слово (удобно на ноутбуках без Pause)
double_shift = true

# Backspace сразу после автоисправления возвращает слово как было набрано
undo_backspace = true

# Исправлять случайно нажатый CapsLock (пРИВЕТ -> Привет) и выключать его
fix_caps = true

# Не трогать ссылки, почту, имена файлов и идентификаторы вида camelCase
skip_code = true

# Ничего не делать в полях пароля (пароль не исправляется и не запоминается)
skip_passwords = true

# Осторожнее с первым словом после того, как вы сами переключили раскладку
careful_after_switch = true

# Запоминать слова, исправленные вручную (Pause), и дальше исправлять их сразу (always.txt)
learn_manual = true

# Заглавная буква в начале предложения (после . ! ? и Enter), английское i -> I
capitalize = true

# Убирать пробел перед знаком препинания: «привет , как» -> «привет, как»
fix_space = true

# Чувствительность автопереключения: 0.5 (осторожно) ... 2.0 (агрессивно)
sensitivity = 1.0

# Язык интерфейса: system (как в Windows), en, ru, uk
language = system

# Тема окон: system (как в Windows), light (светлая), dark (тёмная)
theme = system

# Исправить последнее слово (повторное нажатие — вернуть обратно)
hotkey_convert_word = Pause

# Исправить раскладку выделенного текста
hotkey_convert_selection = Shift+Pause

# Включить/выключить автопереключение
hotkey_toggle_auto = Alt+Pause

# Раскладки, на которые не переключать автоматически (коды через запятую: EN, RU, UA, BY)
disabled_layouts =

# Программы, в которых автопереключение не работает (ручное исправление работает).
# Имена exe через запятую.
exclude = cmd.exe, powershell.exe, pwsh.exe, windowsterminal.exe, conhost.exe, putty.exe, mintty.exe, keepass.exe, keepassxc.exe
";

fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" | "да" => Some(true),
        "false" | "no" | "off" | "0" | "нет" => Some(false),
        _ => None,
    }
}

impl Settings {
    /// Parse settings; unknown keys and bad values are reported in the returned warnings.
    pub fn parse(text: &str) -> (Settings, Vec<String>) {
        let mut s = Settings::default();
        let mut warn = Vec::new();
        for (n, line) in text.lines().enumerate() {
            let line = line.trim().trim_start_matches('\u{feff}');
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') || line.starts_with('[') {
                continue;
            }
            let Some((k, v)) = line.split_once('=') else {
                warn.push(i18n::f("line {}: no '='", &[&(n + 1)]));
                continue;
            };
            let (k, v) = (k.trim().to_ascii_lowercase(), v.trim());
            let ok = match k.as_str() {
                "module_layout" => parse_bool(v).map(|b| s.module_layout = b).is_some(),
                "module_text" => parse_bool(v).map(|b| s.module_text = b).is_some(),
                "module_spelling" => parse_bool(v).map(|b| s.module_spelling = b).is_some(),
                "setup_done" => parse_bool(v).map(|b| s.setup_done = b).is_some(),
                "run_as_admin" => parse_bool(v).map(|b| s.run_as_admin = b).is_some(),
                "auto_switch" => parse_bool(v).map(|b| s.auto_switch = b).is_some(),
                "single_letters" => parse_bool(v).map(|b| s.single_letters = b).is_some(),
                "switch_on_enter" => parse_bool(v).map(|b| s.switch_on_enter = b).is_some(),
                "double_shift" => parse_bool(v).map(|b| s.double_shift = b).is_some(),
                "undo_backspace" => parse_bool(v).map(|b| s.undo_backspace = b).is_some(),
                "fix_caps" => parse_bool(v).map(|b| s.fix_caps = b).is_some(),
                "skip_code" => parse_bool(v).map(|b| s.skip_code = b).is_some(),
                "skip_passwords" => parse_bool(v).map(|b| s.skip_passwords = b).is_some(),
                "careful_after_switch" => parse_bool(v).map(|b| s.careful_after_switch = b).is_some(),
                "learn_manual" => parse_bool(v).map(|b| s.learn_manual = b).is_some(),
                "capitalize" => parse_bool(v).map(|b| s.capitalize = b).is_some(),
                "fix_space" => parse_bool(v).map(|b| s.fix_space = b).is_some(),
                "sensitivity" => v.parse::<f32>().ok().filter(|x| *x > 0.0).map(|x| s.sensitivity = x).is_some(),
                "language" => {
                    let code = v.to_ascii_lowercase();
                    if code == "system" || i18n::Lang::from_code(&code).is_some() {
                        s.language = code;
                        true
                    } else {
                        false
                    }
                }
                "theme" => {
                    let v = v.to_ascii_lowercase();
                    if THEMES.contains(&v.as_str()) {
                        s.theme = v;
                        true
                    } else {
                        false
                    }
                }
                "hotkey_convert_word" => Hotkey::parse(v).map(|h| s.hotkey_convert_word = h).is_some(),
                "hotkey_convert_selection" => Hotkey::parse(v).map(|h| s.hotkey_convert_selection = h).is_some(),
                "hotkey_toggle_auto" => Hotkey::parse(v).map(|h| s.hotkey_toggle_auto = h).is_some(),
                "disabled_layouts" => {
                    s.disabled_layouts =
                        v.split([',', ';']).map(|x| x.trim().to_uppercase()).filter(|x| !x.is_empty()).collect();
                    true
                }
                "exclude" => {
                    s.exclude =
                        v.split([',', ';']).map(|x| x.trim().to_lowercase()).filter(|x| !x.is_empty()).collect();
                    true
                }
                _ => {
                    warn.push(i18n::f("line {}: unknown option '{}'", &[&(n + 1), &k]));
                    continue;
                }
            };
            if !ok {
                warn.push(i18n::f("line {}: bad value for '{}'", &[&(n + 1), &k]));
            }
        }
        (s, warn)
    }

    /// Serialise to the settings file format (with comments).
    pub fn to_text(&self) -> String {
        let b = |x: bool| if x { "true" } else { "false" };
        let mut ex: Vec<&String> = self.exclude.iter().collect();
        ex.sort();
        let ex: Vec<&str> = ex.iter().map(|s| s.as_str()).collect();
        let mut t = String::from(DEFAULT_FILE);
        for (k, v) in [
            ("module_layout", b(self.module_layout).to_string()),
            ("module_text", b(self.module_text).to_string()),
            ("module_spelling", b(self.module_spelling).to_string()),
            ("setup_done", b(self.setup_done).to_string()),
            ("run_as_admin", b(self.run_as_admin).to_string()),
            ("auto_switch", b(self.auto_switch).to_string()),
            ("single_letters", b(self.single_letters).to_string()),
            ("switch_on_enter", b(self.switch_on_enter).to_string()),
            ("double_shift", b(self.double_shift).to_string()),
            ("undo_backspace", b(self.undo_backspace).to_string()),
            ("fix_caps", b(self.fix_caps).to_string()),
            ("skip_code", b(self.skip_code).to_string()),
            ("skip_passwords", b(self.skip_passwords).to_string()),
            ("careful_after_switch", b(self.careful_after_switch).to_string()),
            ("learn_manual", b(self.learn_manual).to_string()),
            ("capitalize", b(self.capitalize).to_string()),
            ("fix_space", b(self.fix_space).to_string()),
            ("sensitivity", format!("{:.2}", self.sensitivity)),
            ("language", self.language.clone()),
            ("theme", self.theme.clone()),
            ("hotkey_convert_word", self.hotkey_convert_word.to_string()),
            ("hotkey_convert_selection", self.hotkey_convert_selection.to_string()),
            ("hotkey_toggle_auto", self.hotkey_toggle_auto.to_string()),
            ("exclude", ex.join(", ")),
            ("disabled_layouts", {
                let mut d: Vec<&str> = self.disabled_layouts.iter().map(|s| s.as_str()).collect();
                d.sort();
                d.join(", ")
            }),
        ] {
            t = Settings::set_in_text(&t, k, &v);
        }
        t
    }

    /// Replace the value of `key` in an existing settings file text (or append it).
    pub fn set_in_text(text: &str, key: &str, value: &str) -> String {
        let mut found = false;
        let mut out: Vec<String> = text
            .lines()
            .map(|l| {
                let t = l.trim();
                if !t.starts_with('#') {
                    if let Some((k, _)) = t.split_once('=') {
                        if k.trim().eq_ignore_ascii_case(key) {
                            found = true;
                            return format!("{key} = {value}");
                        }
                    }
                }
                l.to_string()
            })
            .collect();
        if !found {
            out.push(format!("{key} = {value}"));
        }
        out.join("\n") + "\n"
    }
}

/// Learned words (always.txt): `word` or `word CODE` per line (CODE = layout to convert to,
/// e.g. UA), `#` comments.
pub fn parse_always(text: &str) -> std::collections::HashMap<String, Option<String>> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let word = parts.next()?.to_lowercase();
            Some((word, parts.next().map(str::to_uppercase)))
        })
        .collect()
}

/// User exception list: one word per line, `#` comments.
pub fn parse_exceptions(text: &str) -> HashSet<String> {
    text.lines().map(|l| l.trim().to_lowercase()).filter(|l| !l.is_empty() && !l.starts_with('#')).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_setting() {
        let (s, w) = Settings::parse(DEFAULT_FILE);
        assert!(w.is_empty(), "{w:?}");
        assert_eq!(s.language, "system");
        let uk = Settings::set_in_text(DEFAULT_FILE, "language", "uk");
        assert_eq!(Settings::parse(&uk).0.language, "uk");
        let bad = Settings::set_in_text(DEFAULT_FILE, "language", "de");
        assert!(!Settings::parse(&bad).1.is_empty());
    }

    #[test]
    fn always_list() {
        let m = parse_always("# comment\nghbdtn\nGhbdsn ua\n\n");
        assert_eq!(m.get("ghbdtn"), Some(&None));
        assert_eq!(m.get("ghbdsn"), Some(&Some("UA".to_string())));
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn hotkeys() {
        assert_eq!(Hotkey::parse("Pause").unwrap().vk, VK_PAUSE);
        let h = Hotkey::parse("Ctrl+Shift+F12").unwrap();
        assert!(h.ctrl && h.shift && !h.alt && h.vk == 0x7B);
        assert!(Hotkey::parse("Ctrl+Pause").unwrap().matches(VK_CANCEL, true, false, false, false));
        assert_eq!(Hotkey::parse("none"), Some(Hotkey::NONE));
        assert!(Hotkey::parse("Ctrl+Foo").is_none());
    }

    #[test]
    fn hotkey_keys_round_trip() {
        for vk in [VK_PAUSE, 0x91, 0x14, 0x2D, 0x20, 0x5D, 0x2C, 0x70, 0x87, b'A', b'Z', b'0', b'9'] {
            assert!(is_hotkey_key(vk), "{vk:#x}");
        }
        // Delete, arrows, Enter, Esc have no name in settings.ini
        for vk in [0, 0x2E, 0x25, 0x0D, 0x1B, 0x10] {
            assert!(!is_hotkey_key(vk), "{vk:#x}");
        }
    }

    #[test]
    fn default_file_parses() {
        let (s, w) = Settings::parse(DEFAULT_FILE);
        assert!(w.is_empty(), "{w:?}");
        let d = Settings::default();
        assert_eq!(s.hotkey_convert_selection, d.hotkey_convert_selection);
        assert_eq!(s.exclude, d.exclude);
        let t = Settings::set_in_text(DEFAULT_FILE, "auto_switch", "false");
        assert!(!Settings::parse(&t).0.auto_switch);

        let m = Settings {
            auto_switch: false,
            skip_passwords: false,
            run_as_admin: true,
            theme: "dark".into(),
            sensitivity: 1.5,
            hotkey_convert_word: Hotkey::parse("Ctrl+Shift+F12").unwrap(),
            hotkey_toggle_auto: Hotkey::NONE,
            exclude: ["code.exe".to_string()].into_iter().collect(),
            ..Settings::default()
        };
        let (p, w) = Settings::parse(&m.to_text());
        assert!(w.is_empty(), "{w:?}");
        assert!(!p.auto_switch);
        assert!(!p.skip_passwords);
        assert!(p.run_as_admin && !d.run_as_admin);
        assert_eq!((p.theme.as_str(), s.theme.as_str()), ("dark", "system"));
        assert_eq!(Settings::parse("theme = blue").1.len(), 1);
        assert!(d.skip_passwords && s.skip_passwords);
        assert_eq!(p.sensitivity, 1.5);
        assert_eq!(p.hotkey_convert_word, m.hotkey_convert_word);
        assert_eq!(p.hotkey_toggle_auto, Hotkey::NONE);
        assert_eq!(p.exclude, m.exclude);
    }
}
