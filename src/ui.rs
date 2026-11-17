//! Settings window (`openswitch.exe --settings`), built with egui.
//! It edits %APPDATA%\OpenSwitch\settings.ini / exceptions.txt and tells the running tray app to reload.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText};
use openswitch::config::{key_choices, Hotkey, Settings};

use crate::sys;

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    General,
    Hotkeys,
    Programs,
    Words,
    About,
}

struct SettingsApp {
    saved: Settings,
    s: Settings,
    saved_words: Vec<String>,
    words: Vec<String>,
    saved_autostart: bool,
    autostart: bool,
    tab: Tab,
    new_exe: String,
    new_word: String,
    word_filter: String,
    running: Vec<String>,
    test_text: String,
    status: Option<(String, bool, Instant)>,
    keys: Vec<(String, u8)>,
}

impl SettingsApp {
    fn new() -> Self {
        let (s, _) = sys::load_settings();
        let mut words: Vec<String> = sys::load_exceptions().into_iter().collect();
        words.sort();
        let autostart = sys::startup_enabled();
        SettingsApp {
            saved: s.clone(),
            s,
            saved_words: words.clone(),
            words,
            saved_autostart: autostart,
            autostart,
            tab: Tab::General,
            new_exe: String::new(),
            new_word: String::new(),
            word_filter: String::new(),
            running: sys::window_exes(),
            test_text: String::new(),
            status: None,
            keys: key_choices(),
        }
    }

    fn dirty(&self) -> bool {
        self.s.to_text() != self.saved.to_text() || self.words != self.saved_words || self.autostart != self.saved_autostart
    }

    fn save(&mut self) {
        let r1 = sys::save_settings(&self.s);
        let r2 = sys::save_exceptions(&self.words);
        if self.autostart != sys::startup_enabled() {
            sys::set_startup(self.autostart);
        }
        match (r1, r2) {
            (Ok(()), Ok(())) => {
                self.saved = self.s.clone();
                self.saved_words = self.words.clone();
                self.saved_autostart = self.autostart;
                let msg = if sys::notify_reload() {
                    "Сохранено и применено."
                } else {
                    "Сохранено. OpenSwitch сейчас не запущен — запустите его, чтобы настройки заработали."
                };
                self.status = Some((msg.into(), true, Instant::now()));
            }
            (Err(e), _) | (_, Err(e)) => {
                self.status = Some((format!("Не удалось сохранить: {e}"), false, Instant::now()));
            }
        }
    }

