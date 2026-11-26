// SPDX-License-Identifier: GPL-3.0-or-later
//! Settings window (`openswitch.exe --settings`), built with egui.
//!
//! One scrolling page with all sections; the menu on the left follows the scroll position and
//! the search box above it filters the rows. Every change is written to
//! %APPDATA%\OpenSwitch\settings.ini / exceptions.txt right away and the running tray app is
//! told to reload: there is no "Save" button.

use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Color32, RichText, Sense};
use openswitch::config::{is_hotkey_key, Hotkey, Settings, SENSITIVITY_PRESETS, VK_CANCEL, VK_PAUSE};
use openswitch::i18n::{self, t, Lang};

use crate::admin::{self, Failure};
use crate::{logging, sys};

const SIDEBAR_WIDTH: f32 = 190.0;
/// Menu item highlighted after a click while the page scrolls there.
const JUMP_MS: u64 = 700;
pub(crate) const RED: Color32 = Color32::from_rgb(0xD0, 0x50, 0x30);
const GREEN: Color32 = Color32::from_rgb(0x2E, 0x9E, 0x4F);

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
enum Section {
    General,
    Layout,
    Text,
    Spelling,
    Hotkeys,
    Programs,
    Words,
    About,
}

impl Section {
    const ALL: [Section; 8] = [
        Section::General,
        Section::Layout,
        Section::Text,
        Section::Spelling,
        Section::Hotkeys,
        Section::Programs,
        Section::Words,
        Section::About,
    ];

    fn title(self) -> &'static str {
        match self {
            Section::General => t("General"),
            Section::Layout => t("Layout"),
            Section::Text => t("Text"),
            Section::Spelling => t("Spelling"),
            Section::Hotkeys => t("Hotkeys"),
            Section::Programs => t("Programs"),
            Section::Words => t("Words"),
            Section::About => t("About"),
        }
    }
}

/// The hotkeys edited in the window, in display order.
const HOTKEYS: [&str; 3] = ["Fix the last word", "Fix selected text", "Enable/disable auto switching"];

fn hotkey_mut(s: &mut Settings, i: usize) -> &mut Hotkey {
    match i {
        0 => &mut s.hotkey_convert_word,
        1 => &mut s.hotkey_convert_selection,
        _ => &mut s.hotkey_toggle_auto,
    }
}

/// Per-frame state of the page being drawn: search filter, section headers, scroll targets.
struct Page {
    /// Lowercase search query.
    query: String,
    section: Section,
    /// The section title matches the query: show all its rows.
    all: bool,
    header_drawn: bool,
    /// Screen y of each drawn section header.
    tops: Vec<(Section, f32)>,
    jump: Option<Section>,
}

impl Page {
    fn filtering(&self) -> bool {
        !self.query.is_empty()
    }

    fn hit(&self, text: &str) -> bool {
        self.query.is_empty() || text.to_lowercase().contains(&self.query)
    }

    fn begin(&mut self, section: Section) {
        self.section = section;
        self.header_drawn = false;
        self.all = self.filtering() && self.hit(section.title());
    }

    /// Should a row with these texts be shown? Draws the section header before the first one,
    /// so a section with no matching rows disappears entirely.
    fn row(&mut self, ui: &mut egui::Ui, texts: &[&str]) -> bool {
        if !(self.all || texts.iter().any(|x| self.hit(x))) {
            return false;
        }
        if !self.header_drawn {
            self.header_drawn = true;
            if !self.tops.is_empty() {
                ui.add_space(22.0);
            }
            let r = ui.label(RichText::new(self.section.title()).strong().size(19.0));
            if self.jump == Some(self.section) {
                ui.scroll_to_rect(r.rect.expand(10.0), Some(Align::TOP));
            }
            self.tops.push((self.section, r.rect.top()));
            ui.separator();
        }
        true
    }
}

struct SettingsApp {
    /// What the window shows and edits.
    s: Settings,
    /// What is on disk.
    saved: Settings,
    words: Vec<String>,
    saved_words: Vec<String>,
    /// Words learned from manual fixes (always.txt).
    always: Vec<String>,
    saved_always: Vec<String>,
    new_always: String,
    autostart: bool,
    search: String,
    /// Highlighted menu item.
    active: Section,
    /// Sections drawn in the last frame (others are hidden by the search).
    visible: Vec<Section>,
    jump: Option<(Section, Instant)>,
    new_exe: String,
    new_word: String,
    running: Vec<String>,
    /// Codes of the installed layouts OpenSwitch can switch between (EN, RU, UA, BY).
    layout_codes: Vec<&'static str>,
    test_text: String,
    status: Option<(String, bool, Instant)>,
    /// Index into HOTKEYS of the hotkey being recorded.
    recording: Option<usize>,
    /// Keys that were already down when recording started (ignored until released).
    rec_ignore: Vec<u8>,
    rec_ping: Instant,
    rec_error: bool,
    confirm_reset: Option<Instant>,
    window_focused: bool,
    tray_running: bool,
    tray_checked: Instant,
    /// "Run as administrator" being turned on/off on a worker thread (the wanted state).
    admin_job: Option<(bool, std::thread::JoinHandle<Result<(), Failure>>)>,
}

