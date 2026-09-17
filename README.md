# SpeakEasy

macOS desktop app for interview-style video. You name a project, give a topic, and Gemini 2.5 Flash writes 10 questions. Record a square answer for each one, then export 16:9 horizontal and vertical crops.

Captions on the horizontal export are optional. Transcription is required before export.

The Gemini API key is never compiled in and never stored in this repo. After you paste it, it lives only in Application Support.

## Requirements

- macOS 13+
- [Rust](https://rustup.rs/) (stable; this crate uses edition 2024)
- [ffmpeg](https://ffmpeg.org/) and `ffprobe` on your `PATH`
- A [Gemini API key](https://aistudio.google.com/apikey)
- Camera and microphone access

## Development setup

```bash
# 1. Toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# 2. ffmpeg (Homebrew)
brew install ffmpeg

# 3. Clone and run
git clone git@github.com:xeno/speakeasy.git
cd speakeasy
cargo run
```

On first launch the app blocks until you paste a Gemini key. macOS will prompt for camera and microphone. If you deny them, allow SpeakEasy under **System Settings → Privacy & Security**.

Settings (cog) persist video source, microphone, resolution, and the key to:

```text
~/Library/Application Support/SpeakEasy/config.yaml
```

Projects and answers land in:

```text
~/Movies/SpeakEasy/{Project Name}/
  Q1.mp4          square master
  Q1.wav
  Q1-H.mp4        horizontal export
  Q1-V.mp4        vertical export
```

The SQLite index is `~/Library/Application Support/SpeakEasy/speakeasy.sqlite`.

## Useful commands

```bash
cargo test
cargo build --release
./scripts/package-macos.sh
```

`cargo run` uses the ffmpeg on your `PATH`. The package script builds `dist/SpeakEasy.app` and copies static `ffmpeg` / `ffprobe` into the bundle. It does not copy Application Support or any API key.

To sign a distribution build:

```bash
CODESIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" ./scripts/package-macos.sh
```
