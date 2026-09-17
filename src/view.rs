use crate::app::{PERM_DENIED, Screen, SpeakEasyApp};
use crate::quality::OutputQuality;
use crate::widgets::{
    ACCENT, MUTED, field_label, full_combo, padded_choice, section_title, wide_button,
};
use eframe::egui::{self, Align, Align2, Color32, Frame, Layout, RichText, TextEdit};
use std::sync::atomic::Ordering;

impl SpeakEasyApp {
    pub(crate) fn draw(&mut self, ui: &mut egui::Ui) {
        match &self.screen {
            Screen::KeyGate => self.draw_gate(ui),
            Screen::Home => self.draw_home(ui),
            Screen::NewProject => self.draw_new_project(ui),
            Screen::Topic { project_id } => {
                let project_id = project_id.clone();
                self.draw_topic(ui, &project_id);
            }
            Screen::Studio { project_id, selected } => {
                let project_id = project_id.clone();
                let selected = *selected;
                self.draw_studio(ui, &project_id, selected);
            }
            Screen::Finish { project_id } => {
                let project_id = project_id.clone();
                self.draw_finish(ui, &project_id);
            }
        }
        if !matches!(self.screen, Screen::KeyGate) {
            self.draw_settings_window(ui.ctx());
        }
    }

    pub(crate) fn draw_header(&mut self, ui: &mut egui::Ui, show_settings: bool) {
        ui.horizontal(|ui| {
            ui.heading(RichText::new("SpeakEasy").size(26.0).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if show_settings
                    && ui
                        .add(egui::Button::new(RichText::new("⚙").size(22.0)).frame(false))
                        .on_hover_text("Settings")
                        .clicked()
                {
                    self.settings_open = !self.settings_open;
                }
            });
        });
    }

    fn draw_gate(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(48.0);
            ui.vertical_centered(|ui| {
                ui.heading(RichText::new("SpeakEasy").size(32.0).strong());
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Paste a Gemini API key to continue. SpeakEasy uses Gemini 2.5 Flash for questions and transcription.")
                        .color(MUTED),
                );
            });
            ui.add_space(28.0);
            ui.horizontal(|ui| {
                let pad = ((ui.available_width() - 420.0) / 2.0).max(0.0);
                ui.add_space(pad);
                ui.vertical(|ui| {
                    ui.set_max_width(420.0);
                    field_label(ui, "Gemini API key");
                    ui.add(
                        TextEdit::singleline(&mut self.gate_key)
                            .password(true)
                            .desired_width(f32::INFINITY)
                            .margin(egui::vec2(10.0, 10.0)),
                    );
                    if let Some(err) = &self.gate_error {
                        ui.add_space(8.0);
                        ui.colored_label(Color32::from_rgb(232, 160, 80), err);
                    }
                    ui.add_space(16.0);
                    if wide_button(
                        ui,
                        if self.busy { "Checking key…" } else { "Continue" },
                        ACCENT,
                        !self.busy && !self.gate_key.trim().is_empty(),
                    ) {
                        self.submit_key();
                    }
                });
            });
        });
    }

    fn draw_new_project(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_header(ui, true);
            ui.add_space(20.0);
            field_label(ui, "Project name");
            ui.add(
                TextEdit::singleline(&mut self.new_title)
                    .desired_width(f32::INFINITY)
                    .margin(egui::vec2(10.0, 10.0)),
            );
            if let Some(err) = &self.form_error {
                ui.add_space(8.0);
                ui.colored_label(Color32::from_rgb(232, 160, 80), err);
            }
            ui.add_space(16.0);
            if wide_button(ui, "Create project", ACCENT, !self.new_title.trim().is_empty()) {
                self.create_project();
            }
            ui.add_space(8.0);
            if ui.button("Cancel").clicked() {
                self.form_error = None;
                self.screen = Screen::Home;
            }
        });
    }

    fn draw_topic(&mut self, ui: &mut egui::Ui, _project_id: &str) {
        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_header(ui, true);
            ui.add_space(20.0);
            ui.label(RichText::new("What would you like to speak about?").size(20.0).strong());
            ui.add_space(10.0);
            ui.add(
                TextEdit::multiline(&mut self.topic_input)
                    .desired_width(f32::INFINITY)
                    .desired_rows(5)
                    .margin(egui::vec2(10.0, 10.0)),
            );
            if let Some(err) = &self.form_error {
                ui.add_space(8.0);
                ui.colored_label(Color32::from_rgb(232, 160, 80), err);
            }
            ui.add_space(16.0);
            if wide_button(
                ui,
                if self.busy {
                    "Generating questions…"
                } else {
                    "Generate 10 questions"
                },
                ACCENT,
                !self.busy && !self.topic_input.trim().is_empty(),
            ) {
                self.submit_topic();
            }
            ui.add_space(8.0);
            if ui.add_enabled(!self.busy, egui::Button::new("Back")).clicked() {
                self.screen = Screen::Home;
            }
            if !self.status.is_empty() {
                ui.add_space(10.0);
                ui.label(RichText::new(&self.status).color(MUTED));
            }
        });
    }

    fn draw_settings_window(&mut self, ctx: &egui::Context) {
        let mut open = self.settings_open;
        egui::Window::new("Settings")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(420.0)
            .frame(
                Frame::window(&ctx.style_of(ctx.theme()))
                    .inner_margin(20)
                    .corner_radius(12.0)
                    .fill(Color32::from_rgb(28, 28, 32)),
            )
            .show(ctx, |ui| {
                ui.set_min_width(380.0);
                ui.spacing_mut().item_spacing.y = 10.0;
                ui.spacing_mut().button_padding = egui::vec2(12.0, 8.0);
                self.draw_settings_form(ui);
            });
        if self.settings_open && !open {
            self.persist_settings();
        }
        self.settings_open = open;
    }

    fn draw_settings_form(&mut self, ui: &mut egui::Ui) {
        let busy = self.recording.is_some() || self.saving;
        ui.add_enabled_ui(!busy, |ui| {
            section_title(ui, "Gemini");
            field_label(ui, "API key");
            let response = ui.add(
                TextEdit::singleline(&mut self.config.gemini_api_key)
                    .password(true)
                    .desired_width(f32::INFINITY)
                    .margin(egui::vec2(10.0, 8.0)),
            );
            if response.changed() {
                self.persist_settings();
            }

            ui.add_space(16.0);
            section_title(ui, "Sources");
            field_label(ui, "Video");
            full_combo(ui, "camera", self.camera_label(), |ui| {
                for camera in &self.cameras {
                    padded_choice(
                        ui,
                        &mut self.selected_camera,
                        Some(camera.index.clone()),
                        &camera.name,
                    );
                }
            });
            ui.add_space(4.0);
            if ui
                .add_sized(
                    [ui.available_width(), 34.0],
                    egui::Button::new("Refresh devices"),
                )
                .clicked()
            {
                self.camera = None;
                self.reload_devices();
            }

            ui.add_space(16.0);
            field_label(ui, "Microphone");
            let mic_label = self
                .selected_mic
                .clone()
                .unwrap_or_else(|| "No microphone".into());
            full_combo(ui, "mic", mic_label, |ui| {
                for mic in &self.mics {
                    padded_choice(ui, &mut self.selected_mic, Some(mic.name.clone()), &mic.name);
                }
            });

            ui.add_space(20.0);
            section_title(ui, "Output");
            field_label(ui, "Resolution");
            full_combo(ui, "quality", self.quality.label(), |ui| {
                for quality in OutputQuality::ALL {
                    padded_choice(ui, &mut self.quality, quality, quality.label());
                }
            });
            if let Some(side) = self.frame_side {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "Records a {side}×{side} square, then exports {}×{} landscape and {}×{side} portrait.",
                        side,
                        side * 9 / 16,
                        side * 9 / 16
                    ))
                    .size(12.0)
                    .color(MUTED),
                );
            }
        });
        if busy {
            ui.add_space(8.0);
            ui.label(
                RichText::new("Settings lock while recording or saving.")
                    .italics()
                    .color(MUTED),
            );
        }
    }
}

impl SpeakEasyApp {
    pub(crate) fn preview_placeholder(&self) -> &'static str {
        if self.permission.load(Ordering::Relaxed) == PERM_DENIED {
            "Video permission denied"
        } else if self.cameras.is_empty() {
            "No video source found"
        } else {
            "Waiting for video…"
        }
    }
}
