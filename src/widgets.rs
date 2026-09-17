use eframe::egui::{self, Color32, ComboBox, RichText};

pub const FIELD: f32 = 40.0;
pub const LABEL: Color32 = Color32::from_rgb(196, 196, 200);
pub const MUTED: Color32 = Color32::from_gray(150);
pub const SECTION: Color32 = Color32::from_rgb(130, 130, 138);
pub const ACCENT: Color32 = Color32::from_rgb(188, 44, 44);

pub fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.label(
        RichText::new(text.to_uppercase())
            .size(11.0)
            .strong()
            .color(SECTION),
    );
    ui.add_space(2.0);
}

pub fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(13.0).color(LABEL));
}

pub fn full_combo(
    ui: &mut egui::Ui,
    id: &'static str,
    selected: impl Into<egui::WidgetText>,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    ComboBox::from_id_salt(id)
        .width(ui.available_width())
        .height(FIELD)
        .selected_text(selected)
        .show_ui(ui, |ui| {
            ui.set_min_width(280.0);
            ui.spacing_mut().item_spacing.y = 6.0;
            ui.spacing_mut().button_padding = egui::vec2(12.0, 8.0);
            add_contents(ui);
        });
}

pub fn padded_choice<T: PartialEq>(ui: &mut egui::Ui, current: &mut T, value: T, label: &str) {
    ui.selectable_value(current, value, RichText::new(label).size(15.0));
}

pub fn format_clock(duration: std::time::Duration) -> String {
    let secs = duration.as_secs();
    format!("{:02}:{:02}", secs / 60, secs % 60)
}

pub fn wide_button(ui: &mut egui::Ui, label: &str, color: Color32, enabled: bool) -> bool {
    let button = egui::Button::new(RichText::new(label).size(17.0).strong())
        .fill(color)
        .min_size(egui::vec2(ui.available_width(), 44.0));
    ui.add_enabled(enabled, button).clicked()
}
