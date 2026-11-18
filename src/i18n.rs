// SPDX-License-Identifier: GPL-3.0-or-later
//! UI translations: English, Russian, Ukrainian.
//!
//! The English text is the lookup key: `t("Save")` returns the translation for the current
//! language, or the key itself in English. The current language is set once from the
//! `language` setting (`system` follows the Windows display language).

use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    En,
    Ru,
    Uk,
}

impl Lang {
    pub const ALL: [Lang; 3] = [Lang::En, Lang::Ru, Lang::Uk];

    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ru => "ru",
            Lang::Uk => "uk",
        }
    }

    /// Name in its own language, as shown in the language picker.
    pub fn native(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ru => "Русский",
            Lang::Uk => "Українська",
        }
    }

    pub fn from_code(s: &str) -> Option<Lang> {
        match s.to_ascii_lowercase().as_str() {
            "en" => Some(Lang::En),
            "ru" => Some(Lang::Ru),
            "uk" => Some(Lang::Uk),
            _ => None,
        }
    }

    fn id(self) -> u8 {
        match self {
            Lang::En => 0,
            Lang::Ru => 1,
            Lang::Uk => 2,
        }
    }

    fn from_id(id: u8) -> Lang {
        match id {
            1 => Lang::Ru,
            2 => Lang::Uk,
            _ => Lang::En,
        }
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: Lang) {
    CURRENT.store(lang.id(), Ordering::Relaxed);
}

pub fn current() -> Lang {
    Lang::from_id(CURRENT.load(Ordering::Relaxed))
}

/// Resolve the `language` setting value: `system`/empty follows Windows, otherwise `en`/`ru`/`uk`.
pub fn resolve(setting: &str) -> Lang {
    Lang::from_code(setting).unwrap_or_else(system)
}

/// Display language of Windows. Unsupported languages fall back to English.
#[cfg(windows)]
pub fn system() -> Lang {
    // LANGID: primary language in the low 10 bits; 0x09 English, 0x19 Russian, 0x22 Ukrainian.
    let id = unsafe { windows_sys::Win32::Globalization::GetUserDefaultUILanguage() };
    match id & 0x3ff {
        0x19 => Lang::Ru,
        0x22 => Lang::Uk,
        _ => Lang::En,
    }
}

#[cfg(not(windows))]
pub fn system() -> Lang {
    Lang::En
}

/// Translatable UI string; `key` is the English text.
pub fn t(key: &'static str) -> &'static str {
    match current() {
        Lang::En => key,
        Lang::Ru => lookup(RU, key),
        Lang::Uk => lookup(UK, key),
    }
}

fn lookup(table: &'static [(&'static str, &'static str)], key: &'static str) -> &'static str {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v).unwrap_or(key)
}

/// Translate a message with `{}` placeholders and fill them in order.
pub fn f(key: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    let mut out = t(key).to_string();
    for arg in args {
        let Some(i) = out.find("{}") else { break };
        out.replace_range(i..i + 2, &arg.to_string());
    }
    out
}

