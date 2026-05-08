use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use unio_core::WorkspacePaths;

#[derive(Debug, Error)]
pub enum SkillDiscoveryError {
    #[error("failed to read skill directory `{path}`: {message}")]
    ReadDir { path: String, message: String },
    #[error("failed to read skill file `{path}`: {message}")]
    ReadFile { path: String, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub source: SkillSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillSource {
    Workspace,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillToolDefinition {
    pub name: String,
    pub description: String,
    pub skill_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillAgentTask {
    pub skill_name: String,
    pub request: String,
    pub skill_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillAgentResult {
    pub skill_name: String,
    pub summary: String,
    pub artifacts: Vec<String>,
    pub status: SkillAgentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillAgentStatus {
    Completed,
    Failed,
}

pub fn discover_skills(paths: &WorkspacePaths) -> Result<Vec<SkillManifest>, SkillDiscoveryError> {
    let mut skills = Vec::new();
    load_skill_root(&paths.user_skills_dir(), SkillSource::User, &mut skills)?;
    load_skill_root(
        &paths.workspace_skills_dir(),
        SkillSource::Workspace,
        &mut skills,
    )?;
    skills.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(skills)
}

pub fn inject_skill_tools(skills: &[SkillManifest]) -> Vec<SkillToolDefinition> {
    skills
        .iter()
        .map(|skill| SkillToolDefinition {
            name: skill.name.clone(),
            description: skill.description.clone(),
            skill_path: skill.path.clone(),
        })
        .collect()
}

pub fn execute_skill_agent(task: SkillAgentTask) -> Result<SkillAgentResult, SkillDiscoveryError> {
    let content =
        fs::read_to_string(&task.skill_path).map_err(|err| SkillDiscoveryError::ReadFile {
            path: task.skill_path.display().to_string(),
            message: err.to_string(),
        })?;
    let description = first_non_empty_line(&content).unwrap_or_else(|| task.skill_name.clone());
    let request = if task.request.trim().is_empty() {
        "no request provided".to_string()
    } else {
        task.request.trim().to_string()
    };

    Ok(SkillAgentResult {
        skill_name: task.skill_name,
        summary: format!("Skill `{description}` handled request: {request}"),
        artifacts: Vec::new(),
        status: SkillAgentStatus::Completed,
    })
}

pub fn execute_skill_tool(
    paths: &WorkspacePaths,
    name: &str,
    request: &str,
) -> Result<SkillAgentResult, SkillDiscoveryError> {
    let skills = discover_skills(paths)?;
    let Some(skill) = skills.into_iter().find(|skill| skill.name == name) else {
        return Ok(SkillAgentResult {
            skill_name: name.to_string(),
            summary: format!("skill not found: {name}"),
            artifacts: Vec::new(),
            status: SkillAgentStatus::Failed,
        });
    };
    execute_skill_agent(SkillAgentTask {
        skill_name: skill.name,
        request: request.to_string(),
        skill_path: skill.path,
    })
}

fn load_skill_root(
    root: &Path,
    source: SkillSource,
    skills: &mut Vec<SkillManifest>,
) -> Result<(), SkillDiscoveryError> {
    if !root.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(root).map_err(|err| SkillDiscoveryError::ReadDir {
        path: root.display().to_string(),
        message: err.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|err| SkillDiscoveryError::ReadDir {
            path: root.display().to_string(),
            message: err.to_string(),
        })?;
        let path = entry.path().join("SKILL.md");
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(&path).map_err(|err| SkillDiscoveryError::ReadFile {
            path: path.display().to_string(),
            message: err.to_string(),
        })?;
        let name = entry.file_name().to_string_lossy().to_string();
        let description = first_non_empty_line(&content).unwrap_or_else(|| name.clone());
        skills.push(SkillManifest {
            name,
            description,
            path,
            source,
        });
    }
    Ok(())
}

fn first_non_empty_line(content: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("---"))
        .map(|line| line.trim_start_matches('#').trim().to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;
    use unio_core::WorkspacePaths;

    use super::{
        discover_skills, execute_skill_tool, inject_skill_tools, SkillAgentStatus, SkillSource,
    };

    #[test]
    fn discovers_unio_skill_roots_and_injects_tools() {
        let user = tempdir().unwrap();
        let workspace = tempdir().unwrap();
        let paths = WorkspacePaths::new(user.path(), workspace.path());
        let skill_dir = paths.workspace_skills_dir().join("repo");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "# Repo skill\nUse for repo work",
        )
        .unwrap();

        let skills = discover_skills(&paths).unwrap();
        let tools = inject_skill_tools(&skills);

        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].source, SkillSource::Workspace);
        assert_eq!(tools[0].name, "repo");
        assert!(tools[0].description.contains("Repo skill"));
    }

    #[test]
    fn skill_tool_returns_structured_result_without_full_skill_body() {
        let user = tempdir().unwrap();
        let workspace = tempdir().unwrap();
        let paths = WorkspacePaths::new(user.path(), workspace.path());
        let skill_dir = paths.workspace_skills_dir().join("repo");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "# Repo skill\nPrivate instruction body that should not be returned verbatim",
        )
        .unwrap();

        let result = execute_skill_tool(&paths, "repo", "inspect modules").unwrap();

        assert_eq!(result.status, SkillAgentStatus::Completed);
        assert!(result.summary.contains("inspect modules"));
        assert!(!result.summary.contains("Private instruction body"));
    }
}
