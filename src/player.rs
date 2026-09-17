use crate::quality::PREVIEW_SIDE;
use eframe::egui;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone)]
pub struct PlayerFrame {
    pub rgb: Arc<[u8]>,
    pub side: u32,
    pub seq: u64,
}

pub struct VideoPlayer {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    frames: Arc<Mutex<Option<PlayerFrame>>>,
    ffmpeg: Arc<Mutex<Option<Child>>>,
    audio: Option<Child>,
    pub path: PathBuf,
}

impl VideoPlayer {
    pub fn start(video: PathBuf, wav: Option<PathBuf>, ctx: egui::Context) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let frames = Arc::new(Mutex::new(None));
        let ffmpeg = Arc::new(Mutex::new(None));
        let video_arg = video.to_str().ok_or("Invalid video path")?.to_string();
        let thread_stop = Arc::clone(&stop);
        let thread_frames = Arc::clone(&frames);
        let thread_ffmpeg = Arc::clone(&ffmpeg);

        let join = thread::Builder::new()
            .name("speakeasy-player".into())
            .spawn(move || {
                if let Err(err) = play_frames(&video_arg, thread_stop, thread_frames, thread_ffmpeg, ctx)
                {
                    eprintln!("SpeakEasy player: {err}");
                }
            })
            .map_err(|err| format!("Failed to start player: {err}"))?;

        let audio = wav.and_then(|path| {
            Command::new("afplay")
                .arg(path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .ok()
        });

        Ok(Self {
            stop,
            join: Some(join),
            frames,
            ffmpeg,
            audio,
            path: video,
        })
    }

    pub fn latest(&self) -> Option<PlayerFrame> {
        self.frames.lock().ok().and_then(|guard| guard.clone())
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Ok(mut slot) = self.ffmpeg.lock()
            && let Some(mut child) = slot.take()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(mut child) = self.audio.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn play_frames(
    video: &str,
    stop: Arc<AtomicBool>,
    frames: Arc<Mutex<Option<PlayerFrame>>>,
    ffmpeg_slot: Arc<Mutex<Option<Child>>>,
    ctx: egui::Context,
) -> Result<(), String> {
    let side = PREVIEW_SIDE;
    let mut child = crate::ffmpeg::command()
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            video,
            "-an",
            "-vf",
            &format!("scale={side}:{side}"),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "pipe:1",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("ffmpeg: {err}"))?;
    let mut stdout = child.stdout.take().ok_or("ffmpeg stdout unavailable")?;
    if let Ok(mut slot) = ffmpeg_slot.lock() {
        *slot = Some(child);
    }

    let frame_len = side as usize * side as usize * 3;
    let mut buf = vec![0_u8; frame_len];
    let mut seq = 0_u64;
    while !stop.load(Ordering::Relaxed) {
        if let Err(err) = stdout.read_exact(&mut buf) {
            if err.kind() != std::io::ErrorKind::UnexpectedEof {
                return Err(err.to_string());
            }
            break;
        }
        seq += 1;
        if let Ok(mut slot) = frames.lock() {
            *slot = Some(PlayerFrame {
                rgb: Arc::<[u8]>::from(buf.as_slice()),
                side,
                seq,
            });
        }
        ctx.request_repaint();
        thread::sleep(Duration::from_millis(33));
    }
    Ok(())
}
