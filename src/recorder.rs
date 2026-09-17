use crate::audio::MicCapture;
use crate::camera::{FrameSlot, SquareFrame};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const FPS: u32 = 30;

pub struct Recording {
    stop: Arc<AtomicBool>,
    video_thread: Option<JoinHandle<Result<(), String>>>,
    ffmpeg: Child,
    mic: Option<MicCapture>,
    project_dir: PathBuf,
    square_path: PathBuf,
    wav_path: PathBuf,
    position: i64,
    pub started: Instant,
}

pub struct RecordedAnswer {
    pub square_rel: String,
    pub wav_rel: String,
}

impl Recording {
    pub fn start(
        frames: FrameSlot,
        first: SquareFrame,
        mic_name: &str,
        project_dir: PathBuf,
        position: i64,
    ) -> Result<Self, String> {
        std::fs::create_dir_all(&project_dir).map_err(|err| err.to_string())?;
        let square_path = project_dir.join(format!("Q{position}.recording.mp4"));
        let wav_path = project_dir.join(format!("Q{position}.recording.wav"));
        let (ffmpeg, stdin) = spawn_square_encoder(&square_path, first.side)?;
        let mic = MicCapture::start(mic_name, &wav_path)?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);

        let video_thread = thread::Builder::new()
            .name("speakeasy-record".into())
            .spawn(move || write_frames(stdin, frames, first, thread_stop))
            .map_err(|err| format!("Failed to start recorder: {err}"))?;

        Ok(Self {
            stop,
            video_thread: Some(video_thread),
            ffmpeg,
            mic: Some(mic),
            project_dir,
            square_path,
            wav_path,
            position,
            started: Instant::now(),
        })
    }

    pub fn finish(mut self) -> Result<RecordedAnswer, String> {
        self.stop.store(true, Ordering::Relaxed);
        let video_result = self
            .video_thread
            .take()
            .map(|join| {
                join.join()
                    .unwrap_or_else(|_| Err("Video thread panicked".into()))
            })
            .unwrap_or(Ok(()));
        if let Some(mic) = self.mic.take() {
            mic.finish()?;
        }
        wait_ffmpeg(&mut self.ffmpeg)?;
        video_result?;
        let dest_mp4 = self.project_dir.join(format!("Q{}.mp4", self.position));
        let dest_wav = self.project_dir.join(format!("Q{}.wav", self.position));
        replace_file(&self.square_path, &dest_mp4)?;
        replace_file(&self.wav_path, &dest_wav)?;
        Ok(RecordedAnswer {
            square_rel: format!("Q{}.mp4", self.position),
            wav_rel: format!("Q{}.wav", self.position),
        })
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.video_thread.take() {
            let _ = join.join();
        }
        let _ = self.ffmpeg.kill();
        let _ = self.ffmpeg.wait();
    }
}

fn replace_file(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() {
        std::fs::remove_file(to).map_err(|err| err.to_string())?;
    }
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) => {
            std::fs::copy(from, to).map_err(|err| err.to_string())?;
            let _ = std::fs::remove_file(from);
            Ok(())
        }
    }
}

fn write_frames(
    mut stdin: std::process::ChildStdin,
    frames: FrameSlot,
    first: SquareFrame,
    stop: Arc<AtomicBool>,
) -> Result<(), String> {
    let mut last = first;
    let start = Instant::now();
    let mut n = 0_u64;
    while !stop.load(Ordering::Relaxed) {
        if let Ok(guard) = frames.lock()
            && let Some(frame) = guard.as_ref()
            && frame.side == last.side
        {
            last = frame.clone();
        }
        stdin
            .write_all(&last.rgb)
            .map_err(|err| format!("Writing video: {err}"))?;
        n += 1;
        let target = start + Duration::from_secs_f64(n as f64 / f64::from(FPS));
        if let Some(wait) = target.checked_duration_since(Instant::now()) {
            thread::sleep(wait);
        }
    }
    drop(stdin);
    Ok(())
}

fn spawn_square_encoder(
    path: &Path,
    side: u32,
) -> Result<(Child, std::process::ChildStdin), String> {
    let size = format!("{side}x{side}");
    let mut child = crate::ffmpeg::command()
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-s",
            &size,
            "-r",
            &FPS.to_string(),
            "-i",
            "pipe:0",
            "-an",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-crf",
            "18",
            "-pix_fmt",
            "yuv420p",
            path_str(path)?,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| {
            if err.kind() == ErrorKind::NotFound {
                crate::ffmpeg::missing_message().into()
            } else {
                format!("Failed to start ffmpeg: {err}")
            }
        })?;
    let stdin = child.stdin.take().ok_or("ffmpeg stdin unavailable")?;
    Ok((child, stdin))
}

fn wait_ffmpeg(child: &mut Child) -> Result<(), String> {
    let mut stderr = child.stderr.take();
    let status = child.wait().map_err(|err| err.to_string())?;
    if status.success() {
        return Ok(());
    }
    let mut message = String::new();
    if let Some(mut pipe) = stderr.take() {
        let _ = pipe.read_to_string(&mut message);
    }
    Err(format!("ffmpeg failed: {}", message.trim()))
}

fn path_str(path: &Path) -> Result<&str, String> {
    path.to_str().ok_or_else(|| "Invalid path".into())
}

pub fn ffmpeg_available() -> Result<(), String> {
    crate::ffmpeg::command()
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| {
            if err.kind() == ErrorKind::NotFound {
                crate::ffmpeg::missing_message().into()
            } else {
                format!("ffmpeg: {err}")
            }
        })
        .and_then(|status| {
            if status.success() {
                Ok(())
            } else {
                Err("ffmpeg is installed but failed to run".into())
            }
        })
}
