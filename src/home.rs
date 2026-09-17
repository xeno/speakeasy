use crate::app::{Screen, SpeakEasyApp};
use crate::widgets::{ACCENT, MUTED, wide_button};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, RichText, Sense, Vec2};

impl SpeakEasyApp {
    pub(crate) fn draw_home(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            self.draw_header(ui, true);
            ui.label(
                RichText::new("Your interview projects.")
                    .color(Color32::from_gray(170)),
            );
            ui.add_space(14.0);
            if wide_button(ui, "New project", ACCENT, self.recording.is_none()) {
                self.new_title.clear();
                self.form_error = None;
                self.screen = Screen::NewProject;
            }
            ui.add_space(18.0);

            let projects = self.db.list_projects().unwrap_or_else(|err| {
                self.status = err;
                Vec::new()
            });
            if projects.is_empty() {
                ui.label(
                    RichText::new("No projects yet. Create one to get 10 questions from Gemini.")
                        .color(MUTED),
                );
                return;
            }

            let gap = 16.0;
            let cols = if ui.available_width() > 900.0 { 3 } else { 2 };
            let card = ((ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32)
                .clamp(160.0, 240.0);
            let mut open_id = None;
            egui::ScrollArea::vertical().show(ui, |ui| {
                for row in projects.chunks(cols) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for project in row {
                            if draw_project_card(ui, &project.title, card) {
                                open_id = Some(project.id.clone());
                            }
                        }
                    });
                    ui.add_space(gap);
                }
            });
            if let Some(id) = open_id {
                self.open_project(id);
            }
        });
    }
}

fn draw_project_card(ui: &mut egui::Ui, title: &str, size: f32) -> bool {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let fill = if response.hovered() {
        Color32::from_rgb(42, 42, 48)
    } else {
        Color32::from_rgb(32, 32, 36)
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 12.0, fill);
    painter.rect_stroke(
        rect,
        12.0,
        egui::Stroke::new(1.0, Color32::from_rgb(58, 58, 64)),
        egui::StrokeKind::Inside,
    );
    painter.text(
        Pos2::new(rect.center().x, rect.center().y),
        Align2::CENTER_CENTER,
        title,
        FontId::proportional(16.0),
        Color32::from_rgb(236, 236, 240),
    );
    response.clicked()
}
