use serde::{Deserialize, Serialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub gemini_api_key: String,
    #[serde(default = "default_resolution")]
    pub resolution: String,
    pub video_source: Option<String>,
    pub microphone: Option<String>,
}

fn default_resolution() -> String {
    "1080p".into()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            gemini_api_key: String::new(),
            resolution: default_resolution(),
            video_source: None,
            microphone: None,
        }
    }
}

impl AppConfig {
    pub fn has_key(&self) -> bool {
        !self.gemini_api_key.trim().is_empty()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectYaml {
    pub id: String,
    pub title: String,
    pub topic: Option<String>,
    pub created_at: String,
}

pub fn app_support_dir() -> Result<PathBuf, String> {
    let dir = dirs::data_dir()
        .ok_or("Could not find Application Support")?
        .join("SpeakEasy");
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir)
}

pub fn config_path() -> Result<PathBuf, String> {
    Ok(app_support_dir()?.join("config.yaml"))
}

pub fn db_path() -> Result<PathBuf, String> {
    Ok(app_support_dir()?.join("speakeasy.sqlite"))
}

pub fn movies_root() -> Result<PathBuf, String> {
    let videos = dirs::video_dir()
        .or_else(|| dirs::home_dir().map(|home| home.join("Movies")))
        .ok_or("Could not find the Movies folder")?;
    Ok(videos.join("SpeakEasy"))
}

pub fn load_config() -> AppConfig {
    let Ok(path) = config_path() else {
        return AppConfig::default();
    };
    let Ok(text) = fs::read_to_string(path) else {
        return AppConfig::default();
    };
    serde_yaml::from_str(&text).unwrap_or_default()
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let path = config_path()?;
    let text = serde_yaml::to_string(config).map_err(|err| err.to_string())?;
    fs::write(&path, text).map_err(|err| err.to_string())?;
    let mut perms = fs::metadata(&path)
        .map_err(|err| err.to_string())?
        .permissions();
    perms.set_mode(0o600);
    fs::set_permissions(&path, perms).map_err(|err| err.to_string())?;
    Ok(())
}

pub fn sanitize_folder_name(title: &str) -> String {
    let mut out = String::new();
    for ch in title.trim().chars() {
        match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => out.push('-'),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    let trimmed = out.trim().trim_matches('.').trim();
    if trimmed.is_empty() {
        "Untitled".into()
    } else {
        trimmed.into()
    }
}

pub fn create_project_dir(title: &str, id: &str) -> Result<PathBuf, String> {
    let root = movies_root()?;
    fs::create_dir_all(&root).map_err(|err| err.to_string())?;
    let base = sanitize_folder_name(title);
    let mut dir = root.join(&base);
    if dir.exists() {
        let same_project = read_project_yaml(&dir)
            .ok()
            .is_some_and(|yaml| yaml.id == id);
        if !same_project {
            let suffix = id.get(..8).unwrap_or(id);
            dir = root.join(format!("{base}-{suffix}"));
        }
    }
    fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
    Ok(dir)
}

pub fn write_project_yaml(dir: &Path, yaml: &ProjectYaml) -> Result<(), String> {
    let text = serde_yaml::to_string(yaml).map_err(|err| err.to_string())?;
    fs::write(dir.join("project.yaml"), text).map_err(|err| err.to_string())
}

pub fn read_project_yaml(dir: &Path) -> Result<ProjectYaml, String> {
    let text = fs::read_to_string(dir.join("project.yaml")).map_err(|err| err.to_string())?;
    serde_yaml::from_str(&text).map_err(|err| err.to_string())
}
