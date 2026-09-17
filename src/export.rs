use crate::captions::write_caption_overlays;
use crate::crop::{PixelRect, crops_for_square};
use std::path::Path;

pub fn export_pair(
    project_dir: &Path,
    position: i64,
    captions: Option<&str>,
) -> Result<(), String> {
    let square = project_dir.join(format!("Q{position}.mp4"));
    let wav = project_dir.join(format!("Q{position}.wav"));
    let horizontal = project_dir.join(format!("Q{position}-H.mp4"));
    let vertical = project_dir.join(format!("Q{position}-V.mp4"));
    let side = probe_side(&square)?;
    let crops = crops_for_square(side);
    if let Some(text) = captions {
        let duration = wav_seconds(&wav)?;
        let overlays =
            write_caption_overlays(project_dir, position, text, duration, crops.horizontal.w, crops.horizontal.h)?;
        let result = run_crop_captioned(&square, &wav, crops.horizontal, &overlays, &horizontal);
        for overlay in overlays {
            let _ = std::fs::remove_file(overlay.path);
        }
        result?;
    } else {
        run_crop(&square, &wav, crops.horizontal, &horizontal)?;
    }
    run_crop(&square, &wav, crops.vertical, &vertical)
}

fn run_crop(video: &Path, audio: &Path, crop: PixelRect, out: &Path) -> Result<(), String> {
    run_ffmpeg(&[
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        path_string(video)?,
        "-i".into(),
        path_string(audio)?,
        "-filter:v".into(),
        crop.ffmpeg_crop(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "fast".into(),
        "-crf".into(),
        "18".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-movflags".into(),
        "+faststart".into(),
        "-shortest".into(),
        path_string(out)?,
    ])
}

fn run_crop_captioned(
    video: &Path,
    audio: &Path,
    crop: PixelRect,
    overlays: &[crate::captions::CaptionOverlay],
    out: &Path,
) -> Result<(), String> {
    if overlays.is_empty() {
        return run_crop(video, audio, crop, out);
    }
    let margin = ((crop.h as f32) * 0.15).round() as i32;
    let mut args = vec![
        "-hide_banner".into(),
        "-loglevel".into(),
        "error".into(),
        "-y".into(),
        "-i".into(),
        path_string(video)?,
        "-i".into(),
        path_string(audio)?,
    ];
    for overlay in overlays {
        args.extend([
            "-loop".into(),
            "1".into(),
            "-i".into(),
            path_string(&overlay.path)?,
        ]);
    }
    let mut graph = format!("[0:v]{}[v0]", crop.ffmpeg_crop());
    for (index, overlay) in overlays.iter().enumerate() {
        let src = format!("v{index}");
        let dst = if index + 1 == overlays.len() {
            "v".into()
        } else {
            format!("v{}", index + 1)
        };
        let input = index + 2;
        graph.push_str(&format!(
            ";[{src}][{input}:v]overlay=x=(W-w)/2:y=H-h-{margin}:enable='between(t,{:.2},{:.2})'[{dst}]",
            overlay.start, overlay.end
        ));
    }
    args.extend([
        "-filter_complex".into(),
        graph,
        "-map".into(),
        "[v]".into(),
        "-map".into(),
        "1:a".into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "fast".into(),
        "-crf".into(),
        "18".into(),
        "-pix_fmt".into(),
        "yuv420p".into(),
        "-c:a".into(),
        "aac".into(),
        "-b:a".into(),
        "192k".into(),
        "-movflags".into(),
        "+faststart".into(),
        "-shortest".into(),
        path_string(out)?,
    ]);
    run_ffmpeg(&args)
}

fn run_ffmpeg(args: &[String]) -> Result<(), String> {
    let output = crate::ffmpeg::command()
        .args(args)
        .output()
        .map_err(|err| format!("ffmpeg: {err}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "ffmpeg failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

fn probe_side(path: &Path) -> Result<u32, String> {
    let output = crate::ffmpeg::probe()
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width",
            "-of",
            "csv=p=0",
            path_str(path)?,
        ])
        .output()
        .map_err(|err| format!("ffprobe: {err}"))?;
    if !output.status.success() {
        return Ok(1080);
    }
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .or(Ok(1080))
}

fn wav_seconds(path: &Path) -> Result<f64, String> {
    let reader = hound::WavReader::open(path).map_err(|err| err.to_string())?;
    let rate = f64::from(reader.spec().sample_rate);
    if rate == 0.0 {
        return Ok(1.0);
    }
    Ok(f64::from(reader.duration()) / rate)
}

fn path_str(path: &Path) -> Result<&str, String> {
    path.to_str().ok_or_else(|| "Invalid path".into())
}

fn path_string(path: &Path) -> Result<String, String> {
    Ok(path_str(path)?.to_string())
}