    fn conflicts(&self) -> Option<&'static str> {
        let h = [&self.s.hotkey_convert_word, &self.s.hotkey_convert_selection, &self.s.hotkey_toggle_auto];
        for i in 0..h.len() {
            for j in i + 1..h.len() {
                if h[i].vk != 0 && h[i] == h[j] {
                    return Some("Две команды назначены на одно и то же сочетание клавиш.");
                }
            }
        }
        None
    }

    // ---------------------------------------------------------------- tabs

    fn tab_general(&mut self, ui: &mut egui::Ui) {
        section(ui, "Автоматическое исправление");
        ui.checkbox(&mut self.s.auto_switch, "Исправлять слова, набранные не в той раскладке, и переключать раскладку");
        ui.add_enabled_ui(self.s.auto_switch, |ui| {
            ui.indent("auto", |ui| {
                ui.checkbox(&mut self.s.switch_on_enter, "Проверять слово и при нажатии Enter (не только пробел)");
                ui.checkbox(&mut self.s.single_letters, "Исправлять однобуквенные слова (d — в, z — я, ш — i)");
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Чувствительность:");
                    ui.label(RichText::new("осторожно").weak());
                    ui.add(egui::Slider::new(&mut self.s.sensitivity, 0.5..=2.0).step_by(0.05).show_value(false));
                    ui.label(RichText::new("смело").weak());
                    if (self.s.sensitivity - 1.0).abs() > 0.01 && ui.small_button("по умолчанию").clicked() {
                        self.s.sensitivity = 1.0;
                    }
                });
            });
        });

        section(ui, "Ручное исправление");
        ui.checkbox(&mut self.s.double_shift, "Двойное нажатие Shift исправляет последнее слово (удобно без клавиши Pause)");

        section(ui, "Система");
        ui.checkbox(&mut self.autostart, "Запускать OpenSwitch вместе с Windows");

        section(ui, "Проверка");
        ui.label(
            RichText::new("Наберите здесь что-нибудь не в той раскладке, например «ghbdtn» и пробел. Работает, пока OpenSwitch запущен (значок в трее).")
                .weak(),
        );
        ui.add(egui::TextEdit::multiline(&mut self.test_text).desired_rows(3).desired_width(f32::INFINITY));
    }

    fn tab_hotkeys(&mut self, ui: &mut egui::Ui) {
        section(ui, "Горячие клавиши");
        egui::Grid::new("hk").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
            let keys = &self.keys;
            ui.label("Исправить последнее слово\n(повторно — вернуть как было)");
            hotkey_editor(ui, "hk_word", &mut self.s.hotkey_convert_word, keys);
            ui.end_row();
            ui.label("Исправить выделенный текст");
            hotkey_editor(ui, "hk_sel", &mut self.s.hotkey_convert_selection, keys);
            ui.end_row();
            ui.label("Вкл/выкл автопереключение");
            hotkey_editor(ui, "hk_auto", &mut self.s.hotkey_toggle_auto, keys);
            ui.end_row();
        });
        if let Some(c) = self.conflicts() {
            ui.add_space(6.0);
            ui.colored_label(Color32::from_rgb(0xD0, 0x50, 0x30), c);
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new(
                "Если вы отменили автоисправление горячей клавишей, слово запоминается и больше не исправляется \
                 (см. вкладку «Слова»).\nСочетания с Ctrl+Pause работают тоже (Windows называет их Break).",
            )
            .weak(),
        );
        if ui.button("Вернуть клавиши по умолчанию").clicked() {
            let d = Settings::default();
            self.s.hotkey_convert_word = d.hotkey_convert_word;
            self.s.hotkey_convert_selection = d.hotkey_convert_selection;
            self.s.hotkey_toggle_auto = d.hotkey_toggle_auto;
        }
    }

    fn tab_programs(&mut self, ui: &mut egui::Ui) {
        section(ui, "Программы без автопереключения");
        ui.label(
            RichText::new("В этих программах слова не исправляются сами, но горячие клавиши работают. Удобно для терминалов, редакторов кода и игр.")
                .weak(),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.new_exe).hint_text("например code.exe").desired_width(180.0));
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Добавить").clicked() || enter {
                add_exe(&mut self.s, &self.new_exe);
                self.new_exe.clear();
            }
            let mut picked: Option<String> = None;
            egui::ComboBox::from_id_salt("running")
                .selected_text("Выбрать из открытых окон…")
                .width(220.0)
                .show_ui(ui, |ui| {
                    for e in &self.running {
                        if !self.s.exclude.contains(e) && ui.selectable_label(false, e).clicked() {
                            picked = Some(e.clone());
                        }
                    }
                });
            if let Some(p) = picked {
                add_exe(&mut self.s, &p);
            }
            if ui.small_button("Обновить").on_hover_text("Обновить список окон").clicked() {
                self.running = sys::window_exes();
            }
        });
        ui.add_space(6.0);
        let mut list: Vec<String> = self.s.exclude.iter().cloned().collect();
        list.sort();
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| {
                if list.is_empty() {
                    ui.label(RichText::new("Список пуст").weak());
                }
                for e in list {
                    ui.horizontal(|ui| {
                        if ui.small_button("×").on_hover_text("Убрать").clicked() {
                            self.s.exclude.remove(&e);
                        }
                        ui.label(&e);
                    });
                }
            });
        });
    }

    fn tab_words(&mut self, ui: &mut egui::Ui) {
        section(ui, "Слова-исключения");
        ui.label(
            RichText::new("Эти слова никогда не исправляются автоматически. Пишите их так, как набираете (например «ctx» или «git»).")
                .weak(),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let r = ui.add(egui::TextEdit::singleline(&mut self.new_word).hint_text("новое слово").desired_width(180.0));
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button("Добавить").clicked() || enter {
                let wd = self.new_word.trim().to_lowercase();
                if !wd.is_empty() && !self.words.contains(&wd) {
                    self.words.push(wd);
                    self.words.sort();
                }
                self.new_word.clear();
            }
            ui.add_space(12.0);
            ui.label("Поиск:");
            ui.add(egui::TextEdit::singleline(&mut self.word_filter).desired_width(140.0));
        });
        ui.add_space(6.0);
        let filter = self.word_filter.to_lowercase();
        let mut remove = None;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| {
                if self.words.is_empty() {
                    ui.label(RichText::new("Пока нет ни одного слова").weak());
                }
                for (i, wd) in self.words.iter().enumerate() {
                    if !filter.is_empty() && !wd.contains(&filter) {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        if ui.small_button("×").on_hover_text("Убрать").clicked() {
                            remove = Some(i);
                        }
                        ui.label(wd);
                    });
                }
            });
        });
        if let Some(i) = remove {
            self.words.remove(i);
        }
    }

    fn tab_about(&mut self, ui: &mut egui::Ui) {
        ui.add_space(8.0);
        ui.heading(format!("OpenSwitch {}", env!("CARGO_PKG_VERSION")));
        ui.label("Свободный автоматический переключатель раскладки EN / RU для Windows.");
        ui.add_space(8.0);
        ui.label("• Работает без интернета, ничего никуда не отправляет.");
        ui.label("• Словари: проект wordfreq (CC BY-SA 4.0).");
        ui.label("• Лицензия: GNU GPL v3.0 или новее.");
        ui.add_space(8.0);
        ui.hyperlink_to("Исходный код на GitHub", "https://github.com/DezFix/OpenSwitch");
        ui.add_space(8.0);
        if ui.button("Открыть папку с настройками").clicked() {
            sys::open_path(&sys::data_dir());
        }
    }
}

