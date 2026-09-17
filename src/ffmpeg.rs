use std::path::PathBuf;
use std::process::Command;

pub fn command() -> Command {
    Command::new(resolve("ffmpeg"))
}

pub fn probe() -> Command {
    Command::new(resolve("ffprobe"))
}

pub fn missing_message() -> &'static str {
    "ffmpeg not found. Install it with: brew install ffmpeg — or run scripts/package-macos.sh to bundle it."
}

fn resolve(name: &str) -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let bundled = dir.join(name);
        if bundled.is_file() {
            return bundled;
        }
        if let Some(contents) = dir.parent() {
            let resources = contents.join("Resources").join(name);
            if resources.is_file() {
                return resources;
            }
        }
    }
    PathBuf::from(name)
}
