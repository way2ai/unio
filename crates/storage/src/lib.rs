use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use unio_core::{now_utc, ApprovalId, RunId, SessionId};
use unio_protocol::{ApprovalGrantSummary, PermissionMode, SessionSummary, TranscriptMessage};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: SessionId,
    pub title: String,
    pub workspace_root: PathBuf,
    pub permission_mode: PermissionMode,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_run_id: Option<RunId>,
}

pub struct SqliteSessionStore {
    conn: Connection,
}

impl SqliteSessionStore {
    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "create table if not exists sessions (
                session_id text primary key,
                title text not null,
                workspace_root text not null,
                permission_mode text not null,
                created_at text not null,
                updated_at text not null,
                last_run_id text
            );
            create table if not exists runs (
                run_id text primary key,
                session_id text not null,
                prompt text not null,
                final_text text,
                trace_id text not null,
                provider text not null,
                model text not null,
                input_tokens integer not null,
                output_tokens integer not null,
                context_ratio real not null,
                created_at text not null
            );
            create table if not exists approval_grants (
                approval_id text primary key,
                tool_call_id text not null,
                tool_name text not null,
                workspace_root text not null,
                approved integer not null,
                reason text,
                resolved_at text not null
            );",
        )?;
        Ok(Self { conn })
    }

    pub fn insert_session(&self, session: &SessionRecord) -> anyhow::Result<()> {
        self.conn.execute(
            "insert into sessions
             (session_id, title, workspace_root, permission_mode, created_at, updated_at, last_run_id)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             on conflict(session_id) do update set
                title = excluded.title,
                workspace_root = excluded.workspace_root,
                permission_mode = excluded.permission_mode,
                updated_at = excluded.updated_at,
                last_run_id = excluded.last_run_id",
            params![
                session.session_id.to_string(),
                session.title,
                session.workspace_root.to_string_lossy(),
                format!("{:?}", session.permission_mode),
                session.created_at.to_rfc3339(),
                session.updated_at.to_rfc3339(),
                session.last_run_id.as_ref().map(ToString::to_string)
            ],
        )?;
        Ok(())
    }

    pub fn resolve_session(
        &self,
        workspace_root: impl AsRef<Path>,
        permission_mode: PermissionMode,
    ) -> anyhow::Result<SessionSummary> {
        let workspace_root = workspace_root.as_ref().to_string_lossy().to_string();
        if let Some(session) = self.find_by_workspace(&workspace_root)? {
            return Ok(session);
        }

        let now = now_utc();
        let record = SessionRecord {
            session_id: SessionId::new(),
            title: workspace_root.clone(),
            workspace_root: PathBuf::from(&workspace_root),
            permission_mode,
            created_at: now,
            updated_at: now,
            last_run_id: None,
        };
        self.insert_session(&record)?;
        Ok(record.into())
    }

    pub fn list_sessions(&self) -> anyhow::Result<Vec<SessionSummary>> {
        let mut stmt = self.conn.prepare(
            "select session_id, title, workspace_root, permission_mode, created_at, updated_at, last_run_id
             from sessions order by updated_at desc",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(SessionSummary {
                session_id: SessionId::from_string(row.get::<_, String>(0)?),
                title: row.get(1)?,
                workspace_root: row.get(2)?,
                permission_mode: parse_permission_mode(row.get::<_, String>(3)?),
                created_at: parse_time(row.get::<_, String>(4)?),
                updated_at: parse_time(row.get::<_, String>(5)?),
                last_run_id: row.get::<_, Option<String>>(6)?.map(RunId::from_string),
            })
        })?;
        let mut sessions = Vec::new();
        for row in rows {
            sessions.push(row?);
        }
        Ok(sessions)
    }

    pub fn find_session(&self, session_id: &SessionId) -> anyhow::Result<Option<SessionSummary>> {
        let mut stmt = self.conn.prepare(
            "select session_id, title, workspace_root, permission_mode, created_at, updated_at, last_run_id
             from sessions where session_id = ?1",
        )?;
        let mut rows = stmt.query([session_id.to_string()])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(SessionSummary {
            session_id: SessionId::from_string(row.get::<_, String>(0)?),
            title: row.get(1)?,
            workspace_root: row.get(2)?,
            permission_mode: parse_permission_mode(row.get::<_, String>(3)?),
            created_at: parse_time(row.get::<_, String>(4)?),
            updated_at: parse_time(row.get::<_, String>(5)?),
            last_run_id: row.get::<_, Option<String>>(6)?.map(RunId::from_string),
        }))
    }

    pub fn find_by_workspace(
        &self,
        workspace_root: &str,
    ) -> anyhow::Result<Option<SessionSummary>> {
        let mut stmt = self.conn.prepare(
            "select session_id, title, workspace_root, permission_mode, created_at, updated_at, last_run_id
             from sessions where workspace_root = ?1 order by updated_at desc limit 1",
        )?;
        let mut rows = stmt.query([workspace_root])?;
        let Some(row) = rows.next()? else {
            return Ok(None);
        };
        Ok(Some(SessionSummary {
            session_id: SessionId::from_string(row.get::<_, String>(0)?),
            title: row.get(1)?,
            workspace_root: row.get(2)?,
            permission_mode: parse_permission_mode(row.get::<_, String>(3)?),
            created_at: parse_time(row.get::<_, String>(4)?),
            updated_at: parse_time(row.get::<_, String>(5)?),
            last_run_id: row.get::<_, Option<String>>(6)?.map(RunId::from_string),
        }))
    }

    pub fn touch_session(&self, session_id: &SessionId, run_id: &RunId) -> anyhow::Result<()> {
        self.conn.execute(
            "update sessions set updated_at = ?2, last_run_id = ?3 where session_id = ?1",
            params![
                session_id.to_string(),
                now_utc().to_rfc3339(),
                run_id.to_string()
            ],
        )?;
        Ok(())
    }

    pub fn insert_run(&self, run: &RunRecord) -> anyhow::Result<()> {
        self.conn.execute(
            "insert into runs
             (run_id, session_id, prompt, final_text, trace_id, provider, model, input_tokens, output_tokens, context_ratio, created_at)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                run.run_id.to_string(),
                run.session_id.to_string(),
                run.prompt,
                run.final_text,
                run.trace_id,
                run.provider,
                run.model,
                run.input_tokens as i64,
                run.output_tokens as i64,
                run.context_ratio,
                run.created_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn latest_context_ratio(&self) -> anyhow::Result<Option<f32>> {
        self.conn
            .query_row(
                "select context_ratio from runs order by created_at desc limit 1",
                [],
                |row| row.get::<_, f64>(0).map(|value| value as f32),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn insert_approval_grant(&self, grant: &ApprovalGrantRecord) -> anyhow::Result<()> {
        self.conn.execute(
            "insert into approval_grants
             (approval_id, tool_call_id, tool_name, workspace_root, approved, reason, resolved_at)
             values (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                grant.approval_id.to_string(),
                grant.tool_call_id,
                grant.tool_name,
                grant.workspace_root.to_string_lossy(),
                if grant.approved { 1i64 } else { 0i64 },
                grant.reason,
                grant.resolved_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    pub fn list_approval_grants(&self) -> anyhow::Result<Vec<ApprovalGrantSummary>> {
        let mut stmt = self.conn.prepare(
            "select approval_id, tool_call_id, tool_name, workspace_root, approved, reason, resolved_at
             from approval_grants order by resolved_at desc",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(ApprovalGrantSummary {
                approval_id: ApprovalId::from_string(row.get::<_, String>(0)?),
                tool_call_id: row.get(1)?,
                tool_name: row.get(2)?,
                workspace_root: row.get(3)?,
                approved: row.get::<_, i64>(4)? != 0,
                reason: row.get(5)?,
                resolved_at: parse_time(row.get::<_, String>(6)?),
            })
        })?;
        let mut grants = Vec::new();
        for row in rows {
            grants.push(row?);
        }
        Ok(grants)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: RunId,
    pub session_id: SessionId,
    pub prompt: String,
    pub final_text: String,
    pub trace_id: String,
    pub provider: String,
    pub model: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub context_ratio: f32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalGrantRecord {
    pub approval_id: ApprovalId,
    pub tool_call_id: String,
    pub tool_name: String,
    pub workspace_root: PathBuf,
    pub approved: bool,
    pub reason: Option<String>,
    pub resolved_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct JsonlTranscriptStore {
    path: PathBuf,
}

impl JsonlTranscriptStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn append(&self, message: &TranscriptMessage) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, message)?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(())
    }

    pub fn read_all(&self) -> anyhow::Result<Vec<TranscriptMessage>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&self.path)?;
        let mut messages = Vec::new();
        for line in content.lines().filter(|line| !line.trim().is_empty()) {
            messages.push(serde_json::from_str(line)?);
        }
        Ok(messages)
    }
}

impl From<SessionRecord> for SessionSummary {
    fn from(value: SessionRecord) -> Self {
        Self {
            session_id: value.session_id,
            title: value.title,
            workspace_root: value.workspace_root.to_string_lossy().to_string(),
            permission_mode: value.permission_mode,
            created_at: value.created_at,
            updated_at: value.updated_at,
            last_run_id: value.last_run_id,
        }
    }
}

fn parse_permission_mode(value: String) -> PermissionMode {
    match value.as_str() {
        "Auto" => PermissionMode::Auto,
        "FullTrust" => PermissionMode::FullTrust,
        _ => PermissionMode::Default,
    }
}

fn parse_time(value: String) -> DateTime<Utc> {
    chrono::DateTime::parse_from_rfc3339(&value)
        .map(|value| value.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use tempfile::tempdir;
    use unio_core::{ApprovalId, RunId, SessionId};
    use unio_protocol::{PermissionMode, TranscriptMessage};

    use super::{
        ApprovalGrantRecord, JsonlTranscriptStore, RunRecord, SessionRecord, SqliteSessionStore,
    };

    #[test]
    fn sqlite_session_store_accepts_v1_session_record() {
        let dir = tempdir().unwrap();
        let store = SqliteSessionStore::open(dir.path().join("state.db")).unwrap();
        let now = Utc::now();
        let session = SessionRecord {
            session_id: SessionId::from_string("session_1"),
            title: "demo".into(),
            workspace_root: dir.path().to_path_buf(),
            permission_mode: PermissionMode::Default,
            created_at: now,
            updated_at: now,
            last_run_id: None,
        };

        store.insert_session(&session).unwrap();
    }

    #[test]
    fn resolves_session_by_workspace() {
        let dir = tempdir().unwrap();
        let store = SqliteSessionStore::open(dir.path().join("state.db")).unwrap();
        let session = store
            .resolve_session(dir.path().join("repo"), PermissionMode::Auto)
            .unwrap();
        let same = store
            .resolve_session(dir.path().join("repo"), PermissionMode::Default)
            .unwrap();
        assert_eq!(session.session_id, same.session_id);
        assert_eq!(same.permission_mode, PermissionMode::Auto);
    }

    #[test]
    fn jsonl_transcript_store_writes_message_level_records() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("transcript.jsonl");
        let store = JsonlTranscriptStore::new(&path);
        let message = TranscriptMessage::User {
            session_id: SessionId::from_string("session_1"),
            run_id: RunId::from_string("run_1"),
            content: "hello".into(),
            recorded_at: Utc::now(),
        };

        store.append(&message).unwrap();

        let written = std::fs::read_to_string(path).unwrap();
        assert!(written.contains("\"role\":\"user\""));
        let loaded = store.read_all().unwrap();
        assert_eq!(loaded.len(), 1);
    }

    #[test]
    fn sqlite_session_store_persists_approval_grants() {
        let dir = tempdir().unwrap();
        let store = SqliteSessionStore::open(dir.path().join("state.db")).unwrap();

        store
            .insert_approval_grant(&ApprovalGrantRecord {
                approval_id: ApprovalId::from_string("approval_1"),
                tool_call_id: "tool_1".into(),
                tool_name: "write".into(),
                workspace_root: dir.path().to_path_buf(),
                approved: true,
                reason: None,
                resolved_at: Utc::now(),
            })
            .unwrap();

        let grants = store.list_approval_grants().unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(grants[0].tool_name, "write");
        assert!(grants[0].approved);
    }

    #[test]
    fn sqlite_session_store_reports_latest_context_ratio() {
        let dir = tempdir().unwrap();
        let store = SqliteSessionStore::open(dir.path().join("state.db")).unwrap();
        let session_id = SessionId::from_string("session_1");

        store
            .insert_run(&RunRecord {
                run_id: RunId::from_string("run_1"),
                session_id,
                prompt: "hello".into(),
                final_text: "world".into(),
                trace_id: "trace_1".into(),
                provider: "mock".into(),
                model: "mock".into(),
                input_tokens: 10,
                output_tokens: 5,
                context_ratio: 0.42,
                created_at: Utc::now(),
            })
            .unwrap();

        assert_eq!(store.latest_context_ratio().unwrap(), Some(0.42));
    }
}
