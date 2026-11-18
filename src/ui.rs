// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings window (`openswitch.exe --settings`), built with egui.
//! It edits %APPDATA%\OpenSwitch\settings.ini / exceptions.txt and tells the running tray app to reload.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, RichText};
use openswitch::config::{key_choices, Hotkey, Settings};
use openswitch::i18n::{self, t, Lang};

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
        self.s.to_text() != self.saved.to_text()
            || self.words != self.saved_words
            || self.autostart != self.saved_autostart
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
                    t("Saved and applied.")
                } else {
                    t("Saved. OpenSwitch is not running — start it for the settings to take effect.")
                };
                self.status = Some((msg.into(), true, Instant::now()));
            }
            (Err(e), _) | (_, Err(e)) => {
                log::error!("saving settings failed: {e}");
                self.status = Some((i18n::f("Could not save: {}", &[&e]), false, Instant::now()));
            }
        }
    }

    fn conflicts(&self) -> Option<&'static str> {
        let h = [&self.s.hotkey_convert_word, &self.s.hotkey_convert_selection, &self.s.hotkey_toggle_auto];
        for i in 0..h.len() {
            for j in i + 1..h.len() {
                if h[i].vk != 0 && h[i] == h[j] {
                    return Some(t("Two commands are assigned to the same key combination."));
                }
            }
        }
        None
    }

    // ---------------------------------------------------------------- tabs

    fn tab_general(&mut self, ui: &mut egui::Ui) {
        section(ui, t("Interface language"));
        egui::ComboBox::from_id_salt("language").selected_text(language_label(&self.s.language)).width(260.0).show_ui(
            ui,
            |ui| {
                let system = format!("{} ({})", t("System"), i18n::system().native());
                ui.selectable_value(&mut self.s.language, "system".to_string(), system);
                for lang in Lang::ALL {
                    ui.selectable_value(&mut self.s.language, lang.code().to_string(), lang.native());
                }
            },
        );

        section(ui, t("Automatic switching"));
        ui.checkbox(&mut self.s.auto_switch, t("Fix words typed in the wrong layout and switch the layout"));
        ui.add_enabled_ui(self.s.auto_switch, |ui| {
            ui.indent("auto", |ui| {
                ui.checkbox(&mut self.s.switch_on_enter, t("Check the word on Enter too (not only on Space)"));
                ui.checkbox(&mut self.s.single_letters, t("Fix one-letter words (d — в, z — я, ш — i)"));
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(t("Sensitivity:"));
                    ui.label(RichText::new(t("cautious")).weak());
                    ui.add(egui::Slider::new(&mut self.s.sensitivity, 0.5..=2.0).step_by(0.05).show_value(false));
                    ui.label(RichText::new(t("bold")).weak());
                    if (self.s.sensitivity - 1.0).abs() > 0.01 && ui.small_button(t("default")).clicked() {
                        self.s.sensitivity = 1.0;
                    }
                });
            });
        });

        section(ui, t("Manual correction"));
        ui.checkbox(
            &mut self.s.double_shift,
            t("Double-press Shift fixes the last word (handy on laptops without Pause)"),
        );

        section(ui, t("System"));
        ui.checkbox(&mut self.autostart, t("Start OpenSwitch with Windows"));

        section(ui, t("Test"));
        ui.label(
            RichText::new(t(
                "Type something in the wrong layout here, e.g. «ghbdtn» and a space. Works while OpenSwitch is running (tray icon).",
            ))
            .weak(),
        );
        ui.add(egui::TextEdit::multiline(&mut self.test_text).desired_rows(3).desired_width(f32::INFINITY));
    }

    fn tab_hotkeys(&mut self, ui: &mut egui::Ui) {
        section(ui, t("Hotkeys"));
        egui::Grid::new("hk").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
            let keys = &self.keys;
            ui.label(t("Fix the last word\n(press again to undo)"));
            hotkey_editor(ui, "hk_word", &mut self.s.hotkey_convert_word, keys);
            ui.end_row();
            ui.label(t("Fix selected text"));
            hotkey_editor(ui, "hk_sel", &mut self.s.hotkey_convert_selection, keys);
            ui.end_row();
            ui.label(t("Enable/disable auto switching"));
            hotkey_editor(ui, "hk_auto", &mut self.s.hotkey_toggle_auto, keys);
            ui.end_row();
        });
        if let Some(c) = self.conflicts() {
            ui.add_space(6.0);
            ui.colored_label(Color32::from_rgb(0xD0, 0x50, 0x30), c);
        }
        ui.add_space(10.0);
        ui.label(
            RichText::new(t(
                "If you cancelled an auto fix with the hotkey, the word is remembered and never fixed again (see the «Words» tab).\nCtrl+Pause works too (Windows calls it Break).",
            ))
            .weak(),
        );
        if ui.button(t("Restore default keys")).clicked() {
            let d = Settings::default();
            self.s.hotkey_convert_word = d.hotkey_convert_word;
            self.s.hotkey_convert_selection = d.hotkey_convert_selection;
            self.s.hotkey_toggle_auto = d.hotkey_toggle_auto;
        }
    }

    fn tab_programs(&mut self, ui: &mut egui::Ui) {
        section(ui, t("Programs without auto switching"));
        ui.label(
            RichText::new(t(
                "Words are not fixed automatically in these programs, but hotkeys work. Handy for terminals, code editors and games.",
            ))
            .weak(),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let r = ui
                .add(egui::TextEdit::singleline(&mut self.new_exe).hint_text(t("e.g. code.exe")).desired_width(180.0));
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button(t("Add")).clicked() || enter {
                add_exe(&mut self.s, &self.new_exe);
                self.new_exe.clear();
            }
            let mut picked: Option<String> = None;
            egui::ComboBox::from_id_salt("running").selected_text(t("Choose from open windows…")).width(220.0).show_ui(
                ui,
                |ui| {
                    for e in &self.running {
                        if !self.s.exclude.contains(e) && ui.selectable_label(false, e).clicked() {
                            picked = Some(e.clone());
                        }
                    }
                },
            );
            if let Some(p) = picked {
                add_exe(&mut self.s, &p);
            }
            if ui.small_button(t("Refresh")).on_hover_text(t("Update window list")).clicked() {
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
                    ui.label(RichText::new(t("List is empty")).weak());
                }
                for e in list {
                    ui.horizontal(|ui| {
                        if ui.small_button("×").on_hover_text(t("Remove")).clicked() {
                            self.s.exclude.remove(&e);
                        }
                        ui.label(&e);
                    });
                }
            });
        });
    }

    fn tab_words(&mut self, ui: &mut egui::Ui) {
        section(ui, t("Exception words"));
        ui.label(
            RichText::new(t(
                "These words are never fixed automatically. Write them the way you type them (e.g. «ctx» or «git»).",
            ))
            .weak(),
        );
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let r =
                ui.add(egui::TextEdit::singleline(&mut self.new_word).hint_text(t("new word")).desired_width(180.0));
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button(t("Add")).clicked() || enter {
                let wd = self.new_word.trim().to_lowercase();
                if !wd.is_empty() && !self.words.contains(&wd) {
                    self.words.push(wd);
                    self.words.sort();
                }
                self.new_word.clear();
            }
            ui.add_space(12.0);
            ui.label(t("Search:"));
            ui.add(egui::TextEdit::singleline(&mut self.word_filter).desired_width(140.0));
        });
        ui.add_space(6.0);
        let filter = self.word_filter.to_lowercase();
        let mut remove = None;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().max_height(260.0).auto_shrink([false, true]).show(ui, |ui| {
                if self.words.is_empty() {
                    ui.label(RichText::new(t("No words yet")).weak());
                }
                for (i, wd) in self.words.iter().enumerate() {
                    if !filter.is_empty() && !wd.contains(&filter) {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        if ui.small_button("×").on_hover_text(t("Remove")).clicked() {
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
        ui.label(t("Free automatic keyboard layout switcher for Windows."));
        ui.add_space(8.0);
        ui.label(t("• Works offline, sends nothing anywhere."));
        ui.label(t("• Dictionaries: the wordfreq project (CC BY-SA 4.0)."));
        ui.label(t("• License: GNU GPL v3.0 or later."));
        ui.add_space(8.0);
        ui.hyperlink_to(t("Source code on GitHub"), "https://github.com/DezFix/OpenSwitch");
        ui.add_space(8.0);
        if ui.button(t("Open settings folder")).clicked() {
            sys::open_path(&sys::data_dir());
        }
    }
}

fn language_label(setting: &str) -> String {
    let name = i18n::resolve(setting).native();
    if setting.eq_ignore_ascii_case("system") {
        format!("{} ({name})", t("System"))
    } else {
        name.to_string()
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
        let lang = i18n::resolve(&self.s.language);
        if lang != i18n::current() {
            i18n::set(lang);
            self.keys = key_choices();
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Title(t("OpenSwitch — Settings").into()));
        }
        egui::Panel::top("tabs").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                for (tab, name) in [
                    (Tab::General, t("General")),
                    (Tab::Hotkeys, t("Hotkeys")),
                    (Tab::Programs, t("Programs")),
                    (Tab::Words, t("Words")),
                    (Tab::About, t("About")),
                ] {
                    ui.selectable_value(&mut self.tab, tab, RichText::new(name).size(15.0));
                }
            });
            ui.add_space(4.0);
        });
        egui::Panel::bottom("buttons").show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let dirty = self.dirty();
                let can_save = dirty && self.conflicts().is_none();
                if ui.add_enabled(can_save, egui::Button::new(RichText::new(t("Save")).size(15.0))).clicked() {
                    self.save();
                }
                if ui.add_enabled(dirty, egui::Button::new(t("Revert changes"))).clicked() {
                    self.s = self.saved.clone();
                    self.words = self.saved_words.clone();
                    self.autostart = self.saved_autostart;
                }
                if sys::tray_window().is_null() && ui.button(t("Run OpenSwitch")).clicked() {
                    sys::spawn_self(&[]);
                }
                if let Some((msg, ok, at)) = &self.status {
                    if at.elapsed() < Duration::from_secs(6) {
                        let c =
                            if *ok { Color32::from_rgb(0x2E, 0x9E, 0x4F) } else { Color32::from_rgb(0xD0, 0x50, 0x30) };
                        ui.colored_label(c, msg);
                        ui.ctx().request_repaint_after(Duration::from_millis(500));
                    }
                } else if dirty {
                    ui.label(RichText::new(t("unsaved changes")).weak());
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

/// Window icon: the application icon (assets/openswitch.svg) rendered at 64x64, raw RGBA.
fn app_icon() -> egui::IconData {
    const RGBA: &[u8] = include_bytes!("../assets/icon-64.rgba");
    egui::IconData { rgba: RGBA.to_vec(), width: 64, height: 64 }
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

    let (settings, _) = sys::load_settings();
    i18n::set(i18n::resolve(&settings.language));
    let title = t("OpenSwitch — Settings");
    unsafe {
        let _m = CreateMutexW(std::ptr::null(), 1, sys::w("Local\\OpenSwitch.Settings").as_ptr());
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let h = FindWindowW(std::ptr::null(), sys::w(title).as_ptr());
            if !h.is_null() {
                ShowWindow(h, SW_RESTORE);
                SetForegroundWindow(h);
            }
            return;
        }
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(title)
            .with_inner_size([640.0, 560.0])
            .with_min_inner_size([520.0, 420.0])
            .with_icon(app_icon()),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        title,
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(SettingsApp::new()))
        }),
    ) {
        // Typically: no OpenGL 2+ (virtual machines, some RDP sessions, very old drivers).
        log::error!("settings window failed to start: {e}");
        let text = i18n::f(
            "Could not open the settings window:\n{}\n\nMost likely the video driver does not support OpenGL 2 (happens in virtual machines and over RDP).\nYou can edit the settings manually in the file:\n{}",
            &[&e, &sys::settings_path().display()],
        );
        unsafe {
            windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                std::ptr::null_mut(),
                sys::w(&text).as_ptr(),
                sys::w(title).as_ptr(),
                windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
            );
        }
        sys::open_path(&sys::data_dir());
    }
}