impl SettingsApp {
    fn new() -> Self {
        let (s, _) = sys::load_settings();
        let words = load_words();
        let always = sys::always_lines();
        SettingsApp {
            saved: s.clone(),
            s,
            saved_words: words.clone(),
            words,
            saved_always: always.clone(),
            always,
            new_always: String::new(),
            autostart: sys::startup_enabled(),
            search: String::new(),
            active: Section::General,
            visible: Section::ALL.to_vec(),
            jump: None,
            new_exe: String::new(),
            new_word: String::new(),
            running: sys::window_exes(),
            layout_codes: sys::layout_codes(),
            test_text: String::new(),
            status: None,
            recording: None,
            rec_ignore: Vec::new(),
            rec_ping: Instant::now(),
            rec_error: false,
            confirm_reset: None,
            window_focused: true,
            tray_running: !sys::tray_window().is_null(),
            tray_checked: Instant::now(),
            admin_job: None,
        }
    }

    fn dirty(&self) -> bool {
        self.s.to_text() != self.saved.to_text() || self.words != self.saved_words || self.always != self.saved_always
    }

    /// Re-read the files (the tray menu or a text editor may have changed them).
    fn reload_from_disk(&mut self) {
        let (s, _) = sys::load_settings();
        self.saved = s.clone();
        self.s = s;
        self.words = load_words();
        self.saved_words = self.words.clone();
        self.always = sys::always_lines();
        self.saved_always = self.always.clone();
        self.autostart = sys::startup_enabled();
    }

    fn conflict(&self) -> bool {
        let h = [&self.s.hotkey_convert_word, &self.s.hotkey_convert_selection, &self.s.hotkey_toggle_auto];
        (0..h.len()).any(|i| (i + 1..h.len()).any(|j| h[i].vk != 0 && h[i] == h[j]))
    }

    /// Finish turning "run as administrator" on/off once the worker thread is done.
    fn poll_admin(&mut self, ctx: &egui::Context) {
        if self.admin_job.as_ref().is_some_and(|(_, j)| !j.is_finished()) {
            ctx.request_repaint_after(Duration::from_millis(200));
            return;
        }
        let Some((on, job)) = self.admin_job.take() else { return };
        let result = job.join().unwrap_or_else(|_| Err(Failure::Error("panic".into())));
        self.status = Some(match result {
            Ok(()) => {
                // The worker already wrote it to settings.ini.
                self.s.run_as_admin = on;
                self.saved.run_as_admin = on;
                (t("Changes applied").into(), true, Instant::now())
            }
            Err(Failure::Cancelled) => (t("Windows did not give the permission").into(), false, Instant::now()),
            Err(Failure::Error(e)) => {
                log::error!("run as administrator ({on}): {e}");
                (i18n::f("Could not change it: {}", &[&e]), false, Instant::now())
            }
        });
        self.autostart = sys::startup_enabled();
        self.tray_running = !sys::tray_window().is_null();
    }

    /// Write changed files and tell the tray app.
    fn save(&mut self) {
        let mut result = Ok(());
        if self.s.to_text() != self.saved.to_text() {
            result = sys::save_settings(&self.s);
        }
        if result.is_ok() && self.words != self.saved_words {
            result = sys::save_exceptions(&self.words);
        }
        if result.is_ok() && self.always != self.saved_always {
            result = sys::save_always(&self.always);
        }
        match result {
            Ok(()) => {
                self.saved = self.s.clone();
                self.saved_words = self.words.clone();
                self.tray_running = sys::notify_reload();
                self.status = Some((t("Changes applied").into(), true, Instant::now()));
            }
            Err(e) => {
                log::error!("saving settings failed: {e}");
                // Keep the edits; retry on the next change.
                self.saved = self.s.clone();
                self.saved_words = self.words.clone();
                self.status = Some((i18n::f("Could not save: {}", &[&e]), false, Instant::now()));
            }
        }
    }

    // ---------------------------------------------------------------- hotkey recording

    fn start_recording(&mut self, i: usize) {
        self.recording = Some(i);
        self.rec_ignore = (1..=254u8).filter(|&vk| key_down(vk)).collect();
        self.rec_error = false;
        self.rec_ping = Instant::now();
        sys::suspend_hotkeys(true);
    }

    fn stop_recording(&mut self) {
        if self.recording.take().is_some() {
            sys::suspend_hotkeys(false);
        }
    }

