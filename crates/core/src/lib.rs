use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, Uuid::new_v4()))
            }

            pub fn from_string(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

id_type!(SessionId, "session");
id_type!(RunId, "run");
id_type!(TraceId, "trace");
id_type!(ApprovalId, "approval");

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentId(String);

impl AgentId {
    pub fn new(kind: AgentKind) -> Self {
        Self(format!("{}_{}", kind.as_prefix(), Uuid::new_v4()))
    }

    pub fn new_root() -> Self {
        Self::new(AgentKind::Root)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentKind {
    Root,
    Planner,
    SubAgent,
    SkillAgent,
}

impl AgentKind {
    pub fn as_prefix(self) -> &'static str {
        match self {
            Self::Root => "agent_root",
            Self::Planner => "agent_planner",
            Self::SubAgent => "agent_sub",
            Self::SkillAgent => "agent_skill",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspacePaths {
    pub workspace_root: PathBuf,
    pub user_unio_root: PathBuf,
    pub workspace_unio_root: PathBuf,
}

impl WorkspacePaths {
    pub fn new(user_home: impl AsRef<Path>, workspace_root: impl AsRef<Path>) -> Self {
        let user_unio_root = user_home.as_ref().join(".unio");
        let workspace_root = workspace_root.as_ref().to_path_buf();
        let workspace_unio_root = workspace_root.join(".unio");
        Self {
            workspace_root,
            user_unio_root,
            workspace_unio_root,
        }
    }

    pub fn user_skills_dir(&self) -> PathBuf {
        self.user_unio_root.join("skills")
    }

    pub fn workspace_skills_dir(&self) -> PathBuf {
        self.workspace_unio_root.join("skills")
    }
}

pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserPaths {
    pub root: PathBuf,
    pub daemon_dir: PathBuf,
    pub daemon_logs_dir: PathBuf,
    pub instance_file: PathBuf,
    pub sessions_dir: PathBuf,
    pub state_db_file: PathBuf,
    pub transcripts_dir: PathBuf,
    pub traces_dir: PathBuf,
    pub traces_file: PathBuf,
}

impl UserPaths {
    pub fn current() -> anyhow::Result<Self> {
        let user_home = std::env::var_os("USERPROFILE")
            .or_else(|| std::env::var_os("HOME"))
            .map(PathBuf::from)
            .ok_or_else(|| anyhow::anyhow!("failed to resolve user home"))?;
        let root = user_home.join(".unio");
        let daemon_dir = root.join("daemon");
        let daemon_logs_dir = daemon_dir.join("logs");
        let sessions_dir = root.join("sessions");
        let traces_dir = root.join("traces");
        Ok(Self {
            root: root.clone(),
            daemon_dir: daemon_dir.clone(),
            daemon_logs_dir,
            instance_file: daemon_dir.join("instance.json"),
            sessions_dir: sessions_dir.clone(),
            state_db_file: sessions_dir.join("state.db"),
            transcripts_dir: sessions_dir.join("transcripts"),
            traces_dir: traces_dir.clone(),
            traces_file: traces_dir.join("events.jsonl"),
        })
    }

    pub fn ensure(&self) -> anyhow::Result<()> {
        for dir in [
            &self.root,
            &self.daemon_dir,
            &self.daemon_logs_dir,
            &self.sessions_dir,
            &self.transcripts_dir,
            &self.traces_dir,
        ] {
            fs::create_dir_all(dir)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaemonInstance {
    pub pid: u32,
    pub http_url: String,
    pub started_at: DateTime<Utc>,
}

pub fn write_instance_file(paths: &UserPaths, instance: &DaemonInstance) -> anyhow::Result<()> {
    paths.ensure()?;
    fs::write(&paths.instance_file, serde_json::to_vec_pretty(instance)?)?;
    Ok(())
}

pub fn read_instance_file(paths: &UserPaths) -> anyhow::Result<Option<DaemonInstance>> {
    if !paths.instance_file.exists() {
        return Ok(None);
    }
    let bytes = fs::read(&paths.instance_file)?;
    Ok(Some(serde_json::from_slice(&bytes)?))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        read_instance_file, write_instance_file, AgentId, AgentKind, ApprovalId, DaemonInstance,
        RunId, SessionId, TraceId, UserPaths, WorkspacePaths,
    };

    #[test]
    fn generated_ids_have_stable_prefixes() {
        assert!(SessionId::new().as_str().starts_with("session_"));
        assert!(RunId::new().as_str().starts_with("run_"));
        assert!(TraceId::new().as_str().starts_with("trace_"));
        assert!(ApprovalId::new().as_str().starts_with("approval_"));
        assert!(AgentId::new(AgentKind::Planner)
            .as_str()
            .starts_with("agent_planner_"));
    }

    #[test]
    fn skill_paths_use_unio_roots() {
        let paths = WorkspacePaths::new("/home/me", "/repo");

        assert!(paths.user_skills_dir().ends_with(".unio/skills"));
        assert!(paths.workspace_skills_dir().ends_with(".unio/skills"));
    }

    #[test]
    fn daemon_instance_roundtrip_works() {
        let tmp = tempdir().unwrap();
        let paths = UserPaths {
            root: tmp.path().join(".unio"),
            daemon_dir: tmp.path().join(".unio/daemon"),
            daemon_logs_dir: tmp.path().join(".unio/daemon/logs"),
            instance_file: tmp.path().join(".unio/daemon/instance.json"),
            sessions_dir: tmp.path().join(".unio/sessions"),
            state_db_file: tmp.path().join(".unio/sessions/state.db"),
            transcripts_dir: tmp.path().join(".unio/sessions/transcripts"),
            traces_dir: tmp.path().join(".unio/traces"),
            traces_file: tmp.path().join(".unio/traces/events.jsonl"),
        };
        let instance = DaemonInstance {
            pid: 1,
            http_url: "http://127.0.0.1:7878".into(),
            started_at: chrono::Utc::now(),
        };

        write_instance_file(&paths, &instance).unwrap();
        let loaded = read_instance_file(&paths).unwrap().unwrap();

        assert_eq!(loaded.pid, 1);
        assert_eq!(loaded.http_url, "http://127.0.0.1:7878");
    }
}
