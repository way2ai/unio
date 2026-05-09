use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use unio_core::{AgentId, RunId, SessionId, TraceId};
use unio_model::{ModelMessage, ModelProvider, ModelRequest, ProviderSummary, ResolvedProvider};
use unio_protocol::{ConversationId, PermissionMode, TranscriptMessage};
use unio_tools::{ToolCall, ToolRegistry};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentRuntime {
    pub session_id: SessionId,
    pub conversation_id: ConversationId,
    pub run_id: RunId,
    pub trace_id: TraceId,
    pub agent_id: AgentId,
    pub input: String,
    #[serde(default)]
    pub history: Vec<TranscriptMessage>,
    pub permission_mode: PermissionMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentOutcome {
    pub final_text: String,
    pub plan: Option<PlanSpec>,
    pub sub_agent_results: Vec<SubAgentResult>,
    pub tool_calls: Vec<ToolCall>,
    pub events: Vec<String>,
    pub provider: ProviderSummary,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub context_ratio: f32,
}

pub type AgentEvent = String;

#[async_trait]
pub trait RootAgent: Send + Sync {
    async fn run(&self, runtime: AgentRuntime) -> anyhow::Result<AgentOutcome>;
}

#[async_trait]
pub trait SubAgent: Send + Sync {
    async fn run(&self, task: SubAgentTask) -> anyhow::Result<SubAgentResult>;
}

#[async_trait]
pub trait SkillAgent: Send + Sync {
    async fn run(&self, task: SkillAgentTask) -> anyhow::Result<SkillAgentResult>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubAgentTask {
    pub agent_id: AgentId,
    pub goal: String,
    pub input_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubAgentResult {
    pub agent_id: AgentId,
    pub goal: String,
    pub summary: String,
    pub status: SubAgentStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubAgentStatus {
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillAgentTask {
    pub agent_id: AgentId,
    pub skill_name: String,
    pub request: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillAgentResult {
    pub agent_id: AgentId,
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

#[derive(Debug, Clone, Default)]
pub struct MockSubAgent;

#[async_trait]
impl SubAgent for MockSubAgent {
    async fn run(&self, task: SubAgentTask) -> anyhow::Result<SubAgentResult> {
        Ok(SubAgentResult {
            agent_id: task.agent_id,
            goal: task.goal,
            summary: format!("Mock sub-agent completed: {}", task.input_summary),
            status: SubAgentStatus::Completed,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct MockSkillAgent;

#[async_trait]
impl SkillAgent for MockSkillAgent {
    async fn run(&self, task: SkillAgentTask) -> anyhow::Result<SkillAgentResult> {
        Ok(SkillAgentResult {
            agent_id: task.agent_id,
            skill_name: task.skill_name.clone(),
            summary: format!(
                "Mock skill-agent `{}` completed request: {}",
                task.skill_name, task.request
            ),
            artifacts: Vec::new(),
            status: SkillAgentStatus::Completed,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanSpec {
    pub goal: String,
    pub steps: Vec<PlanStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanStep {
    pub id: String,
    pub title: String,
    pub depends_on: Vec<String>,
    pub executor: StepExecutor,
    pub parallelizable: bool,
    pub status: PlanStepStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepExecutor {
    RootAgent,
    SubAgent,
    SkillAgent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanStepStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Clone)]
pub struct RootAgentRuntime<P = ResolvedProvider> {
    provider: P,
    tools: ToolRegistry,
}

impl RootAgentRuntime {
    pub fn new() -> Self {
        Self {
            provider: ResolvedProvider::from_env(),
            tools: ToolRegistry::with_builtin_tools(),
        }
    }
}

impl Default for RootAgentRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl RootAgentRuntime<ResolvedProvider> {
    pub async fn run_with_summary(&self, runtime: AgentRuntime) -> anyhow::Result<AgentOutcome> {
        if should_plan(&runtime.input) {
            let plan = create_plan_spec(&runtime.input);
            return Ok(AgentOutcome {
                final_text: format_plan(&plan),
                plan: Some(plan),
                sub_agent_results: Vec::new(),
                tool_calls: Vec::new(),
                events: vec!["planner.completed".into()],
                provider: self.provider.summary().clone(),
                input_tokens: runtime.input.split_whitespace().count(),
                output_tokens: 0,
                context_ratio: context_ratio(runtime.input.split_whitespace().count(), 0),
            });
        }
        let messages = model_messages_from_runtime(&runtime);
        let tools = model_tool_definitions_for_runtime(&self.tools);
        let response = self
            .provider
            .complete(ModelRequest {
                model: Some(self.provider.summary().model.clone()),
                messages,
                tools,
            })
            .await?;

        Ok(AgentOutcome {
            final_text: response.content,
            plan: None,
            sub_agent_results: Vec::new(),
            tool_calls: response.tool_calls.clone(),
            events: if response.tool_calls.is_empty() {
                vec!["root_agent.completed".into()]
            } else {
                vec!["root_agent.requested_tools".into()]
            },
            provider: self.provider.summary().clone(),
            input_tokens: response.input_tokens,
            output_tokens: response.output_tokens,
            context_ratio: context_ratio(response.input_tokens, response.output_tokens),
        })
    }
}

fn should_plan(input: &str) -> bool {
    let trimmed = input.trim();
    let input = trimmed.to_ascii_lowercase();
    input.starts_with("plan ")
        || input.starts_with("mock-plan ")
        || input.contains("complex task")
        || input.contains("systematic")
        || trimmed.contains("复杂任务")
}

fn create_plan_spec(input: &str) -> PlanSpec {
    let goal = input
        .trim()
        .strip_prefix("plan ")
        .or_else(|| input.trim().strip_prefix("mock-plan "))
        .unwrap_or_else(|| input.trim())
        .trim()
        .to_string();
    PlanSpec {
        goal,
        steps: vec![
            PlanStep {
                id: "step-1".into(),
                title: "Inspect current state and constraints".into(),
                depends_on: Vec::new(),
                executor: StepExecutor::RootAgent,
                parallelizable: false,
                status: PlanStepStatus::Pending,
            },
            PlanStep {
                id: "step-2".into(),
                title: "Break work into bounded implementation slices".into(),
                depends_on: vec!["step-1".into()],
                executor: StepExecutor::RootAgent,
                parallelizable: false,
                status: PlanStepStatus::Pending,
            },
            PlanStep {
                id: "step-3".into(),
                title: "Execute safe slices with verification after each step".into(),
                depends_on: vec!["step-2".into()],
                executor: StepExecutor::SubAgent,
                parallelizable: true,
                status: PlanStepStatus::Pending,
            },
        ],
    }
}

fn format_plan(plan: &PlanSpec) -> String {
    let mut text = format!("Plan: {}\n", plan.goal);
    for step in &plan.steps {
        text.push_str(&format!(
            "- {}: {} [{:?}]\n",
            step.id, step.title, step.status
        ));
    }
    text.trim_end().to_string()
}

pub fn sub_agent_task_from_plan(plan: &PlanSpec) -> Option<SubAgentTask> {
    let step = plan
        .steps
        .iter()
        .find(|step| step.executor == StepExecutor::SubAgent)?;
    Some(SubAgentTask {
        agent_id: AgentId::new(unio_core::AgentKind::SubAgent),
        goal: step.title.clone(),
        input_summary: format!("Goal: {}; Step: {}", plan.goal, step.id),
    })
}

fn model_messages_from_runtime(runtime: &AgentRuntime) -> Vec<ModelMessage> {
    let mut messages = Vec::new();
    messages.push(ModelMessage {
        role: "system".into(),
        content: system_prompt_with_environment(),
    });
    for message in &runtime.history {
        match message {
            TranscriptMessage::User { content, .. } => messages.push(ModelMessage {
                role: "user".into(),
                content: content.clone(),
            }),
            TranscriptMessage::Assistant { content, .. } => messages.push(ModelMessage {
                role: "assistant".into(),
                content: content.clone(),
            }),
            TranscriptMessage::Tool {
                tool_name, content, ..
            } => messages.push(ModelMessage {
                role: "user".into(),
                content: format!("Tool `{tool_name}` result:\n{content}"),
            }),
        }
    }
    messages.push(ModelMessage {
        role: "user".into(),
        content: runtime.input.clone(),
    });
    messages
}

fn model_tool_definitions_for_runtime(registry: &ToolRegistry) -> Vec<unio_tools::ToolDefinition> {
    let mut tools = registry.definitions().to_vec();
    if let Some(bash) = tools.iter_mut().find(|tool| tool.name == "bash") {
        bash.description = format!("{}. {}", bash.description, bash_tool_environment_hint());
    }
    tools
}

fn system_prompt_with_environment() -> String {
    format!(
        "You are Unio, a local coding agent. Use tools when needed and keep answers concise. {}",
        bash_tool_environment_hint()
    )
}

fn bash_tool_environment_hint() -> &'static str {
    #[cfg(windows)]
    {
        "Environment: Windows. For `bash` tool, generate Windows-compatible commands (cmd/PowerShell style), avoid Unix-only syntax, and when a tool failure is returned, adapt the command in the next step (ReAct retry)."
    }
    #[cfg(not(windows))]
    {
        "Environment: Unix-like shell. For `bash` tool, generate POSIX-compatible commands, and when a tool failure is returned, adapt the command in the next step (ReAct retry)."
    }
}

#[async_trait]
impl RootAgent for RootAgentRuntime<ResolvedProvider> {
    async fn run(&self, runtime: AgentRuntime) -> anyhow::Result<AgentOutcome> {
        self.run_with_summary(runtime).await
    }
}

fn context_ratio(input_tokens: usize, output_tokens: usize) -> f32 {
    let total = input_tokens + output_tokens;
    if total == 0 {
        0.0
    } else {
        (total as f32 / 128_000.0).min(1.0)
    }
}

#[cfg(test)]
mod tests {
    use unio_core::{RunId, SessionId};
    use unio_protocol::TranscriptMessage;
    use unio_tools::ToolRegistry;

    use super::{
        create_plan_spec, model_messages_from_runtime, model_tool_definitions_for_runtime,
        sub_agent_task_from_plan, AgentRuntime, MockSkillAgent, MockSubAgent, PlanSpec, PlanStep,
        PlanStepStatus, SkillAgent, SkillAgentStatus, SkillAgentTask, StepExecutor, SubAgent,
        SubAgentStatus,
    };

    #[test]
    fn plan_spec_marks_dependencies_and_executor() {
        let plan = PlanSpec {
            goal: "refactor".into(),
            steps: vec![PlanStep {
                id: "step-1".into(),
                title: "inspect".into(),
                depends_on: Vec::new(),
                executor: StepExecutor::SubAgent,
                parallelizable: true,
                status: PlanStepStatus::Pending,
            }],
        };

        assert_eq!(plan.steps[0].executor, StepExecutor::SubAgent);
        assert!(plan.steps[0].parallelizable);
        assert_eq!(plan.steps[0].status, PlanStepStatus::Pending);
    }

    #[test]
    fn runtime_history_becomes_model_messages() {
        let runtime = AgentRuntime {
            session_id: SessionId::from_string("session_1"),
            conversation_id: unio_protocol::ConversationId::new(),
            run_id: RunId::from_string("run_1"),
            trace_id: unio_core::TraceId::from_string("trace_1"),
            agent_id: unio_core::AgentId::new_root(),
            input: "next".into(),
            history: vec![TranscriptMessage::User {
                session_id: SessionId::from_string("session_1"),
                run_id: RunId::from_string("run_0"),
                content: "previous".into(),
                recorded_at: unio_core::now_utc(),
            }],
            permission_mode: unio_protocol::PermissionMode::Default,
        };

        let messages = model_messages_from_runtime(&runtime);

        assert!(messages.iter().any(|message| message.content == "previous"));
        assert_eq!(messages.last().unwrap().content, "next");
        assert!(messages[0].content.contains("Environment:"));
    }

    #[test]
    fn bash_tool_description_is_environment_aware() {
        let tools = model_tool_definitions_for_runtime(&ToolRegistry::with_builtin_tools());
        let bash = tools.into_iter().find(|tool| tool.name == "bash").unwrap();

        assert!(bash.description.contains("Environment:"));
    }

    #[test]
    fn planner_creates_displayable_steps() {
        let plan = create_plan_spec("plan refactor this repo");

        assert_eq!(plan.goal, "refactor this repo");
        assert_eq!(plan.steps.len(), 3);
        assert!(plan.steps[2].parallelizable);
    }

    #[tokio::test]
    async fn mock_sub_agent_returns_structured_result() {
        let plan = create_plan_spec("plan refactor this repo");
        let task = sub_agent_task_from_plan(&plan).unwrap();
        let result = MockSubAgent.run(task).await.unwrap();

        assert_eq!(result.status, SubAgentStatus::Completed);
        assert!(result.summary.contains("Mock sub-agent completed"));
    }

    #[tokio::test]
    async fn mock_skill_agent_returns_structured_result() {
        let result = MockSkillAgent
            .run(SkillAgentTask {
                agent_id: unio_core::AgentId::new(unio_core::AgentKind::SkillAgent),
                skill_name: "repo".into(),
                request: "inspect modules".into(),
            })
            .await
            .unwrap();

        assert_eq!(result.status, SkillAgentStatus::Completed);
        assert_eq!(result.skill_name, "repo");
        assert!(result.summary.contains("inspect modules"));
    }
}
