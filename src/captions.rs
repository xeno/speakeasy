use crate::bitmap_font;
use std::path::{Path, PathBuf};

pub struct CaptionOverlay {
    pub path: PathBuf,
    pub start: f64,
    pub end: f64,
}

pub fn write_caption_overlays(
    project_dir: &Path,
    position: i64,
    transcript: &str,
    duration: f64,
    width: u32,
    height: u32,
) -> Result<Vec<CaptionOverlay>, String> {
    let chunks = chunk_text(transcript);
    if chunks.is_empty() {
        return Err("Transcript is empty.".into());
    }
    let duration = duration.max(0.5);
    let slice = duration / chunks.len() as f64;
    let scale = (((height as f32) * 0.055) / 8.0).round().max(2.0) as u32;
    let max_chars = ((width / 2) / (8 * scale)).max(8) as usize;
    let mut overlays = Vec::new();
    for (index, chunk) in chunks.iter().enumerate() {
        let lines = wrap_line(chunk, max_chars);
        let (w, h, rgba) = bitmap_font::render_lines(&lines, scale);
        let path = project_dir.join(format!("Q{position}-cap{index}.pam"));
        write_pam(&path, w, h, &rgba)?;
        overlays.push(CaptionOverlay {
            path,
            start: slice * index as f64,
            end: if index + 1 == chunks.len() {
                duration
            } else {
                slice * (index + 1) as f64
            },
        });
    }
    Ok(overlays)
}

pub fn chunk_text(transcript: &str) -> Vec<String> {
    let words: Vec<&str> = transcript.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    words.chunks(8).map(|chunk| chunk.join(" ")).collect()
}

fn wrap_line(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        if current.is_empty() {
            current = word.to_string();
            continue;
        }
        if current.chars().count() + 1 + word.chars().count() > max_chars {
            lines.push(current);
            current = word.to_string();
        } else {
            current.push(' ');
            current.push_str(word);
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(text.to_string());
    }
    lines
}

fn write_pam(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    let header = format!(
        "P7\nWIDTH {width}\nHEIGHT {height}\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n"
    );
    let mut bytes = header.into_bytes();
    bytes.extend_from_slice(rgba);
    std::fs::write(path, bytes).map_err(|err| err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_group_words() {
        let text = "one two three four five six seven eight nine ten";
        let chunks = chunk_text(text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], "one two three four five six seven eight");
    }
}
