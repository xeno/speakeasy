use crate::storage::db_path;
use rusqlite::{Connection, params};
use std::path::PathBuf;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    slug TEXT NOT NULL,
    movies_path TEXT NOT NULL,
    topic TEXT,
    created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS questions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    text TEXT NOT NULL,
    UNIQUE(project_id, position),
    FOREIGN KEY(project_id) REFERENCES projects(id)
);
CREATE TABLE IF NOT EXISTS answers (
    question_id INTEGER PRIMARY KEY,
    square_path TEXT NOT NULL,
    wav_path TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    FOREIGN KEY(question_id) REFERENCES questions(id)
);
CREATE TABLE IF NOT EXISTS transcripts (
    question_id INTEGER PRIMARY KEY,
    text TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(question_id) REFERENCES questions(id)
);
";

pub struct Database {
    conn: Connection,
}

#[derive(Clone, Debug)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub movies_path: String,
    pub topic: Option<String>,
    pub created_at: String,
}

impl Project {
    pub fn dir(&self) -> PathBuf {
        PathBuf::from(&self.movies_path)
    }
}

#[derive(Clone, Debug)]
pub struct QuestionRow {
    pub id: i64,
    pub position: i64,
    pub text: String,
    pub square_path: Option<String>,
    pub wav_path: Option<String>,
    pub transcript: Option<String>,
}

impl QuestionRow {
    pub fn answered(&self) -> bool {
        self.square_path.is_some()
    }

    pub fn transcribed(&self) -> bool {
        self.transcript
            .as_ref()
            .is_some_and(|text| !text.trim().is_empty())
    }
}

impl Database {
    pub fn open() -> Result<Self, String> {
        let path = db_path()?;
        Self::from_connection(Connection::open(path).map_err(|err| err.to_string())?)
    }

    pub fn open_in_memory() -> Self {
        Self::from_connection(Connection::open_in_memory().expect("in-memory sqlite"))
            .expect("in-memory schema")
    }

    fn from_connection(conn: Connection) -> Result<Self, String> {
        conn.execute_batch(SCHEMA).map_err(|err| err.to_string())?;
        Ok(Self { conn })
    }

    pub fn insert_project(&self, project: &Project) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO projects (id, title, slug, movies_path, topic, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    project.id,
                    project.title,
                    project.slug,
                    project.movies_path,
                    project.topic,
                    project.created_at
                ],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn set_topic(&self, project_id: &str, topic: &str) -> Result<(), String> {
        self.conn
            .execute(
                "UPDATE projects SET topic = ?1 WHERE id = ?2",
                params![topic, project_id],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn project(&self, id: &str) -> Result<Project, String> {
        self.conn
            .query_row(
                "SELECT id, title, slug, movies_path, topic, created_at
                 FROM projects WHERE id = ?1",
                params![id],
                row_to_project,
            )
            .map_err(|err| err.to_string())
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, slug, movies_path, topic, created_at
                 FROM projects ORDER BY created_at DESC",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map([], row_to_project)
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    pub fn replace_questions(&self, project_id: &str, questions: &[String]) -> Result<(), String> {
        self.conn
            .execute(
                "DELETE FROM transcripts WHERE question_id IN
                 (SELECT id FROM questions WHERE project_id = ?1)",
                params![project_id],
            )
            .map_err(|err| err.to_string())?;
        self.conn
            .execute(
                "DELETE FROM answers WHERE question_id IN
                 (SELECT id FROM questions WHERE project_id = ?1)",
                params![project_id],
            )
            .map_err(|err| err.to_string())?;
        self.conn
            .execute(
                "DELETE FROM questions WHERE project_id = ?1",
                params![project_id],
            )
            .map_err(|err| err.to_string())?;
        for (index, text) in questions.iter().enumerate() {
            self.conn
                .execute(
                    "INSERT INTO questions (project_id, position, text) VALUES (?1, ?2, ?3)",
                    params![project_id, (index + 1) as i64, text],
                )
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }

    pub fn questions(&self, project_id: &str) -> Result<Vec<QuestionRow>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT q.id, q.position, q.text, a.square_path, a.wav_path, t.text
                 FROM questions q
                 LEFT JOIN answers a ON a.question_id = q.id
                 LEFT JOIN transcripts t ON t.question_id = q.id
                 WHERE q.project_id = ?1
                 ORDER BY q.position",
            )
            .map_err(|err| err.to_string())?;
        let rows = stmt
            .query_map(params![project_id], |row| {
                Ok(QuestionRow {
                    id: row.get(0)?,
                    position: row.get(1)?,
                    text: row.get(2)?,
                    square_path: row.get(3)?,
                    wav_path: row.get(4)?,
                    transcript: row.get(5)?,
                })
            })
            .map_err(|err| err.to_string())?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|err| err.to_string())
    }

    pub fn upsert_answer(
        &self,
        question_id: i64,
        square_path: &str,
        wav_path: &str,
        recorded_at: &str,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO answers (question_id, square_path, wav_path, recorded_at)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(question_id) DO UPDATE SET
                    square_path = excluded.square_path,
                    wav_path = excluded.wav_path,
                    recorded_at = excluded.recorded_at",
                params![question_id, square_path, wav_path, recorded_at],
            )
            .map_err(|err| err.to_string())?;
        self.conn
            .execute(
                "DELETE FROM transcripts WHERE question_id = ?1",
                params![question_id],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    pub fn set_transcript(
        &self,
        question_id: i64,
        text: &str,
        created_at: &str,
    ) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO transcripts (question_id, text, created_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(question_id) DO UPDATE SET
                    text = excluded.text,
                    created_at = excluded.created_at",
                params![question_id, text, created_at],
            )
            .map_err(|err| err.to_string())?;
        Ok(())
    }
}

fn row_to_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    Ok(Project {
        id: row.get(0)?,
        title: row.get(1)?,
        slug: row.get(2)?,
        movies_path: row.get(3)?,
        topic: row.get(4)?,
        created_at: row.get(5)?,
    })
}
