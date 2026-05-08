use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use unio_core::WorkspacePaths;
use unio_protocol::PermissionMode;
use unio_security::{decide, RiskLevel, SecurityDecision, ToolCapability, ToolPrecheck};
use unio_skills::{execute_skill_tool, SkillAgentStatus};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub capability: ToolCapability,
    pub default_risk: RiskLevel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionStatus {
    Completed,
    ApprovalRequired,
    Denied,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExecutionOutcome {
    pub status: ToolExecutionStatus,
    pub result: Option<ToolResult>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    pub workspace_root: PathBuf,
    pub user_home: PathBuf,
    pub permission_mode: PermissionMode,
}

#[derive(Debug, Clone, Default)]
pub struct ToolRegistry {
    definitions: Vec<ToolDefinition>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_builtin_tools() -> Self {
        let mut registry = Self::new();
        registry.register(
            "glob",
            "Find files by glob pattern",
            ToolCapability::ReadWorkspace,
        );
        registry.register(
            "grep",
            "Search text by regular expression",
            ToolCapability::ReadWorkspace,
        );
        registry.register(
            "read",
            "Read workspace files",
            ToolCapability::ReadWorkspace,
        );
        registry.register(
            "edit",
            "Apply precise patches",
            ToolCapability::WriteWorkspace,
        );
        registry.register(
            "write",
            "Create or overwrite files",
            ToolCapability::WriteWorkspace,
        );
        registry.register(
            "bash",
            "Run sandboxed commands",
            ToolCapability::ExecuteProcess,
        );
        registry.register(
            "fetch",
            "Fetch trusted public URLs",
            ToolCapability::NetworkAccess,
        );
        registry.register(
            "plan",
            "Produce a read-only execution plan",
            ToolCapability::PlanOnly,
        );
        registry.register(
            "skill-tool",
            "Invoke a skill tool agent",
            ToolCapability::SkillTool,
        );
        registry
    }

    pub fn register(
        &mut self,
        name: impl Into<String>,
        description: impl Into<String>,
        capability: ToolCapability,
    ) {
        self.definitions.push(ToolDefinition {
            name: name.into(),
            description: description.into(),
            capability,
            default_risk: RiskLevel::Low,
        });
    }

    pub fn definitions(&self) -> &[ToolDefinition] {
        &self.definitions
    }

    pub fn precheck_for(&self, name: &str) -> Option<ToolPrecheck> {
        self.definitions
            .iter()
            .find(|item| item.name == name)
            .map(|item| ToolPrecheck {
                capability: item.capability,
                risk: item.default_risk,
                inside_workspace: true,
                trusted_network: false,
            })
    }

    pub async fn execute(
        &self,
        call: ToolCall,
        context: ToolExecutionContext,
    ) -> ToolExecutionOutcome {
        let Some(definition) = self.definitions.iter().find(|item| item.name == call.name) else {
            return failed(call, "unknown tool");
        };
        let precheck = precheck_for_call(definition, &call, &context);
        match decide(context.permission_mode, &precheck) {
            SecurityDecision::Allow => match execute_allowed(call.clone(), context).await {
                Ok(content) => ToolExecutionOutcome {
                    status: ToolExecutionStatus::Completed,
                    result: Some(ToolResult {
                        call_id: call.call_id,
                        name: call.name,
                        content,
                    }),
                    reason: None,
                },
                Err(error) => failed(call, error.to_string()),
            },
            SecurityDecision::RequireApproval { reason } => ToolExecutionOutcome {
                status: ToolExecutionStatus::ApprovalRequired,
                result: None,
                reason: Some(reason),
            },
            SecurityDecision::Deny { reason } => ToolExecutionOutcome {
                status: ToolExecutionStatus::Denied,
                result: None,
                reason: Some(reason),
            },
        }
    }
}

fn precheck_for_call(
    definition: &ToolDefinition,
    call: &ToolCall,
    context: &ToolExecutionContext,
) -> ToolPrecheck {
    let path = call
        .arguments
        .get("path")
        .and_then(Value::as_str)
        .map(|value| resolve_workspace_path(&context.workspace_root, value));
    let inside_workspace = path
        .as_ref()
        .map(|path| path.starts_with(&context.workspace_root))
        .unwrap_or(true);
    let trusted_network = call
        .arguments
        .get("url")
        .and_then(Value::as_str)
        .map(is_trusted_url)
        .unwrap_or(false);
    ToolPrecheck {
        capability: definition.capability,
        risk: definition.default_risk,
        inside_workspace,
        trusted_network,
    }
}

async fn execute_allowed(call: ToolCall, context: ToolExecutionContext) -> anyhow::Result<String> {
    match call.name.as_str() {
        "glob" => run_glob(&context.workspace_root, &call.arguments),
        "grep" => run_grep(&context.workspace_root, &call.arguments),
        "read" => run_read(&context.workspace_root, &call.arguments),
        "write" => run_write(&context.workspace_root, &call.arguments),
        "edit" => run_edit(&context.workspace_root, &call.arguments),
        "bash" => run_bash(&context.workspace_root, &call.arguments),
        "fetch" => run_fetch(&call.arguments).await,
        "plan" => run_plan(&call.arguments),
        "skill-tool" => run_skill_tool(&context, &call.arguments),
        _ => anyhow::bail!("unknown tool"),
    }
}

fn run_glob(workspace_root: &Path, args: &Value) -> anyhow::Result<String> {
    let pattern = required_str(args, "pattern")?;
    let mut matches = Vec::new();
    collect_files(workspace_root, &mut matches)?;
    let matched = matches
        .into_iter()
        .filter(|path| wildcard_match(pattern, &path.to_string_lossy()))
        .map(|path| path.to_string_lossy().to_string())
        .collect::<Vec<_>>();
    Ok(matched.join("\n"))
}

fn run_grep(workspace_root: &Path, args: &Value) -> anyhow::Result<String> {
    let query = required_str(args, "query")?;
    let mut files = Vec::new();
    collect_files(workspace_root, &mut files)?;
    let mut lines = Vec::new();
    for file in files {
        let Ok(content) = fs::read_to_string(&file) else {
            continue;
        };
        for (index, line) in content.lines().enumerate() {
            if line.contains(query) {
                lines.push(format!("{}:{}:{}", file.display(), index + 1, line));
            }
        }
    }
    Ok(lines.join("\n"))
}

fn run_read(workspace_root: &Path, args: &Value) -> anyhow::Result<String> {
    let path = resolve_workspace_path(workspace_root, required_str(args, "path")?);
    ensure_workspace_path(workspace_root, &path)?;
    Ok(fs::read_to_string(path)?)
}

fn run_write(workspace_root: &Path, args: &Value) -> anyhow::Result<String> {
    let path = resolve_workspace_path(workspace_root, required_str(args, "path")?);
    ensure_workspace_path(workspace_root, &path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, required_str(args, "content")?)?;
    Ok(format!("wrote {}", path.display()))
}

fn run_edit(workspace_root: &Path, args: &Value) -> anyhow::Result<String> {
    let path = resolve_workspace_path(workspace_root, required_str(args, "path")?);
    ensure_workspace_path(workspace_root, &path)?;
    let old = required_str(args, "old")?;
    let new = required_str(args, "new")?;
    let content = fs::read_to_string(&path)?;
    if !content.contains(old) {
        anyhow::bail!("old text not found");
    }
    fs::write(&path, content.replacen(old, new, 1))?;
    Ok(format!("edited {}", path.display()))
}

fn run_bash(workspace_root: &Path, args: &Value) -> anyhow::Result<String> {
    let command = required_str(args, "command")?;
    if has_shell_syntax(command) {
        anyhow::bail!("shell syntax is not allowed in bash tool");
    }
    let mut parts = command.split_whitespace();
    let program = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("empty command"))?;
    let output = Command::new(program)
        .args(parts)
        .current_dir(workspace_root)
        .output()?;
    let mut text = String::new();
    text.push_str(&String::from_utf8_lossy(&output.stdout));
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(text)
}