const RU: &[(&str, &str)] = &[
    ("OpenSwitch — Settings", "OpenSwitch — настройки"),
    ("General", "Основные"),
    ("Hotkeys", "Клавиши"),
    ("Programs", "Программы"),
    ("Words", "Слова"),
    ("About", "О программе"),
    ("Exit", "Выход"),
    ("Save", "Сохранить"),
    ("Revert changes", "Отменить изменения"),
    ("Run OpenSwitch", "Запустить OpenSwitch"),
    ("Saved and applied.", "Сохранено и применено."),
    (
        "Saved. OpenSwitch is not running — start it for the settings to take effect.",
        "Сохранено. OpenSwitch сейчас не запущен — запустите его, чтобы настройки заработали.",
    ),
    ("Could not save: {}", "Не удалось сохранить: {}"),
    ("unsaved changes", "есть несохранённые изменения"),
    (
        "Two commands are assigned to the same key combination.",
        "Две команды назначены на одно и то же сочетание клавиш.",
    ),
    ("Interface language", "Язык интерфейса"),
    ("Automatic switching", "Автоматическое исправление"),
    (
        "Fix words typed in the wrong layout and switch the layout",
        "Исправлять слова, набранные не в той раскладке, и переключать раскладку",
    ),
    (
        "Check the word on Enter too (not only on Space)",
        "Проверять слово и при нажатии Enter (не только пробел)",
    ),
    (
        "Fix one-letter words (d — в, z — я, ш — i)",
        "Исправлять однобуквенные слова (d — в, z — я, ш — i)",
    ),
    ("Sensitivity:", "Чувствительность:"),
    ("cautious", "осторожно"),
    ("bold", "смело"),
    ("default", "по умолчанию"),
    ("Manual correction", "Ручное исправление"),
    (
        "Double-press Shift fixes the last word (handy on laptops without Pause)",
        "Двойное нажатие Shift исправляет последнее слово (удобно без клавиши Pause)",
    ),
    ("System", "Система"),
    ("Start OpenSwitch with Windows", "Запускать OpenSwitch вместе с Windows"),
    ("Test", "Проверка"),
    (
        "Type something in the wrong layout here, e.g. «ghbdtn» and a space. Works while OpenSwitch is running (tray icon).",
        "Наберите здесь что-нибудь не в той раскладке, например «ghbdtn» и пробел. Работает, пока OpenSwitch запущен (значок в трее).",
    ),
    ("Fix the last word\n(press again to undo)", "Исправить последнее слово\n(повторно — вернуть как было)"),
    ("Fix selected text", "Исправить выделенный текст"),
    ("Enable/disable auto switching", "Вкл/выкл автопереключение"),
    ("Restore default keys", "Вернуть клавиши по умолчанию"),
    (
        "If you cancelled an auto fix with the hotkey, the word is remembered and never fixed again (see the «Words» tab).\nCtrl+Pause works too (Windows calls it Break).",
        "Если вы отменили автоисправление горячей клавишей, слово запоминается и больше не исправляется (см. вкладку «Слова»).\nСочетания с Ctrl+Pause работают тоже (Windows называет их Break).",
    ),
    ("Programs without auto switching", "Программы без автопереключения"),
    (
        "Words are not fixed automatically in these programs, but hotkeys work. Handy for terminals, code editors and games.",
        "В этих программах слова не исправляются сами, но горячие клавиши работают. Удобно для терминалов, редакторов кода и игр.",
    ),
    ("e.g. code.exe", "например code.exe"),
    ("Add", "Добавить"),
    ("Choose from open windows…", "Выбрать из открытых окон…"),
    ("Refresh", "Обновить"),
    ("Update window list", "Обновить список окон"),
    ("List is empty", "Список пуст"),
    ("Remove", "Убрать"),
    ("Exception words", "Слова-исключения"),
    (
        "These words are never fixed automatically. Write them the way you type them (e.g. «ctx» or «git»).",
        "Эти слова никогда не исправляются автоматически. Пишите их так, как набираете (например «ctx» или «git»).",
    ),
    ("new word", "новое слово"),
    ("Search:", "Поиск:"),
    ("No words yet", "Пока нет ни одного слова"),
    ("Free automatic keyboard layout switcher for Windows.", "Свободный автоматический переключатель раскладки для Windows."),
    ("• Works offline, sends nothing anywhere.", "• Работает без интернета, ничего никуда не отправляет."),
    ("• Dictionaries: the wordfreq project (CC BY-SA 4.0).", "• Словари: проект wordfreq (CC BY-SA 4.0)."),
    ("• License: GNU GPL v3.0 or later.", "• Лицензия: GNU GPL v3.0 или новее."),
    ("Source code on GitHub", "Исходный код на GitHub"),
    ("Open settings folder", "Открыть папку с настройками"),
    (
        "Could not open the settings window:\n{}\n\nMost likely the video driver does not support OpenGL 2 (happens in virtual machines and over RDP).\nYou can edit the settings manually in the file:\n{}",
        "Не удалось открыть окно настроек:\n{}\n\nСкорее всего, видеодрайвер не поддерживает OpenGL 2 (бывает в виртуальных машинах и по RDP).\nНастройки можно изменить вручную в файле:\n{}",
    ),
    ("None", "Нет"),
    ("OpenSwitch — auto switching is on", "OpenSwitch — автопереключение включено"),
    ("OpenSwitch — auto switching is off", "OpenSwitch — автопереключение выключено"),
    ("Settings…", "Настройки…"),
    ("Auto layout switching", "Автопереключение раскладки"),
    ("Start with Windows", "Запускать вместе с Windows"),
    ("Reload settings", "Перечитать настройки"),
    ("OpenSwitch {} — free EN/RU/UA layout switcher.\nGNU GPL v3 license.\n\n{} — fix the last word (again — undo)\n{}\n{} — fix selected text\n{} — enable/disable auto switching\n\nClick the icon — enable/disable auto switching.",
     "OpenSwitch {} — свободный переключатель раскладки EN/RU/UA.\nЛицензия GNU GPL v3.\n\n{} — исправить последнее слово (ещё раз — вернуть)\n{}\n{} — исправить выделенный текст\n{} — вкл/выкл автопереключение\n\nКлик по значку — вкл/выкл автопереключение."),
    ("Shift Shift (double press) — the same\n", "Shift Shift (двойное нажатие) — то же самое\n"),
    ("Errors in settings.ini:\n{}", "Ошибки в settings.ini:\n{}"),
    ("Could not create the window.", "Не удалось создать окно."),
    ("Could not install the keyboard hook.", "Не удалось установить перехватчик клавиатуры."),
    ("line {}: no '='", "строка {}: нет '='"),
    ("line {}: unknown option '{}'", "строка {}: неизвестный параметр '{}'"),
    ("line {}: bad value for '{}'", "строка {}: неверное значение для '{}'"),
    ("could not read {}: {}", "не удалось прочитать {}: {}"),
];