    /// Poll the keyboard while recording. The tray app lets keys through meanwhile, so Pause
    /// and other hotkeys reach this window; the state is read straight from Windows because
    /// egui does not know keys like Pause or ScrollLock.
    fn poll_recording(&mut self, ctx: &egui::Context) {
        let Some(i) = self.recording else { return };
        ctx.request_repaint_after(Duration::from_millis(15));
        // A focused button would react to Space/Enter: keep focus away while recording.
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
        if self.rec_ping.elapsed() > Duration::from_secs(1) {
            self.rec_ping = Instant::now();
            sys::suspend_hotkeys(true);
        }
        self.rec_ignore.retain(|&vk| key_down(vk));
        if key_down(0x1B) {
            self.stop_recording(); // Esc
            return;
        }
        let pressed =
            (1..=254u8).find(|&vk| !is_mouse_or_modifier(vk) && !self.rec_ignore.contains(&vk) && key_down(vk));
        let Some(mut vk) = pressed else { return };
        if vk == VK_CANCEL {
            vk = VK_PAUSE; // Ctrl+Pause arrives as Ctrl+Break
        }
        if vk == 0x08 || vk == 0x2E {
            *hotkey_mut(&mut self.s, i) = Hotkey::NONE; // Backspace / Delete: no hotkey
            self.stop_recording();
            return;
        }
        if !is_hotkey_key(vk) {
            self.rec_error = true;
            self.rec_ignore.push(vk);
            return;
        }
        *hotkey_mut(&mut self.s, i) = Hotkey {
            ctrl: key_down(0x11),
            alt: key_down(0x12),
            shift: key_down(0x10),
            win: key_down(0x5B) || key_down(0x5C),
            vk,
        };
        self.stop_recording();
    }

    // ---------------------------------------------------------------- sections