fn add_exe(s: &mut Settings, name: &str) {
    let mut n = name.trim().to_lowercase();
    if n.is_empty() {
        return;
    }
    if !n.ends_with(".exe") {
        n.push_str(".exe");
    }
    s.exclude.insert(n);
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).strong().size(16.0));
    ui.add_space(2.0);
}

fn hotkey_editor(ui: &mut egui::Ui, id: &str, hk: &mut Hotkey, keys: &[(String, u8)]) {
    ui.horizontal(|ui| {
        let on = hk.vk != 0;
        ui.add_enabled_ui(on, |ui| {
            ui.checkbox(&mut hk.ctrl, "Ctrl");
            ui.checkbox(&mut hk.alt, "Alt");
            ui.checkbox(&mut hk.shift, "Shift");
            ui.checkbox(&mut hk.win, "Win");
        });
        let current = keys.iter().find(|(_, v)| *v == hk.vk).map(|(n, _)| n.clone()).unwrap_or_else(|| hk.to_string());
        egui::ComboBox::from_id_salt(id).selected_text(current).width(110.0).height(300.0).show_ui(ui, |ui| {
            for (name, vk) in keys {
                ui.selectable_value(&mut hk.vk, *vk, name);
            }
        });
        if hk.vk == 0 {
            *hk = Hotkey::NONE;
        }
    });
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for (t, name) in [
                    (Tab::General, "Основные"),
                    (Tab::Hotkeys, "Клавиши"),
                    (Tab::Programs, "Программы"),
                    (Tab::Words, "Слова"),
                    (Tab::About, "О программе"),
                ] {
                    ui.selectable_value(&mut self.tab, t, RichText::new(name).size(15.0));
                }
            });
            ui.add_space(4.0);
        });
        egui::Panel::bottom("buttons").show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let dirty = self.dirty();
                let can_save = dirty && self.conflicts().is_none();
                if ui.add_enabled(can_save, egui::Button::new(RichText::new("Сохранить").size(15.0))).clicked() {
                    self.save();
                }
                if ui.add_enabled(dirty, egui::Button::new("Отменить изменения")).clicked() {
                    self.s = self.saved.clone();
                    self.words = self.saved_words.clone();
                    self.autostart = self.saved_autostart;
                }
                if sys::tray_window().is_null() && ui.button("Запустить OpenSwitch").clicked() {
                    sys::spawn_self(&[]);
                }
                if let Some((msg, ok, at)) = &self.status {
                    if at.elapsed() < Duration::from_secs(6) {
                        let c = if *ok { Color32::from_rgb(0x2E, 0x9E, 0x4F) } else { Color32::from_rgb(0xD0, 0x50, 0x30) };
                        ui.colored_label(c, msg);
                        ui.ctx().request_repaint_after(Duration::from_millis(500));
                    }
                } else if dirty {
                    ui.label(RichText::new("есть несохранённые изменения").weak());
                }
            });
            ui.add_space(8.0);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
                Tab::General => self.tab_general(ui),
                Tab::Hotkeys => self.tab_hotkeys(ui),
                Tab::Programs => self.tab_programs(ui),
                Tab::Words => self.tab_words(ui),
                Tab::About => self.tab_about(ui),
            });
        });
    }
}