const UK: &[(&str, &str)] = &[
    ("OpenSwitch — Settings", "OpenSwitch — налаштування"),
    ("General", "Основні"),
    ("Hotkeys", "Клавіші"),
    ("Programs", "Програми"),
    ("Words", "Слова"),
    ("About", "Про програму"),
    ("Exit", "Вихід"),
    ("Save", "Зберегти"),
    ("Revert changes", "Скасувати зміни"),
    ("Run OpenSwitch", "Запустити OpenSwitch"),
    ("Saved and applied.", "Збережено і застосовано."),
    (
        "Saved. OpenSwitch is not running — start it for the settings to take effect.",
        "Збережено. OpenSwitch зараз не запущено — запустіть його, щоб налаштування запрацювали.",
    ),
    ("Could not save: {}", "Не вдалося зберегти: {}"),
    ("unsaved changes", "є незбережені зміни"),
    (
        "Two commands are assigned to the same key combination.",
        "Дві команди призначено на однакове поєднання клавіш.",
    ),
    ("Interface language", "Мова інтерфейсу"),
    ("Automatic switching", "Автоматичне виправлення"),
    (
        "Fix words typed in the wrong layout and switch the layout",
        "Виправляти слова, набрані не в тій розкладці, і перемикати розкладку",
    ),
    (
        "Check the word on Enter too (not only on Space)",
        "Перевіряти слово і під час натискання Enter (не лише пробіл)",
    ),
    (
        "Fix one-letter words (d — в, z — я, ш — i)",
        "Виправляти однолітерні слова (d — в, z — я, ш — і)",
    ),
    ("Sensitivity:", "Чутливість:"),
    ("cautious", "обережно"),
    ("bold", "сміливо"),
    ("default", "типове"),
    ("Manual correction", "Ручне виправлення"),
    (
        "Double-press Shift fixes the last word (handy on laptops without Pause)",
        "Подвійне натискання Shift виправляє останнє слово (зручно без клавіші Pause)",
    ),
    ("System", "Система"),
    ("Start OpenSwitch with Windows", "Запускати OpenSwitch разом із Windows"),
    ("Test", "Перевірка"),
    (
        "Type something in the wrong layout here, e.g. «ghbdtn» and a space. Works while OpenSwitch is running (tray icon).",
        "Наберіть тут щось не в тій розкладці, наприклад «ghbdtn» і пробіл. Працює, поки OpenSwitch запущено (значок у треї).",
    ),
    ("Fix the last word\n(press again to undo)", "Виправити останнє слово\n(повторно — повернути як було)"),
    ("Fix selected text", "Виправити виділений текст"),
    ("Enable/disable auto switching", "Увімк./вимк. автоперемикання"),
    ("Restore default keys", "Повернути клавіші типовими"),
    (
        "If you cancelled an auto fix with the hotkey, the word is remembered and never fixed again (see the «Words» tab).\nCtrl+Pause works too (Windows calls it Break).",
        "Якщо ви скасували автовиправлення гарячою клавішею, слово запам'ятовується і більше не виправляється (див. вкладку «Слова»).\nПоєднання з Ctrl+Pause теж працюють (Windows називає їх Break).",
    ),
    ("Programs without auto switching", "Програми без автоперемикання"),
    (
        "Words are not fixed automatically in these programs, but hotkeys work. Handy for terminals, code editors and games.",
        "У цих програмах слова не виправляються самі, але гарячі клавіші працюють. Зручно для терміналів, редакторів коду та ігор.",
    ),
    ("e.g. code.exe", "наприклад code.exe"),
    ("Add", "Додати"),
    ("Choose from open windows…", "Вибрати з відкритих вікон…"),
    ("Refresh", "Оновити"),
    ("Update window list", "Оновити список вікон"),
    ("List is empty", "Список порожній"),
    ("Remove", "Прибрати"),
    ("Exception words", "Слова-винятки"),
    (
        "These words are never fixed automatically. Write them the way you type them (e.g. «ctx» or «git»).",
        "Ці слова ніколи не виправляються автоматично. Пишіть їх так, як набираєте (наприклад «ctx» або «git»).",
    ),
    ("new word", "нове слово"),
    ("Search:", "Пошук:"),
    ("No words yet", "Поки немає жодного слова"),
    ("Free automatic keyboard layout switcher for Windows.", "Вільний автоматичний перемикач розкладки для Windows."),
    ("• Works offline, sends nothing anywhere.", "• Працює без інтернету, нічого нікуди не надсилає."),
    ("• Dictionaries: the wordfreq project (CC BY-SA 4.0).", "• Словники: проєкт wordfreq (CC BY-SA 4.0)."),
    ("• License: GNU GPL v3.0 or later.", "• Ліцензія: GNU GPL v3.0 або новіша."),
    ("Source code on GitHub", "Вихідний код на GitHub"),
    ("Open settings folder", "Відкрити теку з налаштуваннями"),
    (
        "Could not open the settings window:\n{}\n\nMost likely the video driver does not support OpenGL 2 (happens in virtual machines and over RDP).\nYou can edit the settings manually in the file:\n{}",
        "Не вдалося відкрити вікно налаштувань:\n{}\n\nНайімовірніше, відеодрайвер не підтримує OpenGL 2 (буває у віртуальних машинах і через RDP).\nНалаштування можна змінити вручну у файлі:\n{}",
    ),
    ("None", "Немає"),
    ("OpenSwitch — auto switching is on", "OpenSwitch — автоперемикання увімкнено"),
    ("OpenSwitch — auto switching is off", "OpenSwitch — автоперемикання вимкнено"),
    ("Settings…", "Налаштування…"),
    ("Auto layout switching", "Автоперемикання розкладки"),
    ("Start with Windows", "Запускати разом із Windows"),
    ("Reload settings", "Перечитати налаштування"),
    ("OpenSwitch {} — free EN/RU/UA layout switcher.\nGNU GPL v3 license.\n\n{} — fix the last word (again — undo)\n{}\n{} — fix selected text\n{} — enable/disable auto switching\n\nClick the icon — enable/disable auto switching.",
     "OpenSwitch {} — вільний перемикач розкладки EN/RU/UA.\nЛіцензія GNU GPL v3.\n\n{} — виправити останнє слово (ще раз — повернути)\n{}\n{} — виправити виділений текст\n{} — увімк./вимк. автоперемикання\n\nКлік по значку — увімк./вимк. автоперемикання."),
    ("Shift Shift (double press) — the same\n", "Shift Shift (подвійне натискання) — те саме\n"),
    ("Errors in settings.ini:\n{}", "Помилки в settings.ini:\n{}"),
    ("Could not create the window.", "Не вдалося створити вікно."),
    ("Could not install the keyboard hook.", "Не вдалося встановити перехоплювач клавіатури."),
    ("line {}: no '='", "рядок {}: немає '='"),
    ("line {}: unknown option '{}'", "рядок {}: невідомий параметр '{}'"),
    ("line {}: bad value for '{}'", "рядок {}: некоректне значення для '{}'"),
    ("could not read {}: {}", "не вдалося прочитати {}: {}"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_have_the_same_keys() {
        for (k, v) in RU {
            assert!(!k.is_empty() && !v.is_empty(), "empty RU entry for {k}");
            assert!(UK.iter().any(|(k2, _)| k2 == k), "uk missing key: {k}");
        }
        for (k, v) in UK {
            assert!(!k.is_empty() && !v.is_empty(), "empty UK entry for {k}");
            assert!(RU.iter().any(|(k2, _)| k2 == k), "ru missing key: {k}");
        }
    }

    #[test]
    fn no_duplicate_keys() {
        for table in [RU, UK] {
            for (i, (k, _)) in table.iter().enumerate() {
                assert!(table[..i].iter().all(|(k2, _)| k2 != k), "duplicate key: {k}");
            }
        }
    }

    #[test]
    fn translate_lookup() {
        assert_eq!(lookup(RU, "Save"), "Сохранить");
        assert_eq!(lookup(UK, "Save"), "Зберегти");
        assert_eq!(lookup(RU, "no such key"), "no such key");
        set(Lang::En);
        assert_eq!(f("line {}: no '='", &[&3]), "line 3: no '='");
    }

    #[test]
    fn language_codes() {
        assert_eq!(Lang::from_code("uk"), Some(Lang::Uk));
        assert_eq!(Lang::from_code("RU"), Some(Lang::Ru));
        assert_eq!(Lang::from_code("de"), None);
        assert_eq!(resolve("system"), system());
    }
}
