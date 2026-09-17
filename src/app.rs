use crate::camera::CameraFeed;
use crate::db::Database;
use crate::devices::{AudioSource, VideoSource, list_cameras, list_mics};
use crate::player::VideoPlayer;
use crate::quality::OutputQuality;
use crate::recorder::{Recording, ffmpeg_available};
use crate::storage::{self, AppConfig};
use eframe::egui::{self, Color32, TextureHandle};
use nokhwa::nokhwa_initialize;
use nokhwa::utils::CameraIndex;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

pub(crate) const PERM_PENDING: u8 = 0;
pub(crate) const PERM_GRANTED: u8 = 1;
pub(crate) const PERM_DENIED: u8 = 2;

pub(crate) enum Screen {
    KeyGate,
    Home,
    NewProject,
    Topic { project_id: String },
    Studio { project_id: String, selected: usize },
    Finish { project_id: String },
}

pub(crate) enum Job {
    KeyOk,
    KeyErr(String),
    QuestionsOk {
        project_id: String,
        questions: Vec<String>,
    },
    QuestionsErr(String),
    Recorded {
        question_id: i64,
        square: String,
        wav: String,
    },
    RecordErr(String),
    Transcript {
        question_id: i64,
        text: String,
    },
    TranscribeProgress {
        done: usize,
        total: usize,
    },
    TranscribeDone,
    TranscribeErr(String),
    ExportDone(usize),
    ExportErr(String),
}

pub struct SpeakEasyApp {
    pub(crate) screen: Screen,
    pub(crate) config: AppConfig,
    pub(crate) db: Database,
    pub(crate) cameras: Vec<VideoSource>,
    pub(crate) mics: Vec<AudioSource>,
    pub(crate) selected_camera: Option<CameraIndex>,
    pub(crate) selected_mic: Option<String>,
    pub(crate) quality: OutputQuality,
    pub(crate) camera: Option<CameraFeed>,
    pub(crate) player: Option<VideoPlayer>,
    pub(crate) texture: Option<TextureHandle>,
    pub(crate) frame_side: Option<u32>,
    pub(crate) last_frame_seq: Option<u64>,
    pub(crate) recording: Option<Recording>,
    pub(crate) record_question: Option<i64>,
    pub(crate) saving: bool,
    pub(crate) busy: bool,
    pub(crate) job_tx: Sender<Job>,
    pub(crate) job_rx: Receiver<Job>,
    pub(crate) status: String,
    pub(crate) permission: Arc<AtomicU8>,
    devices_ready: bool,
    devices_applied: bool,
    created: Instant,
    pub(crate) ffmpeg_error: Option<String>,
    pub(crate) settings_open: bool,
    pub(crate) gate_key: String,
    pub(crate) gate_error: Option<String>,
    pub(crate) new_title: String,
    pub(crate) topic_input: String,
    pub(crate) form_error: Option<String>,
    pub(crate) re_recording: bool,
    pub(crate) burn_captions: bool,
    pub(crate) transcribe_progress: Option<(usize, usize)>,
}

impl SpeakEasyApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(20, 20, 22);
        visuals.window_fill = Color32::from_rgb(20, 20, 22);
        cc.egui_ctx.set_visuals(visuals);

        let permission = Arc::new(AtomicU8::new(PERM_PENDING));
        let callback_permission = Arc::clone(&permission);
        nokhwa_initialize(move |granted| {
            callback_permission.store(
                if granted { PERM_GRANTED } else { PERM_DENIED },
                Ordering::Relaxed,
            );
        });

        let config = storage::load_config();
        let quality = OutputQuality::from_label(&config.resolution);
        let db = match Database::open() {
            Ok(db) => db,
            Err(err) => {
                eprintln!("SpeakEasy database: {err}");
                Database::open_in_memory()
            }
        };
        let (job_tx, job_rx) = mpsc::channel();
        let screen = if config.has_key() {
            Screen::Home
        } else {
            Screen::KeyGate
        };

        Self {
            screen,
            config,
            db,
            cameras: Vec::new(),
            mics: Vec::new(),
            selected_camera: None,
            selected_mic: None,
            quality,
            camera: None,
            player: None,
            texture: None,
            frame_side: None,
            last_frame_seq: None,
            recording: None,
            record_question: None,
            saving: false,
            busy: false,
            job_tx,
            job_rx,
            status: String::new(),
            permission,
            devices_ready: false,
            devices_applied: false,
            created: Instant::now(),
            ffmpeg_error: ffmpeg_available().err(),
            settings_open: false,
            gate_key: String::new(),
            gate_error: None,
            new_title: String::new(),
            topic_input: String::new(),
            form_error: None,
            re_recording: false,
            burn_captions: false,
            transcribe_progress: None,
        }
    }

    pub(crate) fn reload_devices(&mut self) {
        match list_cameras() {
            Ok(cameras) => {
                if self.selected_camera.as_ref().is_none_or(|selected| {
                    !cameras.iter().any(|camera| &camera.index == selected)
                }) {
                    self.selected_camera = cameras.first().map(|camera| camera.index.clone());
                }
                self.cameras = cameras;
            }
            Err(err) => self.status = format!("Camera list: {err}"),
        }

        match list_mics() {
            Ok(mics) => {
                if self
                    .selected_mic
                    .as_ref()
                    .is_none_or(|selected| !mics.iter().any(|mic| &mic.name == selected))
                {
                    self.selected_mic = mics.first().map(|mic| mic.name.clone());
                }
                self.mics = mics;
            }
            Err(err) => self.status = format!("Microphone list: {err}"),
        }

        if !self.devices_applied {
            if let Some(name) = &self.config.video_source
                && let Some(camera) = self.cameras.iter().find(|camera| &camera.name == name)
            {
                self.selected_camera = Some(camera.index.clone());
            }
            if let Some(name) = &self.config.microphone
                && self.mics.iter().any(|mic| &mic.name == name)
            {
                self.selected_mic = Some(name.clone());
            }
            self.devices_applied = true;
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        let permission = self.permission.load(Ordering::Relaxed);
        if !self.devices_ready
            && (permission != PERM_PENDING || self.created.elapsed() > Duration::from_millis(800))
        {
            self.reload_devices();
            self.devices_ready = true;
        }
        self.drain_jobs();
        self.ensure_sources(ctx);
        self.update_texture(ctx);
        if self.recording.is_some() || self.saving || self.busy || self.player.is_some() {
            ctx.request_repaint_after(Duration::from_millis(33));
        } else if !self.devices_ready {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

impl eframe::App for SpeakEasyApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.tick(ui.ctx());
        self.draw(ui);
    }
}