    fn general(&mut self, ui: &mut egui::Ui, p: &mut Page) {
        p.begin(Section::General);
        let label = t("Interface language");
        if p.row(ui, &[label, "language"]) {
            setting_row(ui, label, None, |ui| {
                egui::ComboBox::from_id_salt("language")
                    .selected_text(language_label(&self.s.language))
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        let system = format!("{} ({})", t("System"), i18n::system().native());
                        ui.selectable_value(&mut self.s.language, "system".to_string(), system);
                        for lang in Lang::ALL {
                            ui.selectable_value(&mut self.s.language, lang.code().to_string(), lang.native());
                        }
                    });
            });
        }

        let label = t("Start OpenSwitch with Windows");
        if p.row(ui, &[label, "autostart"]) {
            if self.s.run_as_admin {
                // The administrator task starts it at logon.
                let hint = t("With administrator rights, through the Task Scheduler");
                setting_row(ui, label, Some(hint), |ui| ui.add_enabled_ui(false, |ui| toggle(ui, &mut true)));
            } else {
                setting_row(ui, label, None, |ui| {
                    if toggle(ui, &mut self.autostart).changed() {
                        sys::set_startup(self.autostart);
                    }
                });
            }
        }

        let (label, hint) = (
            t("Run as administrator"),
            t("Needed to fix typing in programs started as administrator. Windows asks for permission once, then OpenSwitch starts with Windows without asking."),
        );
        if p.row(ui, &[label, hint, "admin", "UAC"]) {
            setting_row(ui, label, Some(hint), |ui| {
                if self.admin_job.is_some() {
                    ui.spinner();
                } else {
                    let mut on = self.s.run_as_admin;
                    if toggle(ui, &mut on).changed() {
                        self.admin_job = Some((on, std::thread::spawn(move || admin::switch(on))));
                    }
                }
            });
        }

        let (label, hint) = (t("Undo a fix with Backspace"), t("Backspace right after a fix brings the word back"));
        if p.row(ui, &[label, hint]) {
            setting_row(ui, label, Some(hint), |ui| toggle(ui, &mut self.s.undo_backspace));
        }

        let (label, hint) = (t("Try it here"), t("Type «ghbdtn» and a space, it should become «привет»"));
        if p.row(ui, &[label, hint]) {
            ui.add_space(6.0);
            ui.label(label);
            ui.label(RichText::new(hint).small().weak());
            ui.add(egui::TextEdit::multiline(&mut self.test_text).desired_rows(2).desired_width(f32::INFINITY));
        }
    }

    fn layout_module(&mut self, ui: &mut egui::Ui, p: &mut Page) {
        p.begin(Section::Layout);
        let (label, hint) = (
            t("Module «Layout»"),
            t("Fixes words typed in the wrong layout and switches the layout. Hotkeys and double Shift belong to it too."),
        );
        if p.row(ui, &[label, hint]) {
            module_row(ui, label, hint, &mut self.s.module_layout);
        }
        if !self.s.module_layout {
            return;
        }

        let (label, hint) =
            (t("Fix the layout automatically"), t("A word typed in the wrong layout is fixed after Space"));
        if p.row(ui, &[label, hint]) {
            setting_row(ui, label, Some(hint), |ui| toggle(ui, &mut self.s.auto_switch));
        }

        let (label, hint) = (t("How boldly to fix"), t("Cautious: fewer mistakes, bold: fixes more words"));
        if p.row(ui, &[label, hint, t("Sensitivity")]) {
            setting_row(ui, label, Some(hint), |ui| {
                ui.add_enabled_ui(self.s.auto_switch, |ui| {
                    // right-to-left layout: last preset first
                    for (value, key) in SENSITIVITY_PRESETS.iter().rev() {
                        let on = (self.s.sensitivity - value).abs() < 0.01;
                        if ui.selectable_label(on, t(key)).clicked() {
                            self.s.sensitivity = *value;
                        }
                    }
                });
            });
        }

        let label = t("Switch between these layouts");
        let hint = t("Turn a layout off and words are never switched to it automatically (Pause still can).");
        let names: Vec<String> = self.layout_codes.iter().map(|c| layout_name(c)).collect();
        let mut texts: Vec<&str> = vec![label, hint, t("Layouts"), "EN", "RU", "UA", "BY"];
        texts.extend(names.iter().map(|s| s.as_str()));
        if p.row(ui, &texts) {
            ui.add_space(8.0);
            ui.label(label);
            ui.label(RichText::new(hint).small().weak());
            if self.layout_codes.len() < 2 {
                ui.add_space(4.0);
                ui.colored_label(
                    RED,
                    t("Windows has fewer than two layouts OpenSwitch knows (English, Russian, Ukrainian, Belarusian). Add one in Windows settings → Time and language → Language."),
                );
            }
            for (code, name) in self.layout_codes.clone().into_iter().zip(names) {
                setting_row(ui, &name, None, |ui| {
                    let mut on = !self.s.disabled_layouts.contains(code);
                    if toggle(ui, &mut on).changed() {
                        if on {
                            self.s.disabled_layouts.remove(code);
                        } else {
                            self.s.disabled_layouts.insert(code.to_string());
                        }
                    }
                });
            }
        }

        // Fine tuning: folded away normally, shown flat while searching.
        let fine = |this: &mut Self, ui: &mut egui::Ui, p: &mut Page| {
            let label = t("Check the word on Enter too (not only on Space)");
            if p.row(ui, &[label]) {
                setting_row(ui, label, None, |ui| toggle(ui, &mut this.s.switch_on_enter));
            }
            let label = t("Fix one-letter words (d — в, z — я, ш — i)");
            if p.row(ui, &[label]) {
                setting_row(ui, label, None, |ui| toggle(ui, &mut this.s.single_letters));
            }
            let (label, hint) = (t("Don't touch links, e-mail and code"), t("github.com, readme.md, getValue"));
            if p.row(ui, &[label, hint, "url"]) {
                setting_row(ui, label, Some(hint), |ui| toggle(ui, &mut this.s.skip_code));
            }
            let (label, hint) = (
                t("Be careful after you switch the layout yourself"),
                t("The first word after a manual switch is fixed only when it is clearly wrong"),
            );
            if p.row(ui, &[label, hint]) {
                setting_row(ui, label, Some(hint), |ui| toggle(ui, &mut this.s.careful_after_switch));
            }
            let label = t("Exact sensitivity");
            if p.row(ui, &[label, t("Sensitivity")]) {
                setting_row(ui, label, None, |ui| {
                    if (this.s.sensitivity - 1.0).abs() > 0.01 && ui.small_button(t("default")).clicked() {
                        this.s.sensitivity = 1.0;
                    }
                    ui.add(egui::Slider::new(&mut this.s.sensitivity, 0.5..=2.0).step_by(0.05).fixed_decimals(2));
                });
            }
        };
        if p.filtering() {
            fine(self, ui, p);
        } else {
            ui.add_space(4.0);
            egui::CollapsingHeader::new(RichText::new(t("Fine tuning")).weak()).id_salt("fine").show(ui, |ui| {
                fine(self, ui, p);
            });
        }
    }

    fn text_module(&mut self, ui: &mut egui::Ui, p: &mut Page) {
        p.begin(Section::Text);
        let (label, hint) = (
            t("Module «Text»"),
            t("Small fixes by simple rules: capital letters, an accidental CapsLock, a space before a mark."),
        );
        if p.row(ui, &[label, hint]) {
            module_row(ui, label, hint, &mut self.s.module_text);
        }
        if !self.s.module_text {
            return;
        }
        let (label, hint) =
            (t("Capital letter at the start of a sentence"), t("After . ! ? and Enter; English i becomes I"));
        if p.row(ui, &[label, hint]) {
            setting_row(ui, label, Some(hint), |ui| toggle(ui, &mut self.s.capitalize));
        }
        let (label, hint) = (t("Remove the space before , . ! ?"), t("«привет , как» becomes «привет, как»"));
        if p.row(ui, &[label, hint]) {
            setting_row(ui, label, Some(hint), |ui| toggle(ui, &mut self.s.fix_space));
        }
        let (label, hint) = (t("Fix accidental CapsLock"), t("пРИВЕТ becomes Привет and CapsLock turns off"));
        if p.row(ui, &[label, hint, "caps"]) {
            setting_row(ui, label, Some(hint), |ui| toggle(ui, &mut self.s.fix_caps));
        }
    }

    fn spelling_module(&mut self, ui: &mut egui::Ui, p: &mut Page) {
        p.begin(Section::Spelling);
        let (label, hint) = (
            t("Module «Spelling»"),
            t("Fixes typos with the built-in dictionaries: «првиет» becomes «привет». Only when it is at least 85 % sure; words of 5+ letters."),
        );
        if p.row(ui, &[label, hint, "typo"]) {
            module_row(ui, label, hint, &mut self.s.module_spelling);
        }
        if !self.s.module_spelling {
            return;
        }
        let note = t("Backspace right after a fix brings the word back; undo the same word twice and it is never fixed again (see «Words»).");
        if p.row(ui, &[note]) {
            ui.add_space(4.0);
            ui.label(RichText::new(note).small().weak());
        }
    }

    fn hotkeys(&mut self, ui: &mut egui::Ui, p: &mut Page) {
        p.begin(Section::Hotkeys);
        for (i, key) in HOTKEYS.iter().enumerate() {
            let label = t(key);
            let current = hotkey_mut(&mut self.s, i).to_string();
            if !p.row(ui, &[label, &current]) {
                continue;
            }
            let hint = (i == 0).then(|| t("Press again to undo"));
            let mut start = false;
            let mut clear = false;
            setting_row(ui, label, hint, |ui| {
                if self.recording == Some(i) {
                    let text =
                        if self.rec_error { t("This key can't be used, try another") } else { t("Press keys…") };
                    let b = egui::Button::new(RichText::new(text).color(ui.visuals().selection.stroke.color))
                        .selected(true)
                        .min_size(egui::vec2(170.0, 0.0));
                    ui.add(b).on_hover_text(t("Esc: cancel, Backspace: no hotkey"));
                } else {
                    let hk = *hotkey_mut(&mut self.s, i);
                    if hk.vk != 0 && ui.small_button("×").on_hover_text(t("No hotkey")).clicked() {
                        clear = true;
                    }
                    let text = if hk.vk == 0 { t("None").to_string() } else { hk.to_string() };
                    let b = egui::Button::new(RichText::new(text).monospace()).min_size(egui::vec2(170.0, 0.0));
                    start = ui.add(b).on_hover_text(t("Click and press the key combination")).clicked();
                }
            });
            if clear {
                *hotkey_mut(&mut self.s, i) = Hotkey::NONE;
            }
            if start {
                self.stop_recording();
                self.start_recording(i);
            }
        }
        if self.conflict() && p.header_drawn {
            ui.colored_label(RED, t("Two commands are assigned to the same key combination."));
        }

        let (label, hint) = (t("Double Shift fixes the last word"), t("Handy on laptops without the Pause key"));
        if p.row(ui, &[label, hint]) {
            setting_row(ui, label, Some(hint), |ui| toggle(ui, &mut self.s.double_shift));
        }

        let label = t("Restore default keys");
        if p.row(ui, &[label]) {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button(label).clicked() {
                    let d = Settings::default();
                    self.s.hotkey_convert_word = d.hotkey_convert_word;
                    self.s.hotkey_convert_selection = d.hotkey_convert_selection;
                    self.s.hotkey_toggle_auto = d.hotkey_toggle_auto;
                    self.s.double_shift = d.double_shift;
                }
                ui.label(RichText::new(t("Ctrl+Pause works too (Windows calls it Break).")).small().weak());
            });
        }
    }

    fn programs(&mut self, ui: &mut egui::Ui, p: &mut Page) {
        p.begin(Section::Programs);
        let (label, hint) = (
            t("Do nothing in password fields"),
            t("Nothing is fixed or remembered while you type a password (also in browsers)"),
        );
        if p.row(ui, &[label, hint]) {
            setting_row(ui, label, Some(hint), |ui| toggle(ui, &mut self.s.skip_passwords));
            ui.add_space(6.0);
        }
        let mut list: Vec<String> = self.s.exclude.iter().cloned().collect();
        list.sort();
        let label = t("Programs without auto switching");
        let hint =
            t("Words are not fixed automatically in these programs, but hotkeys work. Handy for terminals, code editors and games.");
        let mut texts: Vec<&str> = vec![label, hint];
        texts.extend(list.iter().map(|s| s.as_str()));
        if !p.row(ui, &texts) {
            return;
        }
        ui.label(label);
        ui.label(RichText::new(hint).small().weak());
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            let mut picked: Option<String> = None;
            egui::ComboBox::from_id_salt("running").selected_text(t("Choose from open windows…")).width(230.0).show_ui(
                ui,
                |ui| {
                    for e in &self.running {
                        if !self.s.exclude.contains(e) && ui.selectable_label(false, e).clicked() {
                            picked = Some(e.clone());
                        }
                    }
                },
            );
            if ui.small_button("⟳").on_hover_text(t("Update window list")).clicked() {
                self.running = sys::window_exes();
            }
            if let Some(name) = picked {
                add_exe(&mut self.s, &name);
            }
            ui.label(RichText::new(t("or")).weak());
            let r = ui
                .add(egui::TextEdit::singleline(&mut self.new_exe).hint_text(t("e.g. code.exe")).desired_width(130.0));
            let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if ui.button(t("Add")).clicked() || enter {
                add_exe(&mut self.s, &self.new_exe);
                self.new_exe.clear();
            }
        });
        ui.add_space(4.0);
        let shown: Vec<&String> =
            list.iter().filter(|e| p.all || !p.filtering() || p.hit(e) || p.hit(label) || p.hit(hint)).collect();
        if let Some(remove) = removable_list(ui, &shown, t("List is empty")) {
            self.s.exclude.remove(&remove);
        }
    }

    fn words(&mut self, ui: &mut egui::Ui, p: &mut Page) {
        p.begin(Section::Words);
        let (label, hint) =
            (t("Learn from my fixes"), t("A word you fixed by hand with a hotkey is fixed automatically next time"));
        if p.row(ui, &[label, hint]) {
            setting_row(ui, label, Some(hint), |ui| toggle(ui, &mut self.s.learn_manual));
        }
        word_list(
            ui,
            p,
            t("Never fix"),
            &[
                t("These words are never fixed automatically. Write them the way you type them (e.g. «ctx» or «git»)."),
                t("A word is added here by itself when you undo its automatic fix."),
            ],
            &mut self.words,
            &mut self.new_word,
        );
        word_list(
            ui,
            p,
            t("Always fix"),
            &[
                t("These words are always fixed. Write them the way you type them by mistake (e.g. «ghbdtn»)."),
                t("Words you fixed by hand get here by themselves."),
            ],
            &mut self.always,
            &mut self.new_always,
        );
    }

    fn about(&mut self, ui: &mut egui::Ui, p: &mut Page) {
        p.begin(Section::About);
        let version = format!("OpenSwitch {}", env!("CARGO_PKG_VERSION"));
        let what = t("Free automatic keyboard layout switcher for Windows.");
        if p.row(ui, &[&version, what, t("License: GNU GPL v3.0 or later."), "github"]) {
            ui.label(RichText::new(&version).strong());
            ui.label(what);
            ui.label(RichText::new(t("• Works offline, sends nothing anywhere.")).weak());
            ui.label(RichText::new(t("• Dictionaries: the wordfreq project (CC BY-SA 4.0).")).weak());
            ui.label(RichText::new(t("License: GNU GPL v3.0 or later.")).weak());
            ui.hyperlink_to(t("Source code on GitHub"), "https://github.com/DezFix/OpenSwitch");
        }

        let (label, hint) = (t("First-run guide"), t("How to use OpenSwitch and which modules to turn on"));
        if p.row(ui, &[label, hint, "guide", "help"]) {
            setting_row(ui, label, Some(hint), |ui| {
                if ui.button(t("Show")).clicked() {
                    sys::spawn_self(&["--setup"]);
                }
            });
        }

        let (label, hint) = (t("Program files"), t("Settings, exception words and the log for bug reports"));
        if p.row(ui, &[label, hint, "log", "settings.ini"]) {
            setting_row(ui, label, Some(hint), |ui| {
                if ui.button(t("Log")).clicked() {
                    sys::open_path(&logging::log_dir());
                }
                if ui.button(t("Settings")).clicked() {
                    sys::open_path(&sys::data_dir());
                }
            });
        }

        let (label, hint) = (t("Reset settings"), t("Exception words are kept"));
        if p.row(ui, &[label, hint]) {
            setting_row(ui, label, Some(hint), |ui| {
                let armed = self.confirm_reset.is_some_and(|at| at.elapsed() < Duration::from_secs(4));
                let text = if armed { t("Click again to reset") } else { t("Reset") };
                let b = egui::Button::new(if armed { RichText::new(text).color(RED) } else { RichText::new(text) });
                if ui.add(b).clicked() {
                    if armed {
                        self.s = Settings::default();
                        self.confirm_reset = None;
                    } else {
                        self.confirm_reset = Some(Instant::now());
                        ui.ctx().request_repaint_after(Duration::from_secs(4));
                    }
                }
            });
        }
    }

    // ---------------------------------------------------------------- frame

    fn sidebar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        let search = egui::TextEdit::singleline(&mut self.search)
            .hint_text(format!("🔍 {}", t("Search")))
            .desired_width(f32::INFINITY);
        let r = ui.add(search);
        if r.changed() {
            self.jump = None;
        }
        if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.search.clear();
        }
        ui.add_space(10.0);
        for sec in Section::ALL {
            let shown = self.visible.contains(&sec);
            let text = RichText::new(sec.title()).size(15.0);
            let item = egui::Button::selectable(self.active == sec && shown, text)
                .min_size(egui::vec2(ui.available_width(), 30.0));
            let r = ui.add_enabled(shown, item);
            if r.clicked() {
                self.active = sec;
                self.jump = Some((sec, Instant::now()));
            }
        }
        if self.visible.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new(t("Nothing found")).weak());
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(10.0);
            if let Some((msg, ok, at)) = &self.status {
                let ttl = if *ok { 3 } else { 10 };
                if at.elapsed() < Duration::from_secs(ttl) {
                    ui.add(egui::Label::new(RichText::new(msg).small().color(if *ok { GREEN } else { RED })).wrap());
                    ui.ctx().request_repaint_after(Duration::from_millis(500));
                }
            }
        });
    }

    fn page(&mut self, ui: &mut egui::Ui) {
        if !self.tray_running {
            egui::Frame::new()
                .fill(ui.visuals().warn_fg_color.gamma_multiply(0.12))
                .inner_margin(10.0)
                .corner_radius(6.0)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(t("OpenSwitch is not running, so these settings don't work yet."));
                        if ui.button(t("Run OpenSwitch")).clicked() {
                            sys::spawn_self(&[]);
                            self.tray_checked = Instant::now() - Duration::from_secs(5);
                        }
                    });
                });
            ui.add_space(6.0);
        }

        let jump = self.jump.filter(|(_, at)| at.elapsed() < Duration::from_millis(JUMP_MS));
        let mut p = Page {
            query: self.search.trim().to_lowercase(),
            section: Section::General,
            all: false,
            header_drawn: false,
            tops: Vec::new(),
            // Scroll only in the frame after the click.
            jump: jump.filter(|(_, at)| at.elapsed() < Duration::from_millis(50)).map(|(s, _)| s),
        };
        let out = egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(ui.available_width() - 12.0);
            self.general(ui, &mut p);
            self.layout_module(ui, &mut p);
            self.text_module(ui, &mut p);
            self.spelling_module(ui, &mut p);
            self.hotkeys(ui, &mut p);
            self.programs(ui, &mut p);
            self.words(ui, &mut p);
            self.about(ui, &mut p);
            // Room at the end so the last section can scroll up to the top too.
            if let Some((_, top)) = p.tops.last() {
                let last = ui.min_rect().bottom() - top;
                ui.add_space((ui.clip_rect().height() - last - 20.0).max(0.0));
            }
        });
        if jump.is_some() {
            ui.ctx().request_repaint();
        }

        self.visible = p.tops.iter().map(|(s, _)| *s).collect();
        // The menu follows the scroll position: the last section whose header is in the upper
        // third of the view (or above it) is the current one.
        let top = out.inner_rect.top() + out.inner_rect.height() / 3.0;
        let current = p.tops.iter().take_while(|(_, y)| *y <= top).last().or(p.tops.first()).map(|(s, _)| *s);
        match (jump, current) {
            (Some((s, _)), _) => self.active = s,
            (None, Some(s)) => self.active = s,
            _ => {}
        }
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let lang = i18n::resolve(&self.s.language);
        if lang != i18n::current() {
            i18n::set(lang);
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(t("OpenSwitch — Settings").into()));
        }

        // Back in the window: pick up changes made meanwhile (tray menu, hand edits).
        let focused = ctx.input(|i| i.focused);
        if focused && !self.window_focused {
            if !self.dirty() {
                self.reload_from_disk();
            }
            self.running = sys::window_exes();
            self.layout_codes = sys::layout_codes();
        }
        if !focused && self.window_focused {
            self.stop_recording();
        }
        self.window_focused = focused;
        if self.tray_checked.elapsed() > Duration::from_secs(2) {
            self.tray_checked = Instant::now();
            self.tray_running = !sys::tray_window().is_null();
        }
        ctx.request_repaint_after(Duration::from_secs(2));

        self.poll_recording(&ctx);
        self.poll_admin(&ctx);

        egui::Panel::left("nav").exact_size(SIDEBAR_WIDTH).resizable(false).show(ui, |ui| self.sidebar(ui));
        egui::CentralPanel::default().show(ui, |ui| self.page(ui));

        // Apply changes right away, but not in the middle of dragging a slider.
        let busy = ctx.input(|i| i.pointer.any_down()) || self.recording.is_some() || self.admin_job.is_some();
        if !busy && self.dirty() {
            if self.conflict() {
                if self.status.as_ref().is_none_or(|(_, ok, _)| *ok) {
                    self.status =
                        Some((t("Not saved: two commands have the same keys.").into(), false, Instant::now()));
                }
            } else {
                self.save();
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.stop_recording();
    }
}