/// Window icon: rounded square, half blue (EN) / half red (RU).
fn app_icon() -> egui::IconData {
    let n = 64usize;
    let mut rgba = vec![0u8; n * n * 4];
    let r = n as f32 * 0.22;
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let cx = fx.clamp(r, n as f32 - r);
            let cy = fy.clamp(r, n as f32 - r);
            let a = (r - ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() + 0.5).clamp(0.0, 1.0);
            let (cr, cg, cb) = if x + y < n { (0x1E, 0x7A, 0xC8) } else { (0xD6, 0x3A, 0x35) };
            // white double arrow stripe in the middle
            let on_line = (y as i32 - n as i32 / 2).abs() <= 3 && x > n / 5 && x < n * 4 / 5;
            let (cr, cg, cb) = if on_line { (0xFF, 0xFF, 0xFF) } else { (cr, cg, cb) };
            let i = (y * n + x) * 4;
            rgba[i..i + 4].copy_from_slice(&[cr, cg, cb, (a * 255.0) as u8]);
        }
    }
    egui::IconData { rgba, width: n as u32, height: n as u32 }
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    if let Ok(bytes) = std::fs::read(format!("{windir}\\Fonts\\segoeui.ttf")) {
        fonts.font_data.insert("segoe".into(), Arc::new(egui::FontData::from_owned(bytes)));
        if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            f.insert(0, "segoe".into());
        }
    }
    // Symbols fallback (arrows, crosses) after the main font.
    if let Ok(bytes) = std::fs::read(format!("{windir}\\Fonts\\seguisym.ttf")) {
        fonts.font_data.insert("segoe_sym".into(), Arc::new(egui::FontData::from_owned(bytes)));
        if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            let pos = if f.first().map(|s| s == "segoe").unwrap_or(false) { 1 } else { 0 };
            f.insert(pos, "segoe_sym".into());
        }
    }
    ctx.set_fonts(fonts);
    ctx.global_style_mut(|s| {
        use egui::{FontId, TextStyle};
        s.text_styles.insert(TextStyle::Body, FontId::proportional(14.5));
        s.text_styles.insert(TextStyle::Button, FontId::proportional(14.5));
        s.text_styles.insert(TextStyle::Heading, FontId::proportional(22.0));
        s.spacing.item_spacing = egui::vec2(8.0, 6.0);
    });
}

pub fn run() {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE};

    const TITLE: &str = "OpenSwitch — настройки";
    unsafe {
        let _m = CreateMutexW(std::ptr::null(), 1, sys::w("Local\\OpenSwitch.Settings").as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let h = FindWindowW(std::ptr::null(), sys::w(TITLE).as_ptr());
            if !h.is_null() {
                ShowWindow(h, SW_RESTORE);
                SetForegroundWindow(h);
            }
            return;
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(TITLE)
            .with_inner_size([640.0, 560.0])
            .with_min_inner_size([520.0, 420.0])
            .with_icon(app_icon()),
        ..Default::default()
    };
    let _ = eframe::run_native(
        TITLE,
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(SettingsApp::new()))
        }),
    );
}
