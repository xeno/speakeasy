use base64::Engine;
use serde_json::{Value, json};
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

const MODEL: &str = "gemini-2.5-flash";
const BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(120)))
        .build()
        .into()
}

fn generate(key: &str, body: &Value) -> Result<String, String> {
    let url = format!("{BASE}/{MODEL}:generateContent");
    let mut response = agent()
        .post(&url)
        .header("x-goog-api-key", key.trim())
        .header("Content-Type", "application/json")
        .send_json(body)
        .map_err(|err| format!("Gemini: {err}"))?;
    let value: Value = response
        .body_mut()
        .read_json()
        .map_err(|err| format!("Gemini response: {err}"))?;
    if !response.status().is_success() {
        let message = value["error"]["message"]
            .as_str()
            .unwrap_or("Gemini request failed");
        return Err(message.to_string());
    }
    let text = value["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .ok_or("Gemini returned an empty response")?;
    Ok(text.trim().to_string())
}

pub fn validate_key(key: &str) -> Result<(), String> {
    if key.trim().is_empty() {
        return Err("Paste a Gemini API key to continue.".into());
    }
    let url = format!("{BASE}/{MODEL}");
    let mut response = agent()
        .get(&url)
        .header("x-goog-api-key", key.trim())
        .call()
        .map_err(|err| format!("Gemini: {err}"))?;
    if response.status().is_success() {
        return Ok(());
    }
    let value: Value = response
        .body_mut()
        .read_json()
        .unwrap_or(json!({}));
    let message = value["error"]["message"]
        .as_str()
        .unwrap_or("That Gemini API key did not work.");
    Err(message.to_string())
}

pub fn generate_questions(key: &str, topic: &str) -> Result<Vec<String>, String> {
    let prompt = format!(
        "You write interview questions for a spoken-video app.\n\
         Given the topic, return JSON {{\"questions\":[\"...\"]}} with exactly 10 concise questions.\n\
         Each question should be answerable on camera in 30 to 90 seconds.\n\
         Topic: {topic}"
    );
    let body = json!({
        "contents": [{"parts": [{"text": prompt}]}],
        "generationConfig": { "responseMimeType": "application/json" }
    });
    let text = generate(key, &body)?;
    let parsed: Value = serde_json::from_str(&text)
        .map_err(|_| format!("Gemini did not return JSON questions: {text}"))?;
    let questions = parsed["questions"]
        .as_array()
        .ok_or("Gemini JSON was missing questions")?
        .iter()
        .filter_map(|item| item.as_str().map(|text| text.trim().to_string()))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    if questions.len() < 10 {
        return Err(format!(
            "Gemini returned {} questions, expected 10.",
            questions.len()
        ));
    }
    Ok(questions.into_iter().take(10).collect())
}

pub fn transcribe(key: &str, wav_path: &Path) -> Result<String, String> {
    let mp3 = compress_speech(wav_path)?;
    let bytes = std::fs::read(&mp3).map_err(|err| err.to_string())?;
    let _ = std::fs::remove_file(&mp3);
    if bytes.len() > 15 * 1024 * 1024 {
        return Err("Answer audio is too large to transcribe.".into());
    }
    let data = base64::engine::general_purpose::STANDARD.encode(bytes);
    let body = json!({
        "contents": [{
            "parts": [
                { "text": "Transcribe this spoken answer verbatim. Return only the transcript text, no timestamps or commentary." },
                { "inlineData": { "mimeType": "audio/mpeg", "data": data } }
            ]
        }]
    });
    let text = generate(key, &body)?;
    if text.is_empty() {
        Err("Gemini returned an empty transcript.".into())
    } else {
        Ok(text)
    }
}

fn compress_speech(wav: &Path) -> Result<std::path::PathBuf, String> {
    let out = wav.with_extension("gemini.mp3");
    let wav = wav.to_str().ok_or("Invalid audio path")?;
    let dest = out.to_str().ok_or("Invalid audio path")?;
    let output = crate::ffmpeg::command()
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-i",
            wav,
            "-ac",
            "1",
            "-ar",
            "16000",
            "-b:a",
            "48k",
            dest,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| format!("ffmpeg: {err}"))?;
    if output.status.success() {
        Ok(out)
    } else {
        Err(format!(
            "Could not prepare audio for Gemini: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}
