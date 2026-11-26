// SPDX-License-Identifier: GPL-3.0-or-later
//! First-run window (`openswitch.exe --setup`): a short guide in a few steps — what OpenSwitch
//! does, which modules to turn on, how to use it, a field to try it. Shown by the tray app on
//! the first start; can be opened again from Settings → About. Choices are saved right away,
//! like in the settings window; closing the window (or "Skip") marks the guide as done.

use std::time::Duration;

use eframe::egui::{self, RichText};
use openswitch::config::Settings;
use openswitch::i18n::{self, t};

use crate::sys;
use crate::ui::{app_icon, layout_name, module_row, setting_row, setup_fonts, toggle, RED};

const STEPS: usize = 5;

struct SetupApp {
    s: Settings,
    saved: Settings,
    autostart: bool,
    step: usize,
    test_text: String,
    layout_codes: Vec<&'static str>,
    done: bool,
}

impl SetupApp {
    fn new() -> Self {
        let (s, _) = sys::load_settings();
        SetupApp {
            saved: s.clone(),
            s,
            autostart: sys::startup_enabled(),
            step: 0,
            test_text: String::new(),
            layout_codes: sys::layout_codes(),
            done: false,
        }
    }

    fn save(&mut self) {
        if self.s.to_text() == self.saved.to_text() {
            return;
        }
        match sys::save_settings(&self.s) {
            Ok(()) => {
                self.saved = self.s.clone();
                sys::notify_reload();
            }
            Err(e) => log::error!("saving settings failed: {e}"),
        }
    }

    /// Mark the guide as seen (finish, skip or close).
    fn finish(&mut self) {
        if !self.done {
            self.done = true;
            self.s.setup_done = true;
            self.save();
        }
    }

    fn title(ui: &mut egui::Ui, text: &str) {
        ui.add_space(4.0);
        ui.label(RichText::new(text).strong().size(22.0));
        ui.add_space(10.0);
    }

    fn welcome(&mut self, ui: &mut egui::Ui) {
        Self::title(ui, t("Welcome to OpenSwitch"));
        ui.label(t("OpenSwitch fixes text typed in the wrong keyboard layout: type «ghbdtn» and a space, and you get «привет» — the layout switches by itself, so the next word is right."));
        ui.add_space(10.0);
        ui.label(t("It can also help with text and fix typos. You choose what it does on the next step."));
        ui.add_space(14.0);
        ui.label(RichText::new(t("Your privacy")).strong());
        for line in [
            t("• Works offline: nothing is ever sent anywhere."),
            t("• What you type is never saved or written to the log."),
            t("• No AI: only dictionaries and simple rules, right on your computer."),
            t("• Free and open source (GNU GPL)."),
        ] {
            ui.label(line);
        }
    }

    fn modules(&mut self, ui: &mut egui::Ui) {
        Self::title(ui, t("What should OpenSwitch do?"));
        ui.label(
            RichText::new(t("Each module works on its own. You can change this any time in the settings.")).weak(),
        );
        ui.add_space(8.0);
        module_row(
            ui,
            t("Module «Layout»"),
            t("Fixes words typed in the wrong layout and switches the layout. Hotkeys and double Shift belong to it too."),
            &mut self.s.module_layout,
        );
        module_row(
            ui,
            t("Module «Text»"),
            t("Small fixes by simple rules: capital letters, an accidental CapsLock, a space before a mark."),
            &mut self.s.module_text,
        );
        module_row(
            ui,
            t("Module «Spelling»"),
            t("Fixes typos with the built-in dictionaries: «првиет» becomes «привет». Only when it is at least 85 % sure; words of 5+ letters."),
            &mut self.s.module_spelling,
        );
    }

    fn how_to(&mut self, ui: &mut egui::Ui) {
        Self::title(ui, t("How to use it"));
        let rows = [
            (self.s.hotkey_convert_word.to_string(), t("fix the last word by hand (press again to put it back)")),
            ("Shift, Shift".to_string(), t("the same, for laptops without Pause")),
            (self.s.hotkey_convert_selection.to_string(), t("fix the layout of the selected text")),
            (self.s.hotkey_toggle_auto.to_string(), t("turn automatic fixing on or off")),
            ("Backspace".to_string(), t("right after a fix: undo it")),
        ];
        egui::Grid::new("keys").num_columns(2).spacing([18.0, 10.0]).show(ui, |ui| {
            for (key, what) in rows {
                ui.label(RichText::new(key).monospace().strong());
                ui.label(what);
                ui.end_row();
            }
        });
        ui.add_space(14.0);
        ui.label(RichText::new(t("The tray icon")).strong());
        ui.label(t("Shows the current layout (EN, RU, UA). Click it to turn automatic fixing on or off; right-click for the menu and Settings."));
        ui.add_space(10.0);
        ui.label(RichText::new(t("If a word is fixed wrongly")).strong());
        ui.label(t("Undo it with Backspace or Pause: after two undos the word is remembered and left alone. A word you fixed by hand is learned and fixed by itself next time."));
    }

