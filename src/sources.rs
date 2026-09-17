use crate::app::{Job, SpeakEasyApp};
use crate::camera::CameraFeed;
use crate::player::VideoPlayer;
use crate::recorder::Recording;
use eframe::egui::{self, TextureOptions};
use std::sync::Arc;
use std::thread;

impl SpeakEasyApp {
    pub(crate) fn ensure_sources(&mut self, ctx: &egui::Context) {
        if self.wants_live_preview() {
            self.player = None;
            self.ensure_camera(ctx);
        } else if self.wants_playback() {
            self.camera = None;
            self.ensure_player(ctx);
        } else {
            self.camera = None;
            self.player = None;
        }
    }

    fn ensure_camera(&mut self, ctx: &egui::Context) {
        let Some(index) = self.selected_camera.clone() else {
            self.camera = None;
            return;
        };
        let needs_restart = self
            .camera
            .as_ref()
            .is_none_or(|feed| feed.index != index || feed.quality != self.quality);
        if needs_restart && self.recording.is_none() {
            match CameraFeed::start(index, self.quality, ctx.clone()) {
                Ok(feed) => {
                    self.last_frame_seq = None;
                    self.camera = Some(feed);
                }
                Err(err) => self.status = format!("Camera: {err}"),
            }
        }
    }

    fn ensure_player(&mut self, ctx: &egui::Context) {
        let Some((video, wav)) = self.selected_media_paths() else {
            self.player = None;
            return;
        };
        let restart = self
            .player
            .as_ref()
            .is_none_or(|player| player.path != video);
        if restart {
            self.last_frame_seq = None;
            match VideoPlayer::start(video, wav, ctx.clone()) {
                Ok(player) => self.player = Some(player),
                Err(err) => self.status = format!("Playback: {err}"),
            }
        }
    }

    pub(crate) fn update_texture(&mut self, ctx: &egui::Context) {
        if let Some(camera) = &self.camera {
            if let Some(err) = camera.take_error() {
                self.status = format!("Camera: {err}");
            }
            let Some(frame) = camera.latest() else {
                return;
            };
            if self.last_frame_seq == Some(frame.seq) {
                return;
            }
            self.last_frame_seq = Some(frame.seq);
            self.frame_side = Some(frame.side);
            self.set_texture(ctx, frame.preview_side, frame.preview.as_ref());
            return;
        }
        let Some(player) = &self.player else {
            return;
        };
        let Some(frame) = player.latest() else {
            return;
        };
        if self.last_frame_seq == Some(frame.seq) {
            return;
        }
        self.last_frame_seq = Some(frame.seq);
        self.frame_side = Some(frame.side);
        self.set_texture(ctx, frame.side, frame.rgb.as_ref());
    }

    fn set_texture(&mut self, ctx: &egui::Context, side: u32, rgb: &[u8]) {
        let image = egui::ColorImage::from_rgb([side as usize, side as usize], rgb);
        match &mut self.texture {
            Some(texture) if texture.size() == [side as usize, side as usize] => {
                texture.set(image, TextureOptions::LINEAR);
            }
            _ => {
                self.texture = Some(ctx.load_texture("preview", image, TextureOptions::LINEAR));
            }
        }
    }

    pub(crate) fn start_recording(&mut self) {
        if let Some(err) = &self.ffmpeg_error {
            self.status = err.clone();
            return;
        }
        let Some((project_id, selected)) = self.studio_selection() else {
            return;
        };
        let Ok(project) = self.db.project(&project_id) else {
            self.status = "Project not found.".into();
            return;
        };
        let Ok(questions) = self.db.questions(&project_id) else {
            return;
        };
        let Some(question) = questions
            .iter()
            .find(|row| row.position == selected as i64)
        else {
            self.status = "Select a question first.".into();
            return;
        };
        let Some(camera) = &self.camera else {
            self.status = "Select a video source first.".into();
            return;
        };
        let Some(frame) = camera.latest() else {
            self.status = "Wait for the video preview before recording.".into();
            return;
        };
        let Some(mic) = self.selected_mic.clone() else {
            self.status = "Select a microphone first.".into();
            return;
        };
        match Recording::start(
            Arc::clone(&camera.frames),
            frame,
            &mic,
            project.dir(),
            question.position,
        ) {
            Ok(recording) => {
                self.status = "Recording…".into();
                self.record_question = Some(question.id);
                self.recording = Some(recording);
            }
            Err(err) => self.status = err,
        }
    }

    pub(crate) fn stop_recording(&mut self) {
        let Some(recording) = self.recording.take() else {
            return;
        };
        let Some(question_id) = self.record_question.take() else {
            return;
        };
        self.saving = true;
        self.status = "Saving Q answer…".into();
        let tx = self.job_tx.clone();
        thread::spawn(move || {
            let result = recording.finish();
            let job = match result {
                Ok(answer) => Job::Recorded {
                    question_id,
                    square: answer.square_rel,
                    wav: answer.wav_rel,
                },
                Err(err) => Job::RecordErr(err),
            };
            let _ = tx.send(job);
        });
    }

    pub(crate) fn camera_label(&self) -> String {
        self.selected_camera
            .as_ref()
            .and_then(|index| {
                self.cameras
                    .iter()
                    .find(|camera| &camera.index == index)
                    .map(|camera| camera.name.clone())
            })
            .unwrap_or_else(|| "No video source".into())
    }
}