// ---------------------------------------------------------------- widgets

/// A row: label (and a small hint under it) on the left, the control on the right.
pub(crate) fn setting_row<R>(
    ui: &mut egui::Ui,
    label: &str,
    hint: Option<&str>,
    control: impl FnOnce(&mut egui::Ui) -> R,
) {
    ui.add_space(4.0);
    let height = if hint.is_some() { 38.0 } else { 26.0 };
    egui::Sides::new().height(height).shrink_left().wrap().show(
        ui,
        |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                ui.label(label);
                if let Some(h) = hint {
                    ui.label(RichText::new(h).small().weak());
                }
            });
        },
        control,
    );
    ui.add_space(4.0);
    let y = ui.cursor().top();
    let x = ui.max_rect().x_range();
    ui.painter().hline(x, y, ui.visuals().widgets.noninteractive.bg_stroke);
}

/// The on/off row of a module: bigger, with a description under it.
pub(crate) fn module_row(ui: &mut egui::Ui, label: &str, hint: &str, on: &mut bool) {
    ui.add_space(4.0);
    egui::Sides::new().height(44.0).shrink_left().wrap().show(
        ui,
        |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(label).strong().size(15.5));
                ui.label(RichText::new(hint).small().weak());
            });
        },
        |ui| toggle(ui, on),
    );
    ui.add_space(6.0);
    let y = ui.cursor().top();
    let x = ui.max_rect().x_range();
    ui.painter().hline(x, y, ui.visuals().widgets.noninteractive.bg_stroke);
}

