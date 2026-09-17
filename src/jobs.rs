use crate::app::{Job, Screen, SpeakEasyApp};
use crate::export::export_pair;
use crate::gemini;
use std::thread;

impl SpeakEasyApp {
    pub(crate) fn start_transcribe(&mut self) {
        let Some(project_id) = self.finish_project_id() else {
            return;
        };
        let Ok(project) = self.db.project(&project_id) else {
            return;
        };
        let Ok(questions) = self.db.questions(&project_id) else {
            return;
        };
        let pending: Vec<_> = questions
            .into_iter()
            .filter(|row| row.answered() && !row.transcribed())
            .collect();
        if pending.is_empty() {
            self.status = "Every answered question already has a transcript.".into();
            return;
        }
        self.busy = true;
        self.transcribe_progress = Some((0, pending.len()));
        self.status = format!("Transcribing 0 / {}", pending.len());
        let key = self.config.gemini_api_key.clone();
        let dir = project.dir();
        let tx = self.job_tx.clone();
        thread::spawn(move || {
            for (index, row) in pending.iter().enumerate() {
                let Some(wav_name) = &row.wav_path else {
                    continue;
                };
                match gemini::transcribe(&key, &dir.join(wav_name)) {
                    Ok(text) => {
                        let _ = tx.send(Job::Transcript {
                            question_id: row.id,
                            text,
                        });
                        let _ = tx.send(Job::TranscribeProgress {
                            done: index + 1,
                            total: pending.len(),
                        });
                    }
                    Err(err) => {
                        let _ = tx.send(Job::TranscribeErr(err));
                        return;
                    }
                }
            }
            let _ = tx.send(Job::TranscribeDone);
        });
    }

    pub(crate) fn start_export(&mut self) {
        let Some(project_id) = self.finish_project_id() else {
            return;
        };
        let Ok(project) = self.db.project(&project_id) else {
            return;
        };
        let Ok(questions) = self.db.questions(&project_id) else {
            return;
        };
        let ready: Vec<_> = questions.into_iter().filter(|row| row.answered()).collect();
        if ready.iter().any(|row| !row.transcribed()) {
            self.status = "Transcribe every answered question before export.".into();
            return;
        }
        if ready.is_empty() {
            self.status = "Record at least one answer before export.".into();
            return;
        }
        self.busy = true;
        self.status = "Exporting horizontal and vertical videos…".into();
        let burn = self.burn_captions;
        let dir = project.dir();
        let tx = self.job_tx.clone();
        thread::spawn(move || {
            for row in &ready {
                let captions = if burn {
                    row.transcript.as_deref()
                } else {
                    None
                };
                if let Err(err) = export_pair(&dir, row.position, captions) {
                    let _ = tx.send(Job::ExportErr(err));
                    return;
                }
            }
            let _ = tx.send(Job::ExportDone(ready.len()));
        });
    }

    pub(crate) fn drain_jobs(&mut self) {
        while let Ok(job) = self.job_rx.try_recv() {
            match job {
                Job::KeyOk => {
                    self.config.gemini_api_key = self.gate_key.trim().to_string();
                    self.persist_settings();
                    self.busy = false;
                    self.gate_error = None;
                    self.screen = Screen::Home;
                }
                Job::KeyErr(err) => {
                    self.busy = false;
                    self.gate_error = Some(err);
                }
                Job::QuestionsOk {
                    project_id,
                    questions,
                } => {
                    self.busy = false;
                    match self.db.replace_questions(&project_id, &questions) {
                        Ok(()) => {
                            self.status.clear();
                            self.screen = Screen::Studio {
                                project_id,
                                selected: 1,
                            };
                        }
                        Err(err) => self.form_error = Some(err),
                    }
                }
                Job::QuestionsErr(err) => {
                    self.busy = false;
                    self.form_error = Some(err.clone());
                    self.status = err;
                }
                Job::Transcript { question_id, text } => {
                    let created = chrono::Local::now().to_rfc3339();
                    if let Err(err) = self.db.set_transcript(question_id, &text, &created) {
                        self.status = err;
                    }
                }
                Job::Recorded {
                    question_id,
                    square,
                    wav,
                } => {
                    self.saving = false;
                    self.re_recording = false;
                    let recorded_at = chrono::Local::now().to_rfc3339();
                    match self
                        .db
                        .upsert_answer(question_id, &square, &wav, &recorded_at)
                    {
                        Ok(()) => self.status = format!("Saved {square}"),
                        Err(err) => self.status = err,
                    }
                }
                Job::RecordErr(err) => {
                    self.saving = false;
                    self.status = err;
                }
                Job::TranscribeProgress { done, total } => {
                    self.transcribe_progress = Some((done, total));
                    self.status = format!("Transcribing {done} / {total}");
                }
                Job::TranscribeDone => {
                    self.busy = false;
                    self.status = "Transcription finished.".into();
                }
                Job::TranscribeErr(err) => {
                    self.busy = false;
                    self.status = err;
                }
                Job::ExportDone(count) => {
                    self.busy = false;
                    self.status = format!("Exported {count} answer pair(s) to Movies.");
                }
                Job::ExportErr(err) => {
                    self.busy = false;
                    self.status = err;
                }
            }
        }
    }
}
