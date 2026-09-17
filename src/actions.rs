use crate::app::{Job, Screen, SpeakEasyApp};
use crate::db::Project;
use crate::gemini;
use crate::storage::{
    ProjectYaml, create_project_dir, save_config, sanitize_folder_name, write_project_yaml,
};
use std::path::PathBuf;
use std::thread;

impl SpeakEasyApp {
    pub(crate) fn persist_settings(&mut self) {
        self.config.resolution = self.quality.label().to_string();
        self.config.video_source = self.selected_camera.as_ref().and_then(|index| {
            self.cameras
                .iter()
                .find(|camera| &camera.index == index)
                .map(|camera| camera.name.clone())
        });
        self.config.microphone = self.selected_mic.clone();
        if let Err(err) = save_config(&self.config) {
            self.status = format!("Could not save settings: {err}");
        }
    }

    pub(crate) fn studio_selection(&self) -> Option<(String, usize)> {
        match &self.screen {
            Screen::Studio {
                project_id,
                selected,
            } => Some((project_id.clone(), *selected)),
            _ => None,
        }
    }

    pub(crate) fn finish_project_id(&self) -> Option<String> {
        match &self.screen {
            Screen::Finish { project_id } => Some(project_id.clone()),
            _ => None,
        }
    }

    pub(crate) fn selected_question_answered(&self) -> bool {
        let Some((project_id, selected)) = self.studio_selection() else {
            return false;
        };
        self.db
            .questions(&project_id)
            .ok()
            .and_then(|rows| rows.into_iter().find(|row| row.position == selected as i64))
            .is_some_and(|row| row.answered())
    }

    pub(crate) fn wants_live_preview(&self) -> bool {
        matches!(self.screen, Screen::Studio { .. })
            && (self.recording.is_some() || self.re_recording || !self.selected_question_answered())
    }

    pub(crate) fn wants_playback(&self) -> bool {
        matches!(self.screen, Screen::Studio { .. })
            && self.recording.is_none()
            && !self.re_recording
            && self.selected_question_answered()
    }

    pub(crate) fn selected_media_paths(&self) -> Option<(PathBuf, Option<PathBuf>)> {
        let (project_id, selected) = self.studio_selection()?;
        let project = self.db.project(&project_id).ok()?;
        let question = self
            .db
            .questions(&project_id)
            .ok()?
            .into_iter()
            .find(|row| row.position == selected as i64)?;
        let square = project.dir().join(question.square_path.as_ref()?);
        let wav = question.wav_path.map(|name| project.dir().join(name));
        Some((square, wav))
    }

    pub(crate) fn submit_key(&mut self) {
        let key = self.gate_key.trim().to_string();
        self.busy = true;
        self.gate_error = None;
        let tx = self.job_tx.clone();
        thread::spawn(move || {
            let job = match gemini::validate_key(&key) {
                Ok(()) => Job::KeyOk,
                Err(err) => Job::KeyErr(err),
            };
            let _ = tx.send(job);
        });
    }

    pub(crate) fn create_project(&mut self) {
        let title = self.new_title.trim().to_string();
        if title.is_empty() {
            self.form_error = Some("Give the project a name.".into());
            return;
        }
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = chrono::Local::now().to_rfc3339();
        let dir = match create_project_dir(&title, &id) {
            Ok(dir) => dir,
            Err(err) => {
                self.form_error = Some(err);
                return;
            }
        };
        let yaml = ProjectYaml {
            id: id.clone(),
            title: title.clone(),
            topic: None,
            created_at: created_at.clone(),
        };
        if let Err(err) = write_project_yaml(&dir, &yaml) {
            self.form_error = Some(err);
            return;
        }
        let project = Project {
            id: id.clone(),
            title,
            slug: sanitize_folder_name(&self.new_title),
            movies_path: dir.to_string_lossy().into_owned(),
            topic: None,
            created_at,
        };
        if let Err(err) = self.db.insert_project(&project) {
            self.form_error = Some(err);
            return;
        }
        self.new_title.clear();
        self.topic_input.clear();
        self.form_error = None;
        self.screen = Screen::Topic { project_id: id };
    }

    pub(crate) fn open_project(&mut self, project_id: String) {
        match self.db.project(&project_id) {
            Ok(project) => {
                self.form_error = None;
                self.re_recording = false;
                self.topic_input = project.topic.clone().unwrap_or_default();
                if project.topic.is_none() {
                    self.screen = Screen::Topic { project_id };
                    return;
                }
                match self.db.questions(&project_id) {
                    Ok(questions) if questions.is_empty() => {
                        self.screen = Screen::Topic { project_id };
                    }
                    Ok(_) => {
                        self.screen = Screen::Studio {
                            project_id,
                            selected: 1,
                        };
                    }
                    Err(err) => self.status = err,
                }
            }
            Err(err) => self.status = err,
        }
    }

    pub(crate) fn submit_topic(&mut self) {
        let Screen::Topic { project_id } = &self.screen else {
            return;
        };
        let project_id = project_id.clone();
        let topic = self.topic_input.trim().to_string();
        if topic.is_empty() {
            self.form_error = Some("What would you like to speak about?".into());
            return;
        }
        if let Err(err) = self.db.set_topic(&project_id, &topic) {
            self.form_error = Some(err);
            return;
        }
        if let Ok(project) = self.db.project(&project_id) {
            let dir = project.dir();
            let yaml = ProjectYaml {
                id: project.id,
                title: project.title,
                topic: Some(topic.clone()),
                created_at: project.created_at,
            };
            let _ = write_project_yaml(&dir, &yaml);
        }
        self.busy = true;
        self.form_error = None;
        self.status = "Asking Gemini for 10 questions…".into();
        let key = self.config.gemini_api_key.clone();
        let tx = self.job_tx.clone();
        thread::spawn(move || {
            let job = match gemini::generate_questions(&key, &topic) {
                Ok(questions) => Job::QuestionsOk {
                    project_id,
                    questions,
                },
                Err(err) => Job::QuestionsErr(err),
            };
            let _ = tx.send(job);
        });
    }
}