/// On/off switch.
pub(crate) fn toggle(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let (rect, mut r) = ui.allocate_exact_size(egui::vec2(38.0, 21.0), Sense::click());
    if r.clicked() {
        *on = !*on;
        r.mark_changed();
    }
    r.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, ui.is_enabled(), *on, ""));
    if ui.is_rect_visible(rect) {
        let how_on = ui.ctx().animate_bool_responsive(r.id, *on);
        let v = ui.style().interact_selectable(&r, *on);
        let radius = rect.height() / 2.0;
        let fill = if *on { ui.visuals().selection.bg_fill } else { ui.visuals().widgets.inactive.bg_fill };
        ui.painter().rect(rect, radius, fill, v.bg_stroke, egui::StrokeKind::Inside);
        let x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), how_on);
        let knob = if *on { Color32::WHITE } else { v.fg_stroke.color };
        ui.painter().circle_filled(egui::pos2(x, rect.center().y), radius - 3.5, knob);
    }
    r
}

/// A word list block: title, hints, an input to add words and the chips.
fn word_list(
    ui: &mut egui::Ui,
    p: &mut Page,
    label: &str,
    hints: &[&str],
    words: &mut Vec<String>,
    new_word: &mut String,
) {
    let mut texts: Vec<&str> = vec![label];
    texts.extend(hints);
    texts.extend(words.iter().map(|s| s.as_str()));
    if !p.row(ui, &texts) {
        return;
    }
    ui.add_space(8.0);
    ui.label(label);
    for h in hints {
        ui.label(RichText::new(*h).small().weak());
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let r = ui.add(egui::TextEdit::singleline(new_word).hint_text(t("new word")).desired_width(180.0));
        let enter = r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if ui.button(t("Add")).clicked() || enter {
            let wd = new_word.trim().to_lowercase();
            if !wd.is_empty() && !words.contains(&wd) {
                words.push(wd);
                words.sort();
            }
            new_word.clear();
        }
    });
    ui.add_space(4.0);
    let general = p.all || !p.filtering() || p.hit(label) || hints.iter().any(|h| p.hit(h));
    let shown: Vec<&String> = words.iter().filter(|w| general || p.hit(w)).collect();
    if let Some(remove) = removable_list(ui, &shown, t("No words yet")) {
        words.retain(|w| *w != remove);
    }
}

