use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use unio_core::{ApprovalId, RunId, SessionId, TraceId};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConversationId(String);

impl ConversationId {
    pub fn new() -> Self {
        Self(format!("conversation_{}", uuid::Uuid::new_v4()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    Default,
    Auto,
    FullTrust,
}

impl Default for PermissionMode {
    fn default() -> Self {
        Self::Default
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStage {
    Queued,
    Planning,
    WaitingApproval,
    RunningTools,
    Streaming,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitTurnRequest {
    pub prompt: String,
    #[serde(default)]
    pub permission_mode: PermissionMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnStarted {
    pub session_id: SessionId,
    pub conversation_id: ConversationId,
    pub run_id: RunId,
    pub stage: RunStage,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TurnCompleted {
    pub session_id: SessionId,
    pub run_id: RunId,
    pub trace_id: TraceId,
    pub stage: RunStage,
    pub final_text: String,
    pub events: Vec<String>,
    pub provider: String,
    pub model: String,
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub context_ratio: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: SessionId,
    pub title: String,
    pub workspace_root: String,
    pub permission_mode: PermissionMode,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_run_id: Option<RunId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveSessionRequest {
    pub workspace_root: String,
    #[serde(default)]
    pub permission_mode: PermissionMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveSessionResponse {
    pub session: SessionSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecTurnRequest {
    pub session_id: SessionId,
    pub prompt: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecTurnResponse {
    pub started: TurnStarted,
    pub completed: TurnCompleted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadTranscriptRequest {
    pub session_id: SessionId,
    #[serde(default)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LoadTranscriptResponse {
    pub session: SessionSummary,
    pub messages: Vec<TranscriptMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelsStatus {
    pub provider: String,
    pub model: String,
    pub fallback_to_mock: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub pid: u32,
    pub http_url: String,
    pub started_at: DateTime<Utc>,
    pub session_count: usize,
    pub pending_approval_count: usize,
    pub latest_session_id: Option<SessionId>,
    pub latest_trace_id: Option<TraceId>,
    pub latest_context_ratio: Option<f32>,
    pub models: ModelsStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceLookupRequest {
    pub trace_id: TraceId,
    #[serde(default)]
    pub run_id: Option<RunId>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceLookupResponse {
    pub trace_id: TraceId,
    pub events: Vec<TraceEventRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceEventRecord {
    pub run_id: RunId,
    pub kind: String,
    pub message: String,
    pub token_usage: Option<TraceTokenUsage>,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceTokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub context_ratio: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolExecuteRequest {
    pub name: String,
    pub arguments: serde_json::Value,
    #[serde(default)]
    pub permission_mode: PermissionMode,
    pub workspace_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolExecuteResponse {
    pub status: String,
    pub content: Option<String>,
    pub reason: Option<String>,
    pub approval_id: Option<ApprovalId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingApprovalSummary {
    pub approval_id: ApprovalId,
    pub tool_call_id: String,
    pub tool_name: String,
    pub reason: String,
    pub workspace_root: String,
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalListResponse {
    pub pending: Vec<PendingApprovalSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalResolveRequest {
    pub approval_id: ApprovalId,
    pub approved: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalResolveResponse {
    pub approval_id: ApprovalId,
    pub status: String,
    pub content: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalGrantSummary {
    pub approval_id: ApprovalId,
    pub tool_call_id: String,
    pub tool_name: String,
    pub workspace_root: String,
    pub approved: bool,
    pub reason: Option<String>,
    pub resolved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalHistoryResponse {
    pub grants: Vec<ApprovalGrantSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum TranscriptMessage {
    User {
        session_id: SessionId,
        run_id: RunId,
        content: String,
        recorded_at: DateTime<Utc>,
    },
    Assistant {
        session_id: SessionId,
        run_id: RunId,
        content: String,
        reasoning_content: Option<String>,
        recorded_at: DateTime<Utc>,
    },
    Tool {
        session_id: SessionId,
        run_id: RunId,
        tool_call_id: String,
        tool_name: String,
        content: String,
        recorded_at: DateTime<Utc>,
    },
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use unio_core::{RunId, SessionId};

    use super::TranscriptMessage;

    #[test]
    fn transcript_message_schema_is_message_level() {
        let record = TranscriptMessage::User {
            session_id: SessionId::from_string("session_1"),
            run_id: RunId::from_string("run_1"),
            content: "hello".into(),
            recorded_at: Utc::now(),
        };

        let json = serde_json::to_value(record).unwrap();

        assert_eq!(json["role"], "user");
        assert_eq!(json["content"], "hello");
        assert!(json.get("final_text").is_none());
    }
}
