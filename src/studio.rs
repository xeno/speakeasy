use crate::app::{Screen, SpeakEasyApp};
use crate::preview::{HORIZONTAL, VERTICAL, draw_square_preview};
use crate::widgets::{ACCENT, MUTED, format_clock, wide_button};
use eframe::egui::{self, Color32, RichText};

impl SpeakEasyApp {
    pub(crate) fn draw_studio(&mut self, ui: &mut egui::Ui, project_id: &str, selected: usize) {
        let project = self.db.project(project_id).ok();
        let questions = self.db.questions(project_id).unwrap_or_default();
        let title = project
            .as_ref()
            .map(|project| project.title.clone())
            .unwrap_or_else(|| "Project".into());
        let current = questions
            .iter()
            .find(|row| row.position == selected as i64)
            .cloned();

        egui::Panel::left("questions")
            .exact_size(280.0)
            .resizable(false)
            .show(ui, |ui| {
                ui.add_space(8.0);
                if ui.add_enabled(self.recording.is_none(), egui::Button::new("← Projects")).clicked()
                {
                    self.camera = None;
                    self.player = None;
                    self.re_recording = false;
                    self.screen = Screen::Home;
                }
                ui.add_space(8.0);
                ui.label(RichText::new(title).size(18.0).strong());
                ui.add_space(10.0);
                let mut next_selected = None;
                for question in &questions {
                    let answered = question.answered();
                    let is_selected = question.position == selected as i64;
                    let label = format!(
                        "{} Q{}  {}",
                        if answered { "●" } else { "○" },
                        question.position,
                        question.text
                    );
                    let text = RichText::new(label).size(14.0);
                    if ui
                        .add_enabled(
                            self.recording.is_none(),
                            egui::Button::selectable(is_selected, text),
                        )
                        .clicked()
                    {
                        next_selected = Some(question.position as usize);
                    }
                }
                if let Some(position) = next_selected {
                    self.re_recording = false;
                    self.player = None;
                    self.texture = None;
                    self.screen = Screen::Studio {
                        project_id: project_id.to_string(),
                        selected: position,
                    };
                }
                ui.add_space(16.0);
                if ui
                    .add_enabled(
                        self.recording.is_none() && !self.saving,
                        egui::Button::new("Finish").min_size(egui::vec2(ui.available_width(), 36.0)),
                    )
                    .clicked()
                {
                    self.camera = None;
                    self.player = None;
                    self.screen = Screen::Finish {
                        project_id: project_id.to_string(),
                    };
                }
            });

        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_header(ui, true);
            ui.add_space(8.0);
            let question_text = current
                .as_ref()
                .map(|row| row.text.as_str())
                .unwrap_or("Select a question");
            ui.label(RichText::new(question_text).size(20.0).strong());
            ui.add_space(10.0);
            let placeholder = if self.wants_playback() {
                "Loading answer…"
            } else {
                self.preview_placeholder()
            };
            let show_texture = if self.wants_live_preview() || self.wants_playback() {
                self.texture.as_ref()
            } else {
                None
            };
            draw_square_preview(ui, show_texture, self.frame_side, placeholder);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.colored_label(HORIZONTAL, "■ 16:9 horizontal");
                ui.add_space(12.0);
                ui.colored_label(VERTICAL, "■ 16:9 vertical");
            });
            ui.add_space(14.0);
            self.draw_answer_button(ui, current.as_ref().is_some_and(|row| row.answered()));
            ui.add_space(10.0);
            ui.label(RichText::new(&self.status).color(Color32::from_gray(190)));
            if let Some(err) = &self.ffmpeg_error {
                ui.colored_label(Color32::from_rgb(232, 160, 80), err);
            }
        });
    }

    fn draw_answer_button(&mut self, ui: &mut egui::Ui, answered: bool) {
        let recording = self.recording.is_some();
        let live = self.wants_live_preview();
        let label = if recording {
            let elapsed = self
                .recording
                .as_ref()
                .map(|session| session.started.elapsed())
                .unwrap_or_default();
            format!("Stop   {}", format_clock(elapsed))
        } else if self.saving {
            "Saving…".into()
        } else if live && answered {
            "Record your answer".into()
        } else if answered {
            "Re-record your answer".into()
        } else {
            "Record your answer".into()
        };
        let color = if recording {
            Color32::from_rgb(196, 56, 56)
        } else {
            ACCENT
        };
        if wide_button(ui, &label, color, !self.saving) {
            if recording {
                self.stop_recording();
            } else if answered && !self.re_recording {
                self.player = None;
                self.texture = None;
                self.re_recording = true;
                self.status = "Live preview ready. Record when you want to replace this answer.".into();
            } else {
                self.start_recording();
            }
        }
    }

    pub(crate) fn draw_finish(&mut self, ui: &mut egui::Ui, project_id: &str) {
        let questions = self.db.questions(project_id).unwrap_or_default();
        let answered = questions.iter().filter(|row| row.answered()).count();
        let transcribed = questions
            .iter()
            .filter(|row| row.answered() && row.transcribed())
            .count();
        let ready = answered > 0 && transcribed == answered;

        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_header(ui, true);
            ui.add_space(12.0);
            ui.label(RichText::new("Finish").size(22.0).strong());
            ui.label(
                RichText::new(format!(
                    "{answered} answered · {transcribed} transcribed. Transcription is required before export."
                ))
                .color(MUTED),
            );
            ui.add_space(18.0);
            let transcribe_label = if let Some((done, total)) = self.transcribe_progress {
                format!("Transcribing {done} / {total}")
            } else {
                "Transcribe videos".into()
            };
            if wide_button(
                ui,
                &transcribe_label,
                ACCENT,
                !self.busy && answered > 0 && transcribed < answered,
            ) {
                self.start_transcribe();
            }
            ui.add_space(12.0);
            ui.checkbox(&mut self.burn_captions, "Burn captions on horizontal export");
            ui.label(
                RichText::new("Optional. White text, black border, 50% width, 15% above the bottom. Vertical stays clean.")
                    .size(12.0)
                    .color(MUTED),
            );
            ui.add_space(16.0);
            if wide_button(
                ui,
                if self.busy && self.transcribe_progress.is_none() {
                    "Exporting…"
                } else {
                    "Export"
                },
                Color32::from_rgb(46, 120, 92),
                !self.busy && ready,
            ) {
                self.start_export();
            }
            ui.add_space(10.0);
            ui.label(RichText::new(&self.status).color(Color32::from_gray(190)));
            ui.add_space(16.0);
            if ui.add_enabled(!self.busy, egui::Button::new("Back to studio")).clicked() {
                self.transcribe_progress = None;
                self.screen = Screen::Studio {
                    project_id: project_id.to_string(),
                    selected: 1,
                };
            }
        });
    }
}
