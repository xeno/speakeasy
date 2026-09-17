mod actions;
mod app;
mod audio;
mod bitmap_font;
mod camera;
mod captions;
mod crop;
mod db;
mod devices;
mod export;
mod ffmpeg;
mod gemini;
mod home;
mod jobs;
mod player;
mod preview;
mod quality;
mod recorder;
mod sources;
mod storage;
mod studio;
mod view;
mod widgets;

use app::SpeakEasyApp;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 760.0])
            .with_min_inner_size([880.0, 640.0])
            .with_title("SpeakEasy"),
        ..Default::default()
    };

    eframe::run_native(
        "SpeakEasy",
        options,
        Box::new(|cc| Ok(Box::new(SpeakEasyApp::new(cc)))),
    )
}