/// Items as compact chips with a remove cross, wrapped over lines (no inner scrolling: the
/// page itself scrolls). Returns the item to remove.
fn removable_list(ui: &mut egui::Ui, items: &[&String], empty: &str) -> Option<String> {
    let mut remove = None;
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        if items.is_empty() {
            ui.label(RichText::new(empty).weak());
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);
            for item in items {
                // learned words carry the target layout: "ghbdsn UA" -> "ghbdsn → UA"
                let text = format!("{}  ×", item.replacen(' ', " → ", 1));
                let chip = egui::Button::new(text).corner_radius(10.0);
                if ui.add(chip).on_hover_text(t("Remove")).clicked() {
                    remove = Some((*item).clone());
                }
            }
        });
    });
    remove
}

fn load_words() -> Vec<String> {
    sorted(sys::load_exceptions())
}

fn sorted(set: std::collections::HashSet<String>) -> Vec<String> {
    let mut words: Vec<String> = set.into_iter().collect();
    words.sort();
    words
}

fn key_down(vk: u8) -> bool {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
    unsafe { (GetAsyncKeyState(vk as i32) as u16 & 0x8000) != 0 }
}

fn is_mouse_or_modifier(vk: u8) -> bool {
    matches!(vk, 0x01 | 0x02 | 0x04..=0x06 | 0x10..=0x12 | 0x5B | 0x5C | 0xA0..=0xA5)
}

