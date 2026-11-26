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
    ("Run OpenSwitch", "Запустить OpenSwitch"),
    ("Could not save: {}", "Не удалось сохранить: {}"),
    (
        "Two commands are assigned to the same key combination.",
        "Две команды назначены на одно и то же сочетание клавиш.",
    ),
    ("Interface language", "Язык интерфейса"),
    (
        "Check the word on Enter too (not only on Space)",
        "Проверять слово и при нажатии Enter (не только пробел)",
    ),
    (
        "Fix one-letter words (d — в, z — я, ш — i)",
        "Исправлять однобуквенные слова (d — в, z — я, ш — i)",
    ),
    ("default", "по умолчанию"),
    ("System", "Система"),
    ("Start OpenSwitch with Windows", "Запускать OpenSwitch вместе с Windows"),
    ("Fix selected text", "Исправить выделенный текст"),
    ("Enable/disable auto switching", "Вкл/выкл автопереключение"),
    ("Restore default keys", "Вернуть клавиши по умолчанию"),
    ("Programs without auto switching", "Программы без автопереключения"),
    (
        "Words are not fixed automatically in these programs, but hotkeys work. Handy for terminals, code editors and games.",
        "В этих программах слова не исправляются сами, но горячие клавиши работают. Удобно для терминалов, редакторов кода и игр.",
    ),
    ("e.g. code.exe", "например code.exe"),
    ("Add", "Добавить"),
    ("Choose from open windows…", "Выбрать из открытых окон…"),
    ("Update window list", "Обновить список окон"),
    ("List is empty", "Список пуст"),
    ("Remove", "Убрать"),
    (
        "These words are never fixed automatically. Write them the way you type them (e.g. «ctx» or «git»).",
        "Эти слова никогда не исправляются автоматически. Пишите их так, как набираете (например «ctx» или «git»).",
    ),
    ("new word", "новое слово"),
    ("No words yet", "Пока нет ни одного слова"),
    ("Free automatic keyboard layout switcher for Windows.", "Свободный автоматический переключатель раскладки для Windows."),
    ("• Works offline, sends nothing anywhere.", "• Работает без интернета, ничего никуда не отправляет."),
    ("• Dictionaries: the wordfreq project (CC BY-SA 4.0).", "• Словари: проект wordfreq (CC BY-SA 4.0)."),
    ("Source code on GitHub", "Исходный код на GitHub"),
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
    ("Fix the layout automatically", "Исправлять раскладку автоматически"),
    ("A word typed in the wrong layout is fixed after Space", "Слово, набранное не в той раскладке, исправляется после пробела"),
    ("How boldly to fix", "Насколько смело исправлять"),
    ("Cautious: fewer mistakes, bold: fixes more words", "Осторожно — меньше ошибок, смело — исправляет больше слов"),
    ("Cautious", "Осторожно"),
    ("Normal", "Обычно"),
    ("Bold", "Смело"),
    ("Sensitivity", "Чувствительность"),
    ("Exact sensitivity", "Точная чувствительность"),
    ("Fine tuning", "Тонкая настройка"),
    ("Try it here", "Проверьте здесь"),
    ("Type «ghbdtn» and a space, it should become «привет»", "Наберите «ghbdtn» и пробел — должно получиться «привет»"),
    ("Fix the last word", "Исправить последнее слово"),
    ("Press again to undo", "Нажмите ещё раз, чтобы вернуть"),
    ("Press keys…", "Нажмите клавиши…"),
    ("This key can't be used, try another", "Эту клавишу нельзя назначить, попробуйте другую"),
    ("Esc: cancel, Backspace: no hotkey", "Esc — отмена, Backspace — без клавиши"),
    ("No hotkey", "Без клавиши"),
    ("Click and press the key combination", "Нажмите и введите сочетание клавиш"),
    ("Double Shift fixes the last word", "Двойной Shift исправляет последнее слово"),
    ("Handy on laptops without the Pause key", "Удобно на ноутбуках без клавиши Pause"),
    ("Ctrl+Pause works too (Windows calls it Break).", "Ctrl+Pause тоже работает (Windows называет его Break)."),
    ("or", "или"),
    ("A word is added here by itself when you undo its automatic fix.", "Слово попадает сюда само, если вы отменили его автоисправление."),
    ("License: GNU GPL v3.0 or later.", "Лицензия: GNU GPL v3.0 или новее."),
    ("Program files", "Файлы программы"),
    ("Settings, exception words and the log for bug reports", "Настройки, слова-исключения и журнал для сообщений об ошибках"),
    ("Log", "Журнал"),
    ("Settings", "Настройки"),
    ("Reset settings", "Сбросить настройки"),
    ("Exception words are kept", "Слова-исключения останутся"),
    ("Reset", "Сбросить"),
    ("Click again to reset", "Нажмите ещё раз для сброса"),
    ("Search", "Поиск"),
    ("Nothing found", "Ничего не найдено"),
    ("Changes applied", "Изменения применены"),
    ("Not saved: two commands have the same keys.", "Не сохранено: у двух команд одинаковые клавиши."),
    ("OpenSwitch is not running, so these settings don't work yet.", "OpenSwitch не запущен, поэтому настройки пока не действуют."),
    ("Undo a fix with Backspace", "Отменять исправление клавишей Backspace"),
    ("Backspace right after a fix brings the word back", "Backspace сразу после исправления возвращает слово как было"),
    ("Fix accidental CapsLock", "Исправлять случайный CapsLock"),
    ("пРИВЕТ becomes Привет and CapsLock turns off", "пРИВЕТ станет Привет, а CapsLock выключится"),
    ("Don't touch links, e-mail and code", "Не трогать ссылки, почту и код"),
    ("github.com, readme.md, getValue", "github.com, readme.md, getValue"),
    ("Be careful after you switch the layout yourself", "Осторожнее после ручного переключения раскладки"),
    ("The first word after a manual switch is fixed only when it is clearly wrong", "Первое слово после ручного переключения исправляется, только если ошибка очевидна"),
    ("Learn from my fixes", "Учиться на моих исправлениях"),
    ("A word you fixed by hand with a hotkey is fixed automatically next time", "Слово, которое вы исправили вручную клавишей, в следующий раз исправится само"),
    ("Never fix", "Никогда не исправлять"),
    ("Always fix", "Всегда исправлять"),
    ("These words are always fixed. Write them the way you type them by mistake (e.g. «ghbdtn»).", "Эти слова исправляются всегда. Пишите их так, как набираете по ошибке (например «ghbdtn»)."),
    ("Words you fixed by hand get here by themselves.", "Слова, которые вы исправили вручную, попадают сюда сами."),
    ("Layouts", "Раскладки"),
    ("Switch between these layouts", "Переключать между раскладками"),
    ("Turn a layout off and words are never switched to it automatically (Pause still can).", "Если выключить раскладку, слова не будут автоматически переводиться на неё (Pause по-прежнему может)."),
    ("Windows has fewer than two layouts OpenSwitch knows (English, Russian, Ukrainian, Belarusian). Add one in Windows settings → Time and language → Language.", "В Windows меньше двух раскладок, которые знает OpenSwitch (английская, русская, украинская, белорусская). Добавьте раскладку: Параметры Windows → Время и язык → Язык."),
    ("English", "Английская"),
    ("Russian", "Русская"),
    ("Ukrainian", "Украинская"),
    ("Belarusian", "Белорусская"),
    ("Capital letter at the start of a sentence", "Заглавная буква в начале предложения"),
    ("After . ! ? and Enter; English i becomes I", "После . ! ? и Enter; английское i станет I"),
    ("Remove the space before , . ! ?", "Убирать пробел перед , . ! ?"),
    ("«привет , как» becomes «привет, как»", "«привет , как» станет «привет, как»"),
    ("Layout", "Раскладка"),
    ("Text", "Текст"),
    ("Spelling", "Орфография"),
    ("Module «Layout»", "Модуль «Раскладка»"),
    ("Fixes words typed in the wrong layout and switches the layout. Hotkeys and double Shift belong to it too.", "Исправляет слова, набранные не в той раскладке, и переключает её. Сюда же относятся горячие клавиши и двойной Shift."),
    ("Module «Text»", "Модуль «Текст»"),
    ("Small fixes by simple rules: capital letters, an accidental CapsLock, a space before a mark.", "Небольшие правки по простым правилам: заглавные буквы, случайный CapsLock, пробел перед знаком."),
    ("Module «Spelling»", "Модуль «Орфография»"),
    ("Fixes typos with the built-in dictionaries: «првиет» becomes «привет». Only when it is at least 85 % sure; words of 5+ letters.", "Исправляет опечатки по встроенным словарям: «првиет» станет «привет». Только когда уверен не меньше чем на 85 %; слова от 5 букв."),
    ("Backspace right after a fix brings the word back; undo the same word twice and it is never fixed again (see «Words»).", "Backspace сразу после исправления возвращает слово; если отменить одно и то же слово дважды, оно больше не исправляется (см. «Слова»)."),
    ("OpenSwitch — getting started", "OpenSwitch — начало работы"),
    ("Welcome to OpenSwitch", "Добро пожаловать в OpenSwitch"),
    ("OpenSwitch fixes text typed in the wrong keyboard layout: type «ghbdtn» and a space, and you get «привет» — the layout switches by itself, so the next word is right.", "OpenSwitch исправляет текст, набранный не в той раскладке: наберите «ghbdtn» и пробел — получится «привет», а раскладка переключится сама, и следующее слово уже будет правильным."),
    ("It can also help with text and fix typos. You choose what it does on the next step.", "Ещё он может помогать с текстом и исправлять опечатки. Что именно ему делать — выберете на следующем шаге."),
    ("Your privacy", "Ваша конфиденциальность"),
    ("• Works offline: nothing is ever sent anywhere.", "• Работает без интернета: ничего никуда не отправляется."),
    ("• What you type is never saved or written to the log.", "• Набранный текст нигде не сохраняется и не пишется в журнал."),
    ("• No AI: only dictionaries and simple rules, right on your computer.", "• Без ИИ: только словари и простые правила, прямо на вашем компьютере."),
    ("• Free and open source (GNU GPL).", "• Бесплатно и с открытым кодом (GNU GPL)."),
    ("What should OpenSwitch do?", "Что должен делать OpenSwitch?"),
    ("Each module works on its own. You can change this any time in the settings.", "Каждый модуль работает сам по себе. Это можно поменять в любой момент в настройках."),
    ("How to use it", "Как пользоваться"),
    ("fix the last word by hand (press again to put it back)", "исправить последнее слово вручную (ещё раз — вернуть)"),
    ("the same, for laptops without Pause", "то же самое, для ноутбуков без Pause"),
    ("fix the layout of the selected text", "исправить раскладку выделенного текста"),
    ("turn automatic fixing on or off", "включить или выключить автоисправление"),
    ("right after a fix: undo it", "сразу после исправления — отменить его"),
    ("The tray icon", "Значок в трее"),
    ("Shows the current layout (EN, RU, UA). Click it to turn automatic fixing on or off; right-click for the menu and Settings.", "Показывает текущую раскладку (EN, RU, UA). Клик — включить или выключить автоисправление; правый клик — меню и настройки."),
    ("If a word is fixed wrongly", "Если слово исправилось неправильно"),
    ("Undo it with Backspace or Pause: after two undos the word is remembered and left alone. A word you fixed by hand is learned and fixed by itself next time.", "Отмените Backspace или Pause: после двух отмен слово запоминается и больше не трогается. А слово, которое вы исправили вручную, программа выучит и в следующий раз исправит сама."),
    ("Try it", "Попробуйте"),
    ("Your layouts: {}", "Ваши раскладки: {}"),
    ("All set", "Готово"),
    ("OpenSwitch works in the background. Everything else — hotkeys, programs where it should not fix words, exception words — is in the settings: right-click the tray icon → Settings.", "OpenSwitch работает в фоне. Всё остальное — горячие клавиши, программы, где не нужно исправлять, слова-исключения — в настройках: правый клик по значку в трее → «Настройки»."),
    ("You can open this guide again in Settings → About.", "Это руководство можно открыть снова: Настройки → «О программе»."),
    ("Next", "Далее"),
    ("Back", "Назад"),
    ("Finish", "Готово"),
    ("Skip", "Пропустить"),
    ("First-run guide", "Руководство для начала"),
    ("How to use OpenSwitch and which modules to turn on", "Как пользоваться OpenSwitch и какие модули включить"),
    ("Show", "Показать"),
];

