use crate::crop::scale_square;
use crate::quality::{OutputQuality, PREVIEW_SIDE};
use eframe::egui;
use nokhwa::Camera;
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType, Resolution,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone)]
pub struct SquareFrame {
    pub rgb: Arc<[u8]>,
    pub side: u32,
    pub preview: Arc<[u8]>,
    pub preview_side: u32,
    pub seq: u64,
}

pub type FrameSlot = Arc<Mutex<Option<SquareFrame>>>;

pub struct CameraFeed {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    pub frames: FrameSlot,
    pub index: CameraIndex,
    pub quality: OutputQuality,
    pub error: Arc<Mutex<Option<String>>>,
}

impl CameraFeed {
    pub fn start(
        index: CameraIndex,
        quality: OutputQuality,
        ctx: egui::Context,
    ) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let frames: FrameSlot = Arc::new(Mutex::new(None));
        let error = Arc::new(Mutex::new(None));
        let thread_stop = Arc::clone(&stop);
        let thread_frames = Arc::clone(&frames);
        let thread_error = Arc::clone(&error);
        let thread_index = index.clone();

        let join = thread::Builder::new()
            .name("speakeasy-camera".into())
            .spawn(move || {
                if let Err(err) = run_camera(
                    thread_index,
                    quality,
                    thread_stop,
                    thread_frames,
                    &thread_error,
                    ctx,
                ) {
                    if let Ok(mut slot) = thread_error.lock() {
                        *slot = Some(err);
                    }
                }
            })
            .map_err(|err| format!("Failed to start camera thread: {err}"))?;

        Ok(Self {
            stop,
            join: Some(join),
            frames,
            index,
            quality,
            error,
        })
    }

    pub fn latest(&self) -> Option<SquareFrame> {
        self.frames.lock().ok().and_then(|guard| guard.clone())
    }

    pub fn take_error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|mut slot| slot.take())
    }
}

impl Drop for CameraFeed {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

fn run_camera(
    index: CameraIndex,
    quality: OutputQuality,
    stop: Arc<AtomicBool>,
    frames: FrameSlot,
    error: &Arc<Mutex<Option<String>>>,
    ctx: egui::Context,
) -> Result<(), String> {
    let mut camera = open_preview_camera(index, quality)?;
    if let Ok(formats) = camera.compatible_camera_formats()
        && let Some(best) = pick_preview_format(&formats, quality)
    {
        let _ = camera.set_camera_requset(RequestedFormat::new::<RgbFormat>(
            RequestedFormatType::Closest(best),
        ));
    }
    camera.open_stream().map_err(|err| err.to_string())?;

    let target_side = quality.square_side();
    let mut decoded = Vec::new();
    let mut square = Vec::new();
    let mut preview = Vec::new();
    let mut seq = 0_u64;

    while !stop.load(Ordering::Relaxed) {
        match camera.frame() {
            Ok(buffer) => {
                let width = buffer.resolution().width();
                let height = buffer.resolution().height();
                decoded.resize(width as usize * height as usize * 3, 0);
                if let Err(err) = buffer.decode_image_to_buffer::<RgbFormat>(&mut decoded) {
                    set_error(error, err.to_string());
                    continue;
                }
                let Some(side) =
                    scale_square(&decoded, width, height, target_side, true, &mut square)
                else {
                    continue;
                };
                let preview_side = if side > PREVIEW_SIDE {
                    scale_square(&square, side, side, PREVIEW_SIDE, false, &mut preview)
                        .unwrap_or(side)
                } else {
                    preview.clear();
                    side
                };
                let full = Arc::<[u8]>::from(square.as_slice());
                let preview_rgb = if preview.is_empty() {
                    Arc::clone(&full)
                } else {
                    Arc::<[u8]>::from(preview.as_slice())
                };
                let shown_side = if preview.is_empty() {
                    side
                } else {
                    preview_side
                };
                seq += 1;
                if let Ok(mut slot) = frames.lock() {
                    *slot = Some(SquareFrame {
                        rgb: full,
                        side,
                        preview: preview_rgb,
                        preview_side: shown_side,
                        seq,
                    });
                }
                ctx.request_repaint();
            }
            Err(err) => {
                set_error(error, err.to_string());
                thread::sleep(Duration::from_millis(50));
            }
        }
    }

    Ok(())
}

fn pick_preview_format(formats: &[CameraFormat], quality: OutputQuality) -> Option<CameraFormat> {
    let (target_w, target_h) = quality.capture_size();
    let target_area = u64::from(target_w) * u64::from(target_h);
    formats.iter().copied().min_by_key(|format| {
        let width = format.width();
        let height = format.height();
        let fps = format.frame_rate();
        let undersized = u8::from(width < target_w && height < target_h);
        let oversized =
            u8::from(width > target_w.saturating_mul(2) || height > target_h.saturating_mul(2));
        let slow = u8::from(fps < 24);
        let size = u64::from(width.saturating_mul(height)).abs_diff(target_area);
        let codec = match format.format() {
            FrameFormat::MJPEG | FrameFormat::NV12 => 0_u8,
            _ => 1,
        };
        (
            undersized,
            oversized,
            slow,
            size,
            codec,
            120_u32.saturating_sub(fps),
        )
    })
}

fn open_preview_camera(index: CameraIndex, quality: OutputQuality) -> Result<Camera, String> {
    let (width, height) = quality.capture_size();
    let requests = [
        (width, height, FrameFormat::MJPEG),
        (width, height, FrameFormat::NV12),
        (width, height, FrameFormat::YUYV),
        (1920, 1080, FrameFormat::MJPEG),
        (1280, 720, FrameFormat::MJPEG),
    ];
    let mut last_error = String::from("No compatible video format");
    for (w, h, format) in requests {
        let request =
            RequestedFormatType::Closest(CameraFormat::new(Resolution::new(w, h), format, 30));
        match Camera::new(index.clone(), RequestedFormat::new::<RgbFormat>(request)) {
            Ok(camera) => return Ok(camera),
            Err(err) => last_error = err.to_string(),
        }
    }
    Err(last_error)
}

fn set_error(error: &Arc<Mutex<Option<String>>>, message: String) {
    if let Ok(mut slot) = error.lock() {
        *slot = Some(message);
    }
}