async fn run_fetch(args: &Value) -> anyhow::Result<String> {
    let url = required_str(args, "url")?;
    if !is_trusted_url(url) {
        anyhow::bail!("untrusted url");
    }
    Ok(reqwest::get(url).await?.error_for_status()?.text().await?)
}

fn run_plan(args: &Value) -> anyhow::Result<String> {
    let goal = required_str(args, "goal")?;
    Ok(serde_json::json!({
        "goal": goal,
        "steps": [
            {
                "id": "step-1",
                "title": "Inspect current state",
                "depends_on": [],
                "executor": "root_agent",
                "parallelizable": false,
                "status": "pending"
            }
        ]
    })
    .to_string())
}

fn run_skill_tool(context: &ToolExecutionContext, args: &Value) -> anyhow::Result<String> {
    let name = required_str(args, "name")?;
    let request = args.get("request").and_then(Value::as_str).unwrap_or("");
    let paths = WorkspacePaths::new(&context.user_home, &context.workspace_root);
    let result = execute_skill_tool(&paths, name, request)?;
    if result.status == SkillAgentStatus::Failed {
        anyhow::bail!("skill not found: {name}");
    }
    Ok(serde_json::to_string(&result)?)
}

fn collect_files(root: &Path, files: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.file_name().and_then(|value| value.to_str()) == Some("target") {
            continue;
        }
        if path.is_dir() {
            collect_files(&path, files)?;
        } else {
            files.push(path);
        }
    }
    Ok(())
}