const UK: &[(&str, &str)] = &[
    ("OpenSwitch — Settings", "OpenSwitch — налаштування"),
    ("General", "Основні"),
    ("Hotkeys", "Клавіші"),
    ("Programs", "Програми"),
    ("Words", "Слова"),
    ("About", "Про програму"),
    ("Exit", "Вихід"),
    ("Run OpenSwitch", "Запустити OpenSwitch"),
    ("Could not save: {}", "Не вдалося зберегти: {}"),
    (
        "Two commands are assigned to the same key combination.",
        "Дві команди призначено на однакове поєднання клавіш.",
    ),
    ("Interface language", "Мова інтерфейсу"),
    (
        "Check the word on Enter too (not only on Space)",
        "Перевіряти слово і під час натискання Enter (не лише пробіл)",
    ),
    (
        "Fix one-letter words (d — в, z — я, ш — i)",
        "Виправляти однолітерні слова (d — в, z — я, ш — і)",
    ),
    ("default", "типове"),
    ("System", "Система"),
    ("Start OpenSwitch with Windows", "Запускати OpenSwitch разом із Windows"),
    ("Fix selected text", "Виправити виділений текст"),
    ("Enable/disable auto switching", "Увімк./вимк. автоперемикання"),
    ("Restore default keys", "Повернути клавіші типовими"),
    ("Programs without auto switching", "Програми без автоперемикання"),
    (
        "Words are not fixed automatically in these programs, but hotkeys work. Handy for terminals, code editors and games.",
        "У цих програмах слова не виправляються самі, але гарячі клавіші працюють. Зручно для терміналів, редакторів коду та ігор.",
    ),
    ("e.g. code.exe", "наприклад code.exe"),
    ("Add", "Додати"),
    ("Choose from open windows…", "Вибрати з відкритих вікон…"),
    ("Update window list", "Оновити список вікон"),
    ("List is empty", "Список порожній"),
    ("Remove", "Прибрати"),
    (
        "These words are never fixed automatically. Write them the way you type them (e.g. «ctx» or «git»).",
        "Ці слова ніколи не виправляються автоматично. Пишіть їх так, як набираєте (наприклад «ctx» або «git»).",
    ),
    ("new word", "нове слово"),
    ("No words yet", "Поки немає жодного слова"),
    ("Free automatic keyboard layout switcher for Windows.", "Вільний автоматичний перемикач розкладки для Windows."),
    ("• Works offline, sends nothing anywhere.", "• Працює без інтернету, нічого нікуди не надсилає."),
    ("• Dictionaries: the wordfreq project (CC BY-SA 4.0).", "• Словники: проєкт wordfreq (CC BY-SA 4.0)."),
    ("Source code on GitHub", "Вихідний код на GitHub"),
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
    ("Fix the layout automatically", "Виправляти розкладку автоматично"),
    ("A word typed in the wrong layout is fixed after Space", "Слово, набране не в тій розкладці, виправляється після пробілу"),
    ("How boldly to fix", "Наскільки сміливо виправляти"),
    ("Cautious: fewer mistakes, bold: fixes more words", "Обережно — менше помилок, сміливо — виправляє більше слів"),
    ("Cautious", "Обережно"),
    ("Normal", "Звичайно"),
    ("Bold", "Сміливо"),
    ("Sensitivity", "Чутливість"),
    ("Exact sensitivity", "Точна чутливість"),
    ("Fine tuning", "Тонке налаштування"),
    ("Try it here", "Перевірте тут"),
    ("Type «ghbdtn» and a space, it should become «привет»", "Наберіть «ghbdtn» і пробіл — має вийти «привет»"),
    ("Fix the last word", "Виправити останнє слово"),
    ("Press again to undo", "Натисніть ще раз, щоб повернути"),
    ("Press keys…", "Натисніть клавіші…"),
    ("This key can't be used, try another", "Цю клавішу не можна призначити, спробуйте іншу"),
    ("Esc: cancel, Backspace: no hotkey", "Esc — скасувати, Backspace — без клавіші"),
    ("No hotkey", "Без клавіші"),
    ("Click and press the key combination", "Натисніть і введіть комбінацію клавіш"),
    ("Double Shift fixes the last word", "Подвійний Shift виправляє останнє слово"),
    ("Handy on laptops without the Pause key", "Зручно на ноутбуках без клавіші Pause"),
    ("Ctrl+Pause works too (Windows calls it Break).", "Ctrl+Pause теж працює (Windows називає його Break)."),
    ("or", "або"),
    ("A word is added here by itself when you undo its automatic fix.", "Слово потрапляє сюди саме, якщо ви скасували його автовиправлення."),
    ("License: GNU GPL v3.0 or later.", "Ліцензія: GNU GPL v3.0 або новіша."),
    ("Program files", "Файли програми"),
    ("Settings, exception words and the log for bug reports", "Налаштування, слова-винятки та журнал для повідомлень про помилки"),
    ("Log", "Журнал"),
    ("Settings", "Налаштування"),
    ("Reset settings", "Скинути налаштування"),
    ("Exception words are kept", "Слова-винятки залишаться"),
    ("Reset", "Скинути"),
    ("Click again to reset", "Натисніть ще раз для скидання"),
    ("Search", "Пошук"),
    ("Nothing found", "Нічого не знайдено"),
    ("Changes applied", "Зміни застосовано"),
    ("Not saved: two commands have the same keys.", "Не збережено: дві команди мають однакові клавіші."),
    ("OpenSwitch is not running, so these settings don't work yet.", "OpenSwitch не запущено, тому налаштування поки не діють."),
    ("Undo a fix with Backspace", "Скасовувати виправлення клавішею Backspace"),
    ("Backspace right after a fix brings the word back", "Backspace одразу після виправлення повертає слово як було"),
    ("Fix accidental CapsLock", "Виправляти випадковий CapsLock"),
    ("пРИВЕТ becomes Привет and CapsLock turns off", "пРИВІТ стане Привіт, а CapsLock вимкнеться"),
    ("Don't touch links, e-mail and code", "Не чіпати посилання, пошту та код"),
    ("github.com, readme.md, getValue", "github.com, readme.md, getValue"),
    ("Be careful after you switch the layout yourself", "Обережніше після ручного перемикання розкладки"),
    ("The first word after a manual switch is fixed only when it is clearly wrong", "Перше слово після ручного перемикання виправляється, лише якщо помилка очевидна"),
    ("Learn from my fixes", "Вчитися на моїх виправленнях"),
    ("A word you fixed by hand with a hotkey is fixed automatically next time", "Слово, яке ви виправили вручну клавішею, наступного разу виправиться саме"),
    ("Never fix", "Ніколи не виправляти"),
    ("Always fix", "Завжди виправляти"),
    ("These words are always fixed. Write them the way you type them by mistake (e.g. «ghbdtn»).", "Ці слова виправляються завжди. Пишіть їх так, як набираєте помилково (наприклад «ghbdtn»)."),
    ("Words you fixed by hand get here by themselves.", "Слова, які ви виправили вручну, потрапляють сюди самі."),
    ("Layouts", "Розкладки"),
    ("Switch between these layouts", "Перемикати між розкладками"),
    ("Turn a layout off and words are never switched to it automatically (Pause still can).", "Якщо вимкнути розкладку, слова не перекладатимуться на неї автоматично (Pause, як і раніше, може)."),
    ("Windows has fewer than two layouts OpenSwitch knows (English, Russian, Ukrainian, Belarusian). Add one in Windows settings → Time and language → Language.", "У Windows менше двох розкладок, які знає OpenSwitch (англійська, російська, українська, білоруська). Додайте розкладку: Параметри Windows → Час і мова → Мова."),
    ("English", "Англійська"),
    ("Russian", "Російська"),
    ("Ukrainian", "Українська"),
    ("Belarusian", "Білоруська"),
    ("Capital letter at the start of a sentence", "Велика літера на початку речення"),
    ("After . ! ? and Enter; English i becomes I", "Після . ! ? та Enter; англійське i стане I"),
    ("Remove the space before , . ! ?", "Прибирати пробіл перед , . ! ?"),
    ("«привет , как» becomes «привет, как»", "«привіт , як» стане «привіт, як»"),
    ("Layout", "Розкладка"),
    ("Text", "Текст"),
    ("Spelling", "Орфографія"),
    ("Module «Layout»", "Модуль «Розкладка»"),
    ("Fixes words typed in the wrong layout and switches the layout. Hotkeys and double Shift belong to it too.", "Виправляє слова, набрані не в тій розкладці, і перемикає її. Сюди ж належать гарячі клавіші та подвійний Shift."),
    ("Module «Text»", "Модуль «Текст»"),
    ("Small fixes by simple rules: capital letters, an accidental CapsLock, a space before a mark.", "Невеликі виправлення за простими правилами: великі літери, випадковий CapsLock, пробіл перед знаком."),
    ("Module «Spelling»", "Модуль «Орфографія»"),
    ("Fixes typos with the built-in dictionaries: «првиет» becomes «привет». Only when it is at least 85 % sure; words of 5+ letters.", "Виправляє помилки за вбудованими словниками: «првіет» стане «привіт». Лише коли впевнений щонайменше на 85 %; слова від 5 літер."),
    ("Backspace right after a fix brings the word back; undo the same word twice and it is never fixed again (see «Words»).", "Backspace одразу після виправлення повертає слово; якщо скасувати одне й те саме слово двічі, воно більше не виправляється (див. «Слова»)."),
    ("OpenSwitch — getting started", "OpenSwitch — початок роботи"),
    ("Welcome to OpenSwitch", "Ласкаво просимо до OpenSwitch"),
    ("OpenSwitch fixes text typed in the wrong keyboard layout: type «ghbdtn» and a space, and you get «привет» — the layout switches by itself, so the next word is right.", "OpenSwitch виправляє текст, набраний не в тій розкладці: наберіть «ghbdtn» і пробіл — вийде «привет», а розкладка перемкнеться сама, і наступне слово вже буде правильним."),
    ("It can also help with text and fix typos. You choose what it does on the next step.", "Ще він може допомагати з текстом і виправляти помилки. Що саме йому робити — оберете на наступному кроці."),
    ("Your privacy", "Ваша конфіденційність"),
    ("• Works offline: nothing is ever sent anywhere.", "• Працює без інтернету: нічого нікуди не надсилається."),
    ("• What you type is never saved or written to the log.", "• Набраний текст ніде не зберігається і не пишеться в журнал."),
    ("• No AI: only dictionaries and simple rules, right on your computer.", "• Без ШІ: лише словники та прості правила, просто на вашому комп'ютері."),
    ("• Free and open source (GNU GPL).", "• Безкоштовно і з відкритим кодом (GNU GPL)."),
    ("What should OpenSwitch do?", "Що має робити OpenSwitch?"),
    ("Each module works on its own. You can change this any time in the settings.", "Кожен модуль працює сам по собі. Це можна змінити будь-коли в налаштуваннях."),
    ("How to use it", "Як користуватися"),
    ("fix the last word by hand (press again to put it back)", "виправити останнє слово вручну (ще раз — повернути)"),
    ("the same, for laptops without Pause", "те саме, для ноутбуків без Pause"),
    ("fix the layout of the selected text", "виправити розкладку виділеного тексту"),
    ("turn automatic fixing on or off", "увімкнути або вимкнути автовиправлення"),
    ("right after a fix: undo it", "одразу після виправлення — скасувати його"),
    ("The tray icon", "Значок у треї"),
    ("Shows the current layout (EN, RU, UA). Click it to turn automatic fixing on or off; right-click for the menu and Settings.", "Показує поточну розкладку (EN, RU, UA). Клік — увімкнути або вимкнути автовиправлення; правий клік — меню та налаштування."),
    ("If a word is fixed wrongly", "Якщо слово виправилося неправильно"),
    ("Undo it with Backspace or Pause: after two undos the word is remembered and left alone. A word you fixed by hand is learned and fixed by itself next time.", "Скасуйте Backspace або Pause: після двох скасувань слово запам'ятовується і більше не чіпається. А слово, яке ви виправили вручну, програма вивчить і наступного разу виправить сама."),
    ("Try it", "Спробуйте"),
    ("Your layouts: {}", "Ваші розкладки: {}"),
    ("All set", "Готово"),
    ("OpenSwitch works in the background. Everything else — hotkeys, programs where it should not fix words, exception words — is in the settings: right-click the tray icon → Settings.", "OpenSwitch працює у фоні. Усе інше — гарячі клавіші, програми, де не треба виправляти, слова-винятки — у налаштуваннях: правий клік по значку в треї → «Налаштування»."),
    ("You can open this guide again in Settings → About.", "Цей посібник можна відкрити знову: Налаштування → «Про програму»."),
    ("Next", "Далі"),
    ("Back", "Назад"),
    ("Finish", "Готово"),
    ("Skip", "Пропустити"),
    ("First-run guide", "Посібник для початку"),
    ("How to use OpenSwitch and which modules to turn on", "Як користуватися OpenSwitch і які модулі увімкнути"),
    ("Show", "Показати"),
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
        assert_eq!(lookup(RU, "Search"), "Поиск");
        assert_eq!(lookup(UK, "Search"), "Пошук");
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