    fn try_it(&mut self, ui: &mut egui::Ui) {
        Self::title(ui, t("Try it"));
        if self.layout_codes.len() < 2 {
            ui.colored_label(
                RED,
                t("Windows has fewer than two layouts OpenSwitch knows (English, Russian, Ukrainian, Belarusian). Add one in Windows settings → Time and language → Language."),
            );
            ui.add_space(6.0);
        } else {
            let names: Vec<String> = self.layout_codes.iter().map(|c| layout_name(c)).collect();
            ui.label(i18n::f("Your layouts: {}", &[&names.join(", ")]));
            ui.add_space(6.0);
        }
        ui.label(t("Type «ghbdtn» and a space, it should become «привет»"));
        ui.add(egui::TextEdit::multiline(&mut self.test_text).desired_rows(3).desired_width(f32::INFINITY));
        if sys::tray_window().is_null() {
            ui.add_space(4.0);
            ui.colored_label(RED, t("OpenSwitch is not running, so these settings don't work yet."));
            if ui.button(t("Run OpenSwitch")).clicked() {
                sys::spawn_self(&[]);
            }
        }
        ui.add_space(10.0);
        setting_row(ui, t("Start OpenSwitch with Windows"), None, |ui| {
            if toggle(ui, &mut self.autostart).changed() {
                sys::set_startup(self.autostart);
            }
        });
    }

    fn ready(&mut self, ui: &mut egui::Ui) {
        Self::title(ui, t("All set"));
        ui.label(t("OpenSwitch works in the background. Everything else — hotkeys, programs where it should not fix words, exception words — is in the settings: right-click the tray icon → Settings."));
        ui.add_space(10.0);
        ui.label(RichText::new(t("You can open this guide again in Settings → About.")).weak());
    }
}

impl eframe::App for SetupApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        egui::Panel::bottom("nav").show(ui, |ui| {
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                // step dots
                for i in 0..STEPS {
                    let c = if i == self.step {
                        ui.visuals().selection.bg_fill
                    } else {
                        ui.visuals().widgets.inactive.bg_fill
                    };
                    let (r, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                    ui.painter().circle_filled(r.center(), 4.0, c);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let last = self.step + 1 == STEPS;
                    let next = if last { t("Finish") } else { t("Next") };
                    if ui
                        .add(egui::Button::new(RichText::new(next).strong()).min_size(egui::vec2(96.0, 30.0)))
                        .clicked()
                    {
                        if last {
                            self.finish();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        } else {
                            self.step += 1;
                        }
                    }
                    if self.step > 0 && ui.add(egui::Button::new(t("Back")).min_size(egui::vec2(80.0, 30.0))).clicked()
                    {
                        self.step -= 1;
                    }
                    if !last && ui.add(egui::Button::new(RichText::new(t("Skip")).weak()).frame(false)).clicked() {
                        self.finish();
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
            ui.add_space(10.0);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.set_max_width(ui.available_width() - 12.0);
                ui.add_space(8.0);
                match self.step {
                    0 => self.welcome(ui),
                    1 => self.modules(ui),
                    2 => self.how_to(ui),
                    3 => self.try_it(ui),
                    _ => self.ready(ui),
                }
            });
        });
        if !ctx.input(|i| i.pointer.any_down()) {
            self.save();
        }
        ctx.request_repaint_after(Duration::from_secs(2));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.finish();
    }
}

pub fn run() {
    use windows_sys::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows_sys::Win32::System::Threading::CreateMutexW;
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE};

    let (settings, _) = sys::load_settings();
    i18n::set(i18n::resolve(&settings.language));
    let title = t("OpenSwitch — getting started");
    unsafe {
        let _m = CreateMutexW(std::ptr::null(), 1, sys::w("Local\\OpenSwitch.Setup").as_ptr());
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
            .with_inner_size([640.0, 520.0])
            .with_min_inner_size([520.0, 420.0])
            .with_icon(app_icon()),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        title,
        options,
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(SetupApp::new()))
        }),
    ) {
        // No OpenGL (virtual machines, RDP): do not ask again, the app works with the defaults.
        log::error!("setup window failed to start: {e}");
        let mut s = settings;
        s.setup_done = true;
        let _ = sys::save_settings(&s);
    }
}