fn wildcard_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    let parts = pattern.split('*').collect::<Vec<_>>();
    if parts.len() == 1 {
        return value.ends_with(pattern) || value.contains(pattern);
    }
    let mut rest = value;
    for part in parts.into_iter().filter(|part| !part.is_empty()) {
        let Some(index) = rest.find(part) else {
            return false;
        };
        rest = &rest[index + part.len()..];
    }
    true
}

fn required_str<'a>(args: &'a Value, key: &str) -> anyhow::Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("missing string argument `{key}`"))
}

fn resolve_workspace_path(workspace_root: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        workspace_root.join(path)
    }
}

fn ensure_workspace_path(workspace_root: &Path, path: &Path) -> anyhow::Result<()> {
    if !path.starts_with(workspace_root) {
        anyhow::bail!("path escapes workspace: {}", path.display());
    }
    Ok(())
}

fn has_shell_syntax(command: &str) -> bool {
    ["&&", "||", "|", ";", ">", "<", "$(", "`"]
        .iter()
        .any(|item| command.contains(item))
}

fn is_trusted_url(url: &str) -> bool {
    url.starts_with("https://raw.githubusercontent.com/")
        || url.starts_with("https://api.github.com/")
        || url.starts_with("https://docs.rs/")
}

fn failed(_call: ToolCall, reason: impl ToString) -> ToolExecutionOutcome {
    ToolExecutionOutcome {
        status: ToolExecutionStatus::Failed,
        result: None,
        reason: Some(reason.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use tempfile::tempdir;
    use unio_protocol::PermissionMode;

    use super::{ToolCall, ToolExecutionContext, ToolExecutionStatus, ToolRegistry};

    #[test]
    fn builtin_registry_contains_planned_tools() {
        let registry = ToolRegistry::with_builtin_tools();
        let names = registry
            .definitions()
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "glob",
                "grep",
                "read",
                "edit",
                "write",
                "bash",
                "fetch",
                "plan",
                "skill-tool"
            ]
        );
    }

    #[tokio::test]
    async fn read_tool_executes_after_security_allow() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("hello.txt"), "hello").unwrap();
        let registry = ToolRegistry::with_builtin_tools();
        let outcome = registry
            .execute(
                ToolCall {
                    call_id: "call_1".into(),
                    name: "read".into(),
                    arguments: json!({ "path": "hello.txt" }),
                },
                ToolExecutionContext {
                    workspace_root: dir.path().to_path_buf(),
                    user_home: dir.path().to_path_buf(),
                    permission_mode: PermissionMode::Default,
                },
            )
            .await;

        assert_eq!(outcome.status, ToolExecutionStatus::Completed);
        assert_eq!(outcome.result.unwrap().content, "hello");
    }

    #[tokio::test]
    async fn write_tool_requires_approval_in_default_mode() {
        let dir = tempdir().unwrap();
        let registry = ToolRegistry::with_builtin_tools();
        let outcome = registry
            .execute(
                ToolCall {
                    call_id: "call_1".into(),
                    name: "write".into(),
                    arguments: json!({ "path": "hello.txt", "content": "hello" }),
                },
                ToolExecutionContext {
                    workspace_root: dir.path().to_path_buf(),
                    user_home: dir.path().to_path_buf(),
                    permission_mode: PermissionMode::Default,
                },
            )
            .await;

        assert_eq!(outcome.status, ToolExecutionStatus::ApprovalRequired);
    }

    #[tokio::test]
    async fn skill_tool_returns_structured_skill_agent_result() {
        let dir = tempdir().unwrap();
        let skill_dir = dir.path().join(".unio").join("skills").join("repo");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "# Repo skill\nPrivate instruction body",
        )
        .unwrap();
        let registry = ToolRegistry::with_builtin_tools();
        let outcome = registry
            .execute(
                ToolCall {
                    call_id: "call_1".into(),
                    name: "skill-tool".into(),
                    arguments: json!({ "name": "repo", "request": "inspect modules" }),
                },
                ToolExecutionContext {
                    workspace_root: dir.path().to_path_buf(),
                    user_home: dir.path().to_path_buf(),
                    permission_mode: PermissionMode::FullTrust,
                },
            )
            .await;

        assert_eq!(outcome.status, ToolExecutionStatus::Completed);
        let content = outcome.result.unwrap().content;
        assert!(content.contains("\"skill_name\":\"repo\""));
        assert!(!content.contains("Private instruction body"));
    }
}