/// "Ukrainian (UA)" for a layout code.
pub(crate) fn layout_name(code: &str) -> String {
    let name = match code {
        "EN" => t("English"),
        "RU" => t("Russian"),
        "UA" => t("Ukrainian"),
        "BY" => t("Belarusian"),
        _ => code,
    };
    format!("{name} ({code})")
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

/// Window icon: the application icon (assets/openswitch.svg) rendered at 64x64, raw RGBA.
pub(crate) fn app_icon() -> egui::IconData {
    const RGBA: &[u8] = include_bytes!("../assets/icon-64.rgba");
    egui::IconData { rgba: RGBA.to_vec(), width: 64, height: 64 }
}

pub(crate) fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    if let Ok(bytes) = std::fs::read(format!("{windir}\\Fonts\\segoeui.ttf")) {
        fonts.font_data.insert("segoe".into(), Arc::new(egui::FontData::from_owned(bytes)));
        if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            f.insert(0, "segoe".into());
        }
    }
    // Symbols fallback (arrows, crosses, the search glass) after the main font.
    for file in ["seguisym.ttf", "seguiemj.ttf"] {
        if let Ok(bytes) = std::fs::read(format!("{windir}\\Fonts\\{file}")) {
            fonts.font_data.insert(file.into(), Arc::new(egui::FontData::from_owned(bytes)));
            if let Some(f) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
                let pos = f.iter().position(|s| s == "segoe").map_or(0, |i| i + 1);
                f.insert(pos, file.into());
            }
        }
    }
    ctx.set_fonts(fonts);
    ctx.global_style_mut(|s| {
        use egui::{FontId, TextStyle};
        s.text_styles.insert(TextStyle::Body, FontId::proportional(14.5));
        s.text_styles.insert(TextStyle::Button, FontId::proportional(14.5));
        s.text_styles.insert(TextStyle::Small, FontId::proportional(12.5));
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
            .with_inner_size([780.0, 580.0])
            .with_min_inner_size([620.0, 420.0])
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
