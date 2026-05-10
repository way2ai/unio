use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use unio_agent::{
    sub_agent_task_from_plan, AgentRuntime, MockSubAgent, RootAgent, RootAgentRuntime, SubAgent,
};
use unio_core::{
    now_utc, read_instance_file, write_instance_file, AgentId, ApprovalId, DaemonInstance, RunId,
    TraceId, UserPaths,
};
use unio_model::ResolvedProvider;
use unio_observability::{context_budget_events, JsonlTraceStore, TraceEvent};
use unio_protocol::{
    ApprovalHistoryResponse, ApprovalListResponse, ApprovalResolveRequest, ApprovalResolveResponse,
    ConversationId, DaemonStatus, ExecTurnRequest, ExecTurnResponse, LoadTranscriptRequest,
    LoadTranscriptResponse, ModelsStatus, PendingApprovalSummary, ResolveSessionRequest,
    ResolveSessionResponse, RunStage, SessionResolveStrategy, SessionSummary, ToolExecuteRequest,
    ToolExecuteResponse, TraceEventRecord, TraceLookupRequest, TraceLookupResponse,
    TraceTokenUsage, TranscriptMessage, TurnCompleted, TurnStarted,
};
use unio_storage::{ApprovalGrantRecord, JsonlTranscriptStore, RunRecord, SqliteSessionStore};
use unio_tools::{ToolCall, ToolExecutionContext, ToolExecutionStatus, ToolRegistry};

const MAX_CONTEXT_TEXT_CHARS: usize = 4_000;
const MAX_CONTEXT_TOOL_CHARS: usize = 1_000;

#[derive(Clone)]
pub struct DaemonState {
    pub paths: UserPaths,
    pub store: Arc<Mutex<SqliteSessionStore>>,
    pub trace_store: Arc<JsonlTraceStore>,
    pub pending_approvals: Arc<Mutex<Vec<PendingApproval>>>,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct PendingApproval {
    pub session_id: Option<unio_core::SessionId>,
    pub approval_id: ApprovalId,
    pub call: ToolCall,
    pub reason: String,
    pub workspace_root: std::path::PathBuf,
    pub user_home: std::path::PathBuf,
    pub run_id: RunId,
    pub trace_id: TraceId,
    pub requested_at: chrono::DateTime<chrono::Utc>,
}

pub async fn serve(bind_addr: SocketAddr) -> anyhow::Result<()> {
    let paths = UserPaths::current()?;
    paths.ensure()?;
    let store = SqliteSessionStore::open(&paths.state_db_file)?;
    let state = DaemonState {
        trace_store: Arc::new(JsonlTraceStore::new(&paths.traces_file)),
        paths,
        store: Arc::new(Mutex::new(store)),
        pending_approvals: Arc::new(Mutex::new(Vec::new())),
        started_at: now_utc(),
    };
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    let local_addr = listener.local_addr()?;
    write_instance_file(
        &state.paths,
        &DaemonInstance {
            pid: std::process::id(),
            http_url: format!("http://{}", local_addr),
            started_at: state.started_at,
        },
    )?;

    let app = Router::new()
        .route("/status", get(status))
        .route("/models", get(models))
        .route("/sessions", get(list_sessions))
        .route("/sessions/resolve", post(resolve_session))
        .route("/sessions/transcript", post(load_transcript))
        .route("/traces/query", post(query_trace))
        .route("/exec", post(exec_turn))
        .route("/tools/execute", post(execute_tool))
        .route("/approvals", get(list_approvals))
        .route("/approvals/history", get(list_approval_history))
        .route("/approvals/resolve", post(resolve_approval))
        .with_state(state);
    axum::serve(listener, app).await?;
    Ok(())
}

pub async fn daemon_status() -> anyhow::Result<Option<DaemonStatus>> {
    let paths = UserPaths::current()?;
    let Some(instance) = read_instance_file(&paths)? else {
        return Ok(None);
    };
    let response = reqwest::get(format!("{}/status", instance.http_url))
        .await?
        .error_for_status()?;
    Ok(Some(response.json().await?))
}

async fn status(State(state): State<DaemonState>) -> Json<DaemonStatus> {
    let (sessions, latest_context_ratio) = {
        let store = state.store.lock().expect("session store poisoned");
        (
            store.list_sessions().unwrap_or_default(),
            store.latest_context_ratio().unwrap_or_default(),
        )
    };
    let traces = state.trace_store.summarize().unwrap_or_else(|_| {
        unio_observability::TracePersistedSummary {
            latest_trace_id: None,
            event_count: 0,
        }
    });
    let provider = ResolvedProvider::from_env();
    let instance = read_instance_file(&state.paths)
        .ok()
        .flatten()
        .unwrap_or(DaemonInstance {
            pid: std::process::id(),
            http_url: "http://127.0.0.1:0".into(),
            started_at: state.started_at,
        });
    Json(DaemonStatus {
        pid: instance.pid,
        http_url: instance.http_url,
        started_at: instance.started_at,
        session_count: sessions.len(),
        pending_approval_count: state
            .pending_approvals
            .lock()
            .expect("approval queue poisoned")
            .len(),
        latest_session_id: sessions.first().map(|session| session.session_id.clone()),
        latest_trace_id: traces.latest_trace_id,
        latest_context_ratio,
        models: ModelsStatus {
            provider: provider.summary().provider.clone(),
            model: provider.summary().model.clone(),
            fallback_to_mock: provider.summary().fallback_to_mock,
        },
    })
}

async fn models() -> Json<ModelsStatus> {
    let provider = ResolvedProvider::from_env();
    Json(ModelsStatus {
        provider: provider.summary().provider.clone(),
        model: provider.summary().model.clone(),
        fallback_to_mock: provider.summary().fallback_to_mock,
    })
}

async fn list_sessions(State(state): State<DaemonState>) -> Json<Vec<SessionSummary>> {
    let sessions = state
        .store
        .lock()
        .expect("session store poisoned")
        .list_sessions()
        .unwrap_or_default();
    Json(sessions)
}

async fn resolve_session(
    State(state): State<DaemonState>,
    Json(request): Json<ResolveSessionRequest>,
) -> Result<Json<ResolveSessionResponse>, (StatusCode, String)> {
    let session = {
        let store = state.store.lock().expect("session store poisoned");
        match request.strategy {
            SessionResolveStrategy::ReuseWorkspaceLatest => {
                store.resolve_session(&request.workspace_root, request.permission_mode)
            }
            SessionResolveStrategy::CreateNew => {
                store.create_session(&request.workspace_root, request.permission_mode)
            }
        }
    }
    .map_err(internal_error)?;
    Ok(Json(ResolveSessionResponse { session }))
}

async fn load_transcript(
    State(state): State<DaemonState>,
    Json(request): Json<LoadTranscriptRequest>,
) -> Result<Json<LoadTranscriptResponse>, (StatusCode, String)> {
    let session = state
        .store
        .lock()
        .expect("session store poisoned")
        .find_session(&request.session_id)
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "session not found".into()))?;
    let transcript = JsonlTranscriptStore::new(
        state
            .paths
            .transcripts_dir
            .join(format!("{}.jsonl", request.session_id)),
    );
    let mut messages = transcript.read_all().map_err(internal_error)?;
    if let Some(limit) = request.limit {
        if messages.len() > limit {
            messages = messages.split_off(messages.len() - limit);
        }
    }
    Ok(Json(LoadTranscriptResponse { session, messages }))
}

async fn query_trace(
    State(state): State<DaemonState>,
    Json(request): Json<TraceLookupRequest>,
) -> Result<Json<TraceLookupResponse>, (StatusCode, String)> {
    let events = state
        .trace_store
        .events_by_trace_id(&request.trace_id)
        .map_err(internal_error)?
        .into_iter()
        .filter(|event| {
            request
                .run_id
                .as_ref()
                .map(|run_id| &event.run_id == run_id)
                .unwrap_or(true)
        })
        .map(|event| TraceEventRecord {
            run_id: event.run_id,
            kind: event.kind,
            message: event.message,
            token_usage: event.token_usage.map(|usage| TraceTokenUsage {
                input_tokens: usage.input_tokens,
                output_tokens: usage.output_tokens,
                context_ratio: usage.context_ratio,
            }),
            recorded_at: event.recorded_at,
        })
        .collect();
    Ok(Json(TraceLookupResponse {
        trace_id: request.trace_id,
        events,
    }))
}

async fn exec_turn(
    State(state): State<DaemonState>,
    Json(request): Json<ExecTurnRequest>,
) -> Result<Json<ExecTurnResponse>, (StatusCode, String)> {
    let session = state
        .store
        .lock()
        .expect("session store poisoned")
        .find_session(&request.session_id)
        .map_err(internal_error)?
        .ok_or_else(|| (StatusCode::NOT_FOUND, "session not found".into()))?;

    let conversation_id = ConversationId::new();
    let run_id = RunId::new();
    let trace_id = TraceId::new();
    let started = TurnStarted {
        session_id: session.session_id.clone(),
        conversation_id: conversation_id.clone(),
        run_id: run_id.clone(),
        stage: if should_enter_planning(&request.prompt) {
            RunStage::Planning
        } else if parse_tool_directive(&request.prompt).is_some() {
            RunStage::RunningTools
        } else {
            RunStage::Streaming
        },
    };

    let latest_context_ratio = state
        .store
        .lock()
        .expect("session store poisoned")
        .latest_context_ratio()
        .map_err(internal_error)?
        .unwrap_or(0.0);
    if context_compaction_required(latest_context_ratio, &request.prompt) {
        let final_text = format!(
            "Context compaction is required before continuing this task. latest_context_ratio={latest_context_ratio:.3}"
        );
        persist_exec_result(
            &state,
            &session,
            &run_id,
            &trace_id,
            &request.prompt,
            &final_text,
            "system",
            "context-policy",
            0,
            0,
            latest_context_ratio,
        )
        .map_err(internal_error)?;
        append_run_trace(
            &state,
            &trace_id,
            &run_id,
            "context.compaction_required",
            &final_text,
            0,
            0,
            latest_context_ratio,
        )
        .map_err(internal_error)?;
        return Ok(Json(ExecTurnResponse {
            started,
            completed: TurnCompleted {
                session_id: session.session_id,
                run_id,
                trace_id,
                stage: RunStage::Failed,
                final_text,
                events: vec!["context.compaction_required".into()],
                provider: "system".into(),
                model: "context-policy".into(),
                input_tokens: 0,
                output_tokens: 0,
                context_ratio: latest_context_ratio,
            },
        }));
    }

    if let Some(call) = parse_tool_directive(&request.prompt) {
        let workspace_root = std::path::PathBuf::from(&session.workspace_root);
        let user_home = user_home().map_err(internal_error)?;
        let outcome = execute_tool_call(
            &state,
            Some(session.session_id.clone()),
            call,
            ToolExecutionContext {
                workspace_root,
                user_home,
                permission_mode: session.permission_mode,
            },
            run_id.clone(),
            trace_id.clone(),
        )
        .await
        .map_err(internal_error)?;
        let final_text = tool_final_text(&outcome);
        persist_exec_result(
            &state,
            &session,
            &run_id,
            &trace_id,
            &request.prompt,
            &final_text,
            "tool",
            "builtin",
            0,
            0,
            0.0,
        )
        .map_err(internal_error)?;
        let completed = TurnCompleted {
            session_id: session.session_id,
            run_id,
            trace_id,
            stage: if outcome.approval_id.is_some() {
                RunStage::WaitingApproval
            } else {
                RunStage::Completed
            },
            final_text,
            events: vec![format!("tool.{}", outcome.status)],
            provider: "tool".into(),
            model: "builtin".into(),
            input_tokens: 0,
            output_tokens: 0,
            context_ratio: 0.0,
        };
        return Ok(Json(ExecTurnResponse { started, completed }));
    }

    let agent = RootAgentRuntime::new();
    let mut runtime_history =
        recent_transcript_messages(&state, &session.session_id, 24).map_err(internal_error)?;
    let mut outcome = agent
        .run(AgentRuntime {
            session_id: session.session_id.clone(),
            conversation_id,
            run_id: run_id.clone(),
            trace_id: trace_id.clone(),
            agent_id: AgentId::new_root(),
            input: request.prompt.clone(),
            history: runtime_history.clone(),
            permission_mode: session.permission_mode,
        })
        .await
        .map_err(internal_error)?;

    if outcome.plan.is_some() {
        let sub_agent_result = if let Some(plan) = &outcome.plan {
            if let Some(task) = sub_agent_task_from_plan(plan) {
                Some(MockSubAgent.run(task).await.map_err(internal_error)?)
            } else {
                None
            }
        } else {
            None
        };
        let final_text = if let Some(result) = &sub_agent_result {
            format!(
                "{}\n\nSub-agent result:\n- {}: {}",
                outcome.final_text, result.agent_id, result.summary
            )
        } else {
            outcome.final_text.clone()
        };
        persist_exec_result(
            &state,
            &session,
            &run_id,
            &trace_id,
            &request.prompt,
            &final_text,
            &outcome.provider.provider,
            &outcome.provider.model,
            outcome.input_tokens,
            outcome.output_tokens,
            outcome.context_ratio,
        )
        .map_err(internal_error)?;
        append_run_trace(
            &state,
            &trace_id,
            &run_id,
            "planning.completed",
            &final_text,
            outcome.input_tokens,
            outcome.output_tokens,
            outcome.context_ratio,
        )
        .map_err(internal_error)?;
        if let Some(result) = &sub_agent_result {
            append_tool_trace(
                &state,
                &trace_id,
                &run_id,
                "sub_agent.completed",
                format!("{}: {}", result.agent_id, result.summary),
            )
            .map_err(internal_error)?;
        }
        let completed = TurnCompleted {
            session_id: session.session_id,
            run_id,
            trace_id,
            stage: RunStage::Planning,
            final_text,
            events: if sub_agent_result.is_some() {
                let mut events = outcome.events;
                events.push("sub_agent.completed".into());
                events
            } else {
                outcome.events
            },
            provider: outcome.provider.provider,
            model: outcome.provider.model,
            input_tokens: outcome.input_tokens,
            output_tokens: outcome.output_tokens,
            context_ratio: outcome.context_ratio,
        };
        return Ok(Json(ExecTurnResponse { started, completed }));
    }

    if !outcome.tool_calls.is_empty() {
        let workspace_root = std::path::PathBuf::from(&session.workspace_root);
        let user_home = user_home().map_err(internal_error)?;
        let mut react_iterations = 0usize;
        let max_react_iterations = 3usize;
        let (mut final_text, completed_stage, mut events) = loop {
            let mut final_parts = Vec::new();
            if !outcome.final_text.trim().is_empty() {
                final_parts.push(outcome.final_text.clone());
            }
            let mut completed_stage = RunStage::Completed;
            let mut events = outcome.events.clone();
            let mut tool_feedback = Vec::new();

            for call in outcome.tool_calls.clone() {
                let tool_name = call.name.clone();
                let tool_call_id = call.call_id.clone();
                let tool_outcome = execute_tool_call(
                    &state,
                    Some(session.session_id.clone()),
                    call,
                    ToolExecutionContext {
                        workspace_root: workspace_root.clone(),
                        user_home: user_home.clone(),
                        permission_mode: session.permission_mode,
                    },
                    run_id.clone(),
                    trace_id.clone(),
                )
                .await
                .map_err(internal_error)?;
                if tool_outcome.approval_id.is_some() {
                    completed_stage = RunStage::WaitingApproval;
                }
                events.push(format!("tool.{}", tool_outcome.status));
                let tool_text = tool_retry_feedback_text(&tool_name, &tool_outcome);
                final_parts.push(tool_text.clone());
                tool_feedback.push(TranscriptMessage::Tool {
                    session_id: session.session_id.clone(),
                    run_id: run_id.clone(),
                    tool_call_id,
                    tool_name,
                    content: tool_text,
                    recorded_at: now_utc(),
                });
            }

            if completed_stage != RunStage::WaitingApproval
                && react_iterations < max_react_iterations
            {
                react_iterations += 1;
                runtime_history.extend(tool_feedback);
                outcome = agent
                    .run(AgentRuntime {
                        session_id: session.session_id.clone(),
                        conversation_id: started.conversation_id.clone(),
                        run_id: run_id.clone(),
                        trace_id: trace_id.clone(),
                        agent_id: AgentId::new_root(),
                        input: request.prompt.clone(),
                        history: runtime_history.clone(),
                        permission_mode: session.permission_mode,
                    })
                    .await
                    .map_err(internal_error)?;
                append_tool_trace(
                    &state,
                    &trace_id,
                    &run_id,
                    "tool.react_continued",
                    "continuing model after tool execution",
                )
                .map_err(internal_error)?;
                if !outcome.tool_calls.is_empty() {
                    continue;
                }
                if !outcome.final_text.trim().is_empty() {
                    final_parts.push(outcome.final_text.clone());
                }
            }

            break (final_parts.join("\n\n"), completed_stage, events);
        };
        if completed_stage == RunStage::Completed {
            if !outcome.tool_calls.is_empty() {
                events.push("tool.react_loop_limited".into());
                final_text = if final_text.trim().is_empty() {
                    "Reached ReAct tool-loop limit before a final answer. Try refining the prompt or narrowing tool scope.".into()
                } else {
                    format!(
                        "Reached ReAct tool-loop limit before a final answer.\n\nLatest tool feedback:\n{}",
                        final_text
                    )
                };
            } else if outcome.final_text.trim().is_empty() && !final_text.trim().is_empty() {
                final_text = format!(
                    "Tool execution completed, but the model returned no final synthesis.\n\nLatest tool feedback:\n{}",
                    final_text
                );
            }
        }
        persist_exec_result(
            &state,
            &session,
            &run_id,
            &trace_id,
            &request.prompt,
            &final_text,
            &outcome.provider.provider,
            &outcome.provider.model,
            outcome.input_tokens,
            outcome.output_tokens,
            outcome.context_ratio,
        )
        .map_err(internal_error)?;
        append_run_trace(
            &state,
            &trace_id,
            &run_id,
            "run.completed",
            &final_text,
            outcome.input_tokens,
            outcome.output_tokens,
            outcome.context_ratio,
        )
        .map_err(internal_error)?;
        let completed = TurnCompleted {
            session_id: session.session_id,
            run_id,
            trace_id,
            stage: completed_stage,
            final_text,
            events,
            provider: outcome.provider.provider,
            model: outcome.provider.model,
            input_tokens: outcome.input_tokens,
            output_tokens: outcome.output_tokens,
            context_ratio: outcome.context_ratio,
        };
        return Ok(Json(ExecTurnResponse { started, completed }));
    }

    persist_exec_result(
        &state,
        &session,
        &run_id,
        &trace_id,
        &request.prompt,
        &outcome.final_text,
        &outcome.provider.provider,
        &outcome.provider.model,
        outcome.input_tokens,
        outcome.output_tokens,
        outcome.context_ratio,
    )
    .map_err(internal_error)?;

    append_run_trace(
        &state,
        &trace_id,
        &run_id,
        "run.completed",
        &outcome.final_text,
        outcome.input_tokens,
        outcome.output_tokens,
        outcome.context_ratio,
    )
    .map_err(internal_error)?;

    let completed = TurnCompleted {
        session_id: session.session_id,
        run_id,
        trace_id,
        stage: RunStage::Completed,
        final_text: outcome.final_text,
        events: outcome.events,
        provider: outcome.provider.provider,
        model: outcome.provider.model,
        input_tokens: outcome.input_tokens,
        output_tokens: outcome.output_tokens,
        context_ratio: outcome.context_ratio,
    };
    Ok(Json(ExecTurnResponse { started, completed }))
}

async fn execute_tool(
    State(state): State<DaemonState>,
    Json(request): Json<ToolExecuteRequest>,
) -> Result<Json<ToolExecuteResponse>, (StatusCode, String)> {
    let workspace_root = request
        .workspace_root
        .as_deref()
        .map(std::path::PathBuf::from)
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)
        .map_err(internal_error)?;
    let user_home = user_home().map_err(internal_error)?;
    let run_id = RunId::new();
    let trace_id = TraceId::new();
    let call = ToolCall {
        call_id: format!("tool_{}", uuid::Uuid::new_v4()),
        name: request.name,
        arguments: request.arguments,
    };
    let outcome = execute_tool_call(
        &state,
        None,
        call,
        ToolExecutionContext {
            workspace_root,
            user_home,
            permission_mode: request.permission_mode,
        },
        run_id,
        trace_id,
    )
    .await
    .map_err(internal_error)?;

    Ok(Json(ToolExecuteResponse {
        status: outcome.status,
        content: outcome.content,
        reason: outcome.reason,
        approval_id: outcome.approval_id,
    }))
}

async fn list_approvals(State(state): State<DaemonState>) -> Json<ApprovalListResponse> {
    let pending = state
        .pending_approvals
        .lock()
        .expect("approval queue poisoned")
        .iter()
        .map(|approval| PendingApprovalSummary {
            approval_id: approval.approval_id.clone(),
            tool_call_id: approval.call.call_id.clone(),
            tool_name: approval.call.name.clone(),
            call_arguments_json: approval.call.arguments.to_string(),
            reason: approval.reason.clone(),
            workspace_root: approval.workspace_root.to_string_lossy().to_string(),
            requested_at: approval.requested_at,
        })
        .collect();
    Json(ApprovalListResponse { pending })
}

async fn list_approval_history(
    State(state): State<DaemonState>,
) -> Result<Json<ApprovalHistoryResponse>, (StatusCode, String)> {
    let grants = state
        .store
        .lock()
        .expect("session store poisoned")
        .list_approval_grants()
        .map_err(internal_error)?;
    Ok(Json(ApprovalHistoryResponse { grants }))
}

async fn resolve_approval(
    State(state): State<DaemonState>,
    Json(request): Json<ApprovalResolveRequest>,
) -> Result<Json<ApprovalResolveResponse>, (StatusCode, String)> {
    let approval = {
        let mut approvals = state
            .pending_approvals
            .lock()
            .expect("approval queue poisoned");
        let Some(index) = approvals
            .iter()
            .position(|item| item.approval_id == request.approval_id)
        else {
            return Ok(Json(ApprovalResolveResponse {
                approval_id: request.approval_id,
                status: "not_found".into(),
                content: None,
                reason: Some("approval already resolved or not found".into()),
                follow_up_text: None,
            }));
        };
        approvals.remove(index)
    };

    if !request.approved {
        persist_approval_grant(&state, &approval, false, Some("denied by user".into()))
            .map_err(internal_error)?;
        append_tool_trace(
            &state,
            &approval.trace_id,
            &approval.run_id,
            "approval.resolved",
            format!("denied: {}", approval.call.name),
        )
        .map_err(internal_error)?;
        append_tool_trace(
            &state,
            &approval.trace_id,
            &approval.run_id,
            "approval.denied",
            approval.call.name.clone(),
        )
        .map_err(internal_error)?;
        if approval.call.name == "skill-tool" {
            append_tool_trace(
                &state,
                &approval.trace_id,
                &approval.run_id,
                "skill.denied",
                skill_trace_message(&approval.call),
            )
            .map_err(internal_error)?;
        }
        return Ok(Json(ApprovalResolveResponse {
            approval_id: approval.approval_id,
            status: "denied".into(),
            content: None,
            reason: Some("denied by user".into()),
            follow_up_text: None,
        }));
    }

    let registry = ToolRegistry::with_builtin_tools();
    let call_name = approval.call.name.clone();
    let outcome = registry
        .execute(
            approval.call.clone(),
            ToolExecutionContext {
                workspace_root: approval.workspace_root.clone(),
                user_home: approval.user_home.clone(),
                permission_mode: unio_protocol::PermissionMode::FullTrust,
            },
        )
        .await;
    persist_approval_grant(&state, &approval, true, outcome.reason.clone())
        .map_err(internal_error)?;
    append_tool_trace(
        &state,
        &approval.trace_id,
        &approval.run_id,
        "approval.resolved",
        format!("approved: {call_name}"),
    )
    .map_err(internal_error)?;
    append_tool_trace(
        &state,
        &approval.trace_id,
        &approval.run_id,
        tool_event_kind(&outcome.status),
        call_name,
    )
    .map_err(internal_error)?;
    if approval.call.name == "skill-tool" {
        append_tool_trace(
            &state,
            &approval.trace_id,
            &approval.run_id,
            skill_event_kind(&outcome.status),
            outcome
                .reason
                .clone()
                .unwrap_or_else(|| skill_trace_message(&approval.call)),
        )
        .map_err(internal_error)?;
    }
    let approval_tool_feedback = approval_tool_feedback_text(&approval.call, &outcome);
    if let Some(session_id) = &approval.session_id {
        append_tool_transcript(
            &state,
            session_id,
            &approval.run_id,
            &approval.call.call_id,
            &approval.call.name,
            &approval_tool_feedback,
        )
        .map_err(internal_error)?;
    }
    let mut follow_up_text = None;
    if !matches!(outcome.status, ToolExecutionStatus::ApprovalRequired) {
        if let Some(session_id) = &approval.session_id {
            let session = {
                let store = state.store.lock().expect("session store poisoned");
                store.find_session(session_id).ok().flatten()
            };
            if let Some(session) = session {
                let history =
                    recent_transcript_messages(&state, session_id, 24).map_err(internal_error)?;
                let agent = RootAgentRuntime::new();
                let continued = agent
                    .run(AgentRuntime {
                        session_id: session_id.clone(),
                        conversation_id: ConversationId::new(),
                        run_id: approval.run_id.clone(),
                        trace_id: approval.trace_id.clone(),
                        agent_id: AgentId::new_root(),
                        input: "Continue based on the latest tool result (including failures), then answer the user clearly.".into(),
                        history,
                        permission_mode: session.permission_mode,
                    })
                    .await
                    .map_err(internal_error)?;
                if !continued.final_text.trim().is_empty() {
                    append_assistant_transcript(
                        &state,
                        session_id,
                        &approval.run_id,
                        &continued.final_text,
                    )
                    .map_err(internal_error)?;
                    append_run_trace(
                        &state,
                        &approval.trace_id,
                        &approval.run_id,
                        "approval.followup.completed",
                        &continued.final_text,
                        continued.input_tokens,
                        continued.output_tokens,
                        continued.context_ratio,
                    )
                    .map_err(internal_error)?;
                    follow_up_text = Some(continued.final_text);
                }
            }
        }
    }

    Ok(Json(ApprovalResolveResponse {
        approval_id: approval.approval_id,
        status: tool_status_label(&outcome.status).into(),
        content: outcome.result.map(|result| result.content),
        reason: outcome.reason,
        follow_up_text,
    }))
}

#[derive(Debug, Clone)]
struct ToolRunOutcome {
    status: String,
    content: Option<String>,
    reason: Option<String>,
    approval_id: Option<ApprovalId>,
}

async fn execute_tool_call(
    state: &DaemonState,
    session_id: Option<unio_core::SessionId>,
    call: ToolCall,
    context: ToolExecutionContext,
    run_id: RunId,
    trace_id: TraceId,
) -> anyhow::Result<ToolRunOutcome> {
    let registry = ToolRegistry::with_builtin_tools();
    let workspace_root = context.workspace_root.clone();
    let user_home = context.user_home.clone();
    append_tool_trace(state, &trace_id, &run_id, "tool.started", call.name.clone())?;
    if call.name == "skill-tool" {
        append_tool_trace(
            state,
            &trace_id,
            &run_id,
            "skill.started",
            skill_trace_message(&call),
        )?;
    }
    let outcome = registry.execute(call.clone(), context).await;
    let approval_id = if matches!(outcome.status, ToolExecutionStatus::ApprovalRequired) {
        let approval_id = ApprovalId::new();
        state
            .pending_approvals
            .lock()
            .expect("approval queue poisoned")
            .push(PendingApproval {
                session_id: session_id.clone(),
                approval_id: approval_id.clone(),
                call: call.clone(),
                reason: outcome.reason.clone().unwrap_or_default(),
                workspace_root,
                user_home,
                run_id: run_id.clone(),
                trace_id: trace_id.clone(),
                requested_at: now_utc(),
            });
        Some(approval_id)
    } else {
        None
    };

    append_tool_trace(
        state,
        &trace_id,
        &run_id,
        tool_event_kind(&outcome.status),
        approval_id
            .as_ref()
            .map(|id| format!("{}: {}", call.name, id))
            .unwrap_or_else(|| call.name.clone()),
    )?;
    if call.name == "skill-tool" {
        append_tool_trace(
            state,
            &trace_id,
            &run_id,
            skill_event_kind(&outcome.status),
            outcome
                .reason
                .clone()
                .unwrap_or_else(|| skill_trace_message(&call)),
        )?;
    }

    if let Some(result) = &outcome.result {
        if let Some(session_id) = &session_id {
            append_tool_transcript(
                state,
                session_id,
                &run_id,
                &result.call_id,
                &result.name,
                &result.content,
            )?;
        }
    }

    Ok(ToolRunOutcome {
        status: tool_status_label(&outcome.status).into(),
        content: outcome.result.map(|result| result.content),
        reason: outcome.reason,
        approval_id,
    })
}

fn parse_tool_directive(prompt: &str) -> Option<ToolCall> {
    let rest = prompt.trim().strip_prefix("/tool ")?;
    let (name, args) = rest
        .split_once(' ')
        .map(|(name, args)| (name, args))
        .unwrap_or((rest, ""));
    if name.trim().is_empty() {
        return None;
    }
    Some(ToolCall {
        call_id: format!("tool_{}", uuid::Uuid::new_v4()),
        name: name.trim().to_string(),
        arguments: parse_tool_args(args.trim()).ok()?,
    })
}

fn should_enter_planning(prompt: &str) -> bool {
    let trimmed = prompt.trim();
    let prompt = trimmed.to_ascii_lowercase();
    prompt.starts_with("plan ")
        || prompt.starts_with("mock-plan ")
        || prompt.contains("complex task")
        || prompt.contains("systematic")
        || trimmed.contains("复杂任务")
}

fn context_compaction_required(latest_context_ratio: f32, prompt: &str) -> bool {
    latest_context_ratio >= 0.90
        && is_large_task_prompt(prompt)
        && !prompt.trim().starts_with("mock-usage ")
}

fn is_large_task_prompt(prompt: &str) -> bool {
    should_enter_planning(prompt) || prompt.chars().count() > 2_000
}

fn parse_tool_args(value: &str) -> anyhow::Result<serde_json::Value> {
    if value.is_empty() {
        return Ok(serde_json::Value::Object(serde_json::Map::new()));
    }
    match serde_json::from_str(value) {
        Ok(json) => Ok(json),
        Err(json_error) => {
            let mut object = serde_json::Map::new();
            for pair in value.split(',').filter(|part| !part.trim().is_empty()) {
                let Some((key, raw_value)) = pair.split_once('=') else {
                    return Err(json_error.into());
                };
                object.insert(
                    key.trim().to_string(),
                    serde_json::Value::String(raw_value.trim().to_string()),
                );
            }
            Ok(serde_json::Value::Object(object))
        }
    }
}

fn tool_final_text(outcome: &ToolRunOutcome) -> String {
    if let Some(content) = &outcome.content {
        return content.clone();
    }
    if let Some(approval_id) = &outcome.approval_id {
        return format!("Tool execution is waiting for approval: {}", approval_id);
    }
    outcome
        .reason
        .clone()
        .unwrap_or_else(|| format!("Tool finished with status {}", outcome.status))
}

fn tool_retry_feedback_text(tool_name: &str, outcome: &ToolRunOutcome) -> String {
    if let Some(content) = &outcome.content {
        return format!(
            "tool={tool_name}\nstatus={}\noutput:\n{}",
            outcome.status, content
        );
    }
    if let Some(approval_id) = &outcome.approval_id {
        return format!(
            "tool={tool_name}\nstatus=approval_required\napproval_id={approval_id}\nmessage=Tool `{tool_name}` is waiting for approval: {approval_id}"
        );
    }
    if let Some(reason) = &outcome.reason {
        return format!(
            "tool={tool_name}\nstatus={}\nerror={}\nhint={}",
            outcome.status,
            reason,
            tool_environment_retry_hint(tool_name, reason)
        );
    }
    format!(
        "tool={tool_name}\nstatus={}\nhint={}",
        outcome.status,
        tool_environment_retry_hint(tool_name, "")
    )
}

fn approval_tool_feedback_text(
    call: &ToolCall,
    outcome: &unio_tools::ToolExecutionOutcome,
) -> String {
    let mut lines = vec![
        format!("tool={}", call.name),
        format!("status={}", tool_status_label(&outcome.status)),
    ];
    if let Some(result) = &outcome.result {
        if !result.content.trim().is_empty() {
            lines.push("output:".into());
            lines.push(result.content.clone());
        }
    }
    if let Some(reason) = &outcome.reason {
        if !reason.trim().is_empty() {
            lines.push(format!("error={reason}"));
        }
    }
    lines.join("\n")
}

fn tool_environment_retry_hint(tool_name: &str, reason: &str) -> &'static str {
    if tool_name != "bash" {
        return "Adjust arguments based on this tool error and retry once.";
    }
    #[cfg(windows)]
    {
        if reason.to_ascii_lowercase().contains("shell syntax")
            || reason.contains("参数格式不正确")
            || reason.to_ascii_lowercase().contains("find:")
        {
            return "Windows detected: use cmd/PowerShell-compatible command style, avoid Unix-only syntax, and prefer direct executable commands.";
        }
        "Windows detected: rewrite bash command in cmd/PowerShell style and retry."
    }
    #[cfg(not(windows))]
    {
        "Unix-like shell detected: rewrite bash command in POSIX style and retry.";
    }
}

#[allow(clippy::too_many_arguments)]
fn persist_exec_result(
    state: &DaemonState,
    session: &SessionSummary,
    run_id: &RunId,
    trace_id: &TraceId,
    prompt: &str,
    final_text: &str,
    provider: &str,
    model: &str,
    input_tokens: usize,
    output_tokens: usize,
    context_ratio: f32,
) -> anyhow::Result<()> {
    let store = state.store.lock().expect("session store poisoned");
    let transcript = JsonlTranscriptStore::new(
        state
            .paths
            .transcripts_dir
            .join(format!("{}.jsonl", session.session_id)),
    );
    transcript.append(&TranscriptMessage::User {
        session_id: session.session_id.clone(),
        run_id: run_id.clone(),
        content: prompt.to_string(),
        recorded_at: now_utc(),
    })?;
    transcript.append(&TranscriptMessage::Assistant {
        session_id: session.session_id.clone(),
        run_id: run_id.clone(),
        content: final_text.to_string(),
        reasoning_content: None,
        recorded_at: now_utc(),
    })?;
    store.insert_run(&RunRecord {
        run_id: run_id.clone(),
        session_id: session.session_id.clone(),
        prompt: prompt.to_string(),
        final_text: final_text.to_string(),
        trace_id: trace_id.to_string(),
        provider: provider.to_string(),
        model: model.to_string(),
        input_tokens,
        output_tokens,
        context_ratio,
        created_at: now_utc(),
    })?;
    store.touch_session(&session.session_id, run_id)?;
    Ok(())
}

fn append_tool_transcript(
    state: &DaemonState,
    session_id: &unio_core::SessionId,
    run_id: &RunId,
    tool_call_id: &str,
    tool_name: &str,
    content: &str,
) -> anyhow::Result<()> {
    let transcript = JsonlTranscriptStore::new(
        state
            .paths
            .transcripts_dir
            .join(format!("{}.jsonl", session_id)),
    );
    transcript.append(&TranscriptMessage::Tool {
        session_id: session_id.clone(),
        run_id: run_id.clone(),
        tool_call_id: tool_call_id.to_string(),
        tool_name: tool_name.to_string(),
        content: content.to_string(),
        recorded_at: now_utc(),
    })
}

fn append_assistant_transcript(
    state: &DaemonState,
    session_id: &unio_core::SessionId,
    run_id: &RunId,
    content: &str,
) -> anyhow::Result<()> {
    let transcript = JsonlTranscriptStore::new(
        state
            .paths
            .transcripts_dir
            .join(format!("{}.jsonl", session_id)),
    );
    transcript.append(&TranscriptMessage::Assistant {
        session_id: session_id.clone(),
        run_id: run_id.clone(),
        content: content.to_string(),
        reasoning_content: None,
        recorded_at: now_utc(),
    })
}

fn recent_transcript_messages(
    state: &DaemonState,
    session_id: &unio_core::SessionId,
    limit: usize,
) -> anyhow::Result<Vec<TranscriptMessage>> {
    let transcript = JsonlTranscriptStore::new(
        state
            .paths
            .transcripts_dir
            .join(format!("{}.jsonl", session_id)),
    );
    let messages = transcript.read_all()?;
    let start = messages.len().saturating_sub(limit);
    Ok(messages
        .into_iter()
        .skip(start)
        .map(compact_transcript_message)
        .collect())
}

fn compact_transcript_message(message: TranscriptMessage) -> TranscriptMessage {
    match message {
        TranscriptMessage::User {
            session_id,
            run_id,
            content,
            recorded_at,
        } => TranscriptMessage::User {
            session_id,
            run_id,
            content: truncate_for_context(&content, MAX_CONTEXT_TEXT_CHARS),
            recorded_at,
        },
        TranscriptMessage::Assistant {
            session_id,
            run_id,
            content,
            reasoning_content,
            recorded_at,
        } => TranscriptMessage::Assistant {
            session_id,
            run_id,
            content: truncate_for_context(&content, MAX_CONTEXT_TEXT_CHARS),
            reasoning_content: reasoning_content
                .map(|content| truncate_for_context(&content, MAX_CONTEXT_TOOL_CHARS)),
            recorded_at,
        },
        TranscriptMessage::Tool {
            session_id,
            run_id,
            tool_call_id,
            tool_name,
            content,
            recorded_at,
        } => TranscriptMessage::Tool {
            session_id,
            run_id,
            tool_call_id,
            tool_name: tool_name.clone(),
            content: compact_tool_result(&tool_name, &content),
            recorded_at,
        },
    }
}

fn compact_tool_result(tool_name: &str, content: &str) -> String {
    if tool_name == "skill-tool" {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(content) {
            let skill_name = value
                .get("skill_name")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let summary = value
                .get("summary")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("no summary");
            return format!("skill `{skill_name}` result: {summary}");
        }
    }
    truncate_for_context(content, MAX_CONTEXT_TOOL_CHARS)
}

fn truncate_for_context(value: &str, max_chars: usize) -> String {
    let mut truncated = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        truncated.push_str("\n[truncated for context]");
    }
    truncated
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartOptions {
    pub bind_addr: String,
}

fn internal_error(error: impl ToString) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

fn tool_status_label(status: &ToolExecutionStatus) -> &'static str {
    match status {
        ToolExecutionStatus::Completed => "completed",
        ToolExecutionStatus::ApprovalRequired => "approval_required",
        ToolExecutionStatus::Denied => "denied",
        ToolExecutionStatus::Failed => "failed",
    }
}

fn tool_event_kind(status: &ToolExecutionStatus) -> &'static str {
    match status {
        ToolExecutionStatus::Completed => "tool.completed",
        ToolExecutionStatus::ApprovalRequired => "approval.requested",
        ToolExecutionStatus::Denied => "tool.denied",
        ToolExecutionStatus::Failed => "tool.failed",
    }
}

fn skill_event_kind(status: &ToolExecutionStatus) -> &'static str {
    match status {
        ToolExecutionStatus::Completed => "skill.completed",
        ToolExecutionStatus::ApprovalRequired => "skill.waiting_approval",
        ToolExecutionStatus::Denied => "skill.denied",
        ToolExecutionStatus::Failed => "skill.failed",
    }
}

fn skill_trace_message(call: &ToolCall) -> String {
    call.arguments
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(|name| format!("skill-tool:{name}"))
        .unwrap_or_else(|| "skill-tool".into())
}

fn append_tool_trace(
    state: &DaemonState,
    trace_id: &TraceId,
    run_id: &RunId,
    kind: impl Into<String>,
    message: impl Into<String>,
) -> anyhow::Result<()> {
    state.trace_store.append(&TraceEvent {
        trace_id: trace_id.clone(),
        run_id: run_id.clone(),
        kind: kind.into(),
        message: message.into(),
        token_usage: None,
        recorded_at: now_utc(),
    })
}

fn append_run_trace(
    state: &DaemonState,
    trace_id: &TraceId,
    run_id: &RunId,
    kind: impl Into<String>,
    message: impl Into<String>,
    input_tokens: usize,
    output_tokens: usize,
    context_ratio: f32,
) -> anyhow::Result<()> {
    let token_usage = unio_observability::TokenUsage {
        input_tokens,
        output_tokens,
        context_ratio,
    };
    state.trace_store.append(&TraceEvent {
        trace_id: trace_id.clone(),
        run_id: run_id.clone(),
        kind: kind.into(),
        message: message.into(),
        token_usage: Some(token_usage.clone()),
        recorded_at: now_utc(),
    })?;
    for event in context_budget_events(context_ratio) {
        state.trace_store.append(&TraceEvent {
            trace_id: trace_id.clone(),
            run_id: run_id.clone(),
            kind: event.kind,
            message: event.message,
            token_usage: Some(token_usage.clone()),
            recorded_at: now_utc(),
        })?;
    }
    Ok(())
}

fn persist_approval_grant(
    state: &DaemonState,
    approval: &PendingApproval,
    approved: bool,
    reason: Option<String>,
) -> anyhow::Result<()> {
    state
        .store
        .lock()
        .expect("session store poisoned")
        .insert_approval_grant(&ApprovalGrantRecord {
            approval_id: approval.approval_id.clone(),
            tool_call_id: approval.call.call_id.clone(),
            tool_name: approval.call.name.clone(),
            workspace_root: approval.workspace_root.clone(),
            approved,
            reason,
            resolved_at: now_utc(),
        })
}

fn user_home() -> anyhow::Result<std::path::PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(std::path::PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("failed to resolve user home"))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{
        compact_tool_result, compact_transcript_message, context_compaction_required, exec_turn,
        list_approval_history, list_approvals, load_transcript, now_utc, parse_tool_directive,
        query_trace, resolve_approval, resolve_session, should_enter_planning, skill_event_kind,
        tool_event_kind, DaemonState, PendingApproval,
    };
    use axum::extract::State;
    use axum::Json;
    use tempfile::{tempdir, TempDir};
    use unio_core::{ApprovalId, RunId, SessionId, TraceId, UserPaths};
    use unio_observability::{JsonlTraceStore, TraceEvent};
    use unio_protocol::{
        ApprovalResolveRequest, ExecTurnRequest, LoadTranscriptRequest, PermissionMode,
        ResolveSessionRequest, RunStage, SessionResolveStrategy, TraceLookupRequest,
        TranscriptMessage,
    };
    use unio_storage::SqliteSessionStore;
    use unio_tools::{ToolCall, ToolExecutionStatus};

    #[test]
    fn parses_exec_tool_directive() {
        let call = parse_tool_directive("/tool read path=README.md").unwrap();

        assert_eq!(call.name, "read");
        assert_eq!(call.arguments["path"], "README.md");
    }

    #[test]
    fn planning_detection_accepts_english_and_chinese_triggers() {
        assert!(should_enter_planning("complex task: refactor workspace"));
        assert!(should_enter_planning("这是一个复杂任务"));
    }

    #[test]
    fn trace_event_kinds_are_stable_for_tools_and_skills() {
        assert_eq!(
            tool_event_kind(&ToolExecutionStatus::Completed),
            "tool.completed"
        );
        assert_eq!(
            tool_event_kind(&ToolExecutionStatus::ApprovalRequired),
            "approval.requested"
        );
        assert_eq!(
            skill_event_kind(&ToolExecutionStatus::Completed),
            "skill.completed"
        );
        assert_eq!(
            skill_event_kind(&ToolExecutionStatus::Failed),
            "skill.failed"
        );
    }

    #[test]
    fn high_context_requires_compaction_for_large_tasks_only() {
        assert!(context_compaction_required(
            0.90,
            "plan refactor the workspace safely"
        ));
        assert!(context_compaction_required(0.95, &"x".repeat(2_001)));
        assert!(!context_compaction_required(0.89, "plan refactor"));
        assert!(!context_compaction_required(0.95, "quick question"));
        assert!(!context_compaction_required(
            0.95,
            "mock-usage input=90000,output=90000"
        ));
    }

    #[test]
    fn compact_tool_result_summarizes_skill_agent_output() {
        let content = serde_json::json!({
            "skill_name": "repo",
            "summary": "inspected modules",
            "artifacts": [],
            "status": "completed",
            "private": "Private instruction body"
        })
        .to_string();

        let compacted = compact_tool_result("skill-tool", &content);

        assert_eq!(compacted, "skill `repo` result: inspected modules");
        assert!(!compacted.contains("Private instruction body"));
    }

    #[test]
    fn compact_transcript_message_truncates_large_tool_output() {
        let content = "x".repeat(2_000);
        let message = compact_transcript_message(TranscriptMessage::Tool {
            session_id: SessionId::from_string("session_1"),
            run_id: RunId::from_string("run_1"),
            tool_call_id: "call_1".into(),
            tool_name: "read".into(),
            content,
            recorded_at: now_utc(),
        });

        let TranscriptMessage::Tool { content, .. } = message else {
            panic!("expected tool message");
        };
        assert!(content.len() < 1_100);
        assert!(content.contains("[truncated for context]"));
    }

    #[tokio::test]
    async fn daemon_executes_model_requested_skill_tool_and_persists_trace() {
        let harness = TestHarness::new();
        let skill_dir = harness.workspace.path().join(".unio/skills/repo");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "# Repo skill\nPrivate instruction body",
        )
        .unwrap();
        let session = resolve_test_session(&harness, PermissionMode::FullTrust).await;

        let response = exec_turn(
            State(harness.state.clone()),
            Json(ExecTurnRequest {
                session_id: session.session_id.clone(),
                prompt: "mock-tool skill-tool name=repo,request=inspect-modules".into(),
            }),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(response.completed.stage, RunStage::Completed);
        assert!(response
            .completed
            .final_text
            .contains("repo"));
        assert!(!response
            .completed
            .final_text
            .contains("Private instruction body"));

        let trace = query_trace(
            State(harness.state.clone()),
            Json(TraceLookupRequest {
                trace_id: response.completed.trace_id.clone(),
                run_id: None,
            }),
        )
        .await
        .unwrap()
        .0;
        let kinds = trace
            .events
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>();
        assert!(kinds.contains(&"tool.started"));
        assert!(kinds.contains(&"skill.started"));
        assert!(kinds.contains(&"tool.completed"));
        assert!(kinds.contains(&"skill.completed"));

        let transcript = load_transcript(
            State(harness.state),
            Json(LoadTranscriptRequest {
                session_id: session.session_id,
                limit: None,
            }),
        )
        .await
        .unwrap()
        .0;
        let serialized = serde_json::to_string(&transcript.messages).unwrap();
        assert!(serialized.contains("skill-tool"));
        assert!(!serialized.contains("Private instruction body"));
    }

    #[tokio::test]
    async fn daemon_requires_approval_for_model_requested_skill_tool_in_default_mode() {
        let harness = TestHarness::new();
        let skill_dir = harness.workspace.path().join(".unio/skills/repo");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# Repo skill").unwrap();
        let session = resolve_test_session(&harness, PermissionMode::Default).await;

        let response = exec_turn(
            State(harness.state.clone()),
            Json(ExecTurnRequest {
                session_id: session.session_id,
                prompt: "mock-tool skill-tool name=repo,request=inspect-modules".into(),
            }),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(response.completed.stage, RunStage::WaitingApproval);
        assert!(response
            .completed
            .final_text
            .contains("waiting for approval"));
        assert_eq!(
            harness
                .state
                .pending_approvals
                .lock()
                .expect("approval queue poisoned")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn approval_resolution_approves_pending_tool_and_persists_audit_records() {
        let harness = TestHarness::new();
        let session = resolve_test_session(&harness, PermissionMode::Default).await;
        let approval_id = ApprovalId::from_string("approval_test_completed");
        let run_id = RunId::from_string("run_completed_approval");
        let trace_id = TraceId::from_string("trace_completed_approval");
        harness
            .state
            .pending_approvals
            .lock()
            .expect("approval queue poisoned")
            .push(PendingApproval {
                session_id: Some(session.session_id.clone()),
                approval_id: approval_id.clone(),
                call: ToolCall {
                    call_id: "call_completed".into(),
                    name: "write".into(),
                    arguments: serde_json::json!({ "path": "approval.txt", "content": "ok" }),
                },
                reason: "requires approval".into(),
                workspace_root: harness.workspace.path().to_path_buf(),
                user_home: harness.workspace.path().to_path_buf(),
                run_id,
                trace_id: trace_id.clone(),
                requested_at: now_utc(),
            });

        let resolved = resolve_approval(
            State(harness.state.clone()),
            Json(ApprovalResolveRequest {
                approval_id,
                approved: true,
            }),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(resolved.status, "completed");
        assert_eq!(
            list_approvals(State(harness.state.clone()))
                .await
                .0
                .pending
                .len(),
            0
        );
        assert_eq!(
            list_approval_history(State(harness.state.clone()))
                .await
                .unwrap()
                .0
                .grants
                .len(),
            1
        );
        assert!(harness.workspace.path().join("approval.txt").exists());

        let transcript = load_transcript(
            State(harness.state.clone()),
            Json(LoadTranscriptRequest {
                session_id: session.session_id,
                limit: None,
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(serde_json::to_string(&transcript.messages)
            .unwrap()
            .contains("approval.txt"));

        let trace = query_trace(
            State(harness.state),
            Json(TraceLookupRequest {
                trace_id,
                run_id: None,
            }),
        )
        .await
        .unwrap()
        .0;
        let kinds = trace
            .events
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>();
        assert!(kinds.contains(&"approval.resolved"));
        assert!(kinds.contains(&"tool.completed"));
    }

    #[tokio::test]
    async fn approval_resolution_denies_pending_tool_and_records_denied_trace() {
        let harness = TestHarness::new();
        let session = resolve_test_session(&harness, PermissionMode::Default).await;
        let approval_id = ApprovalId::from_string("approval_test_denied");
        let run_id = RunId::from_string("run_denied_approval");
        let trace_id = TraceId::from_string("trace_denied_approval");
        harness
            .state
            .pending_approvals
            .lock()
            .expect("approval queue poisoned")
            .push(PendingApproval {
                session_id: Some(session.session_id.clone()),
                approval_id: approval_id.clone(),
                call: ToolCall {
                    call_id: "call_denied".into(),
                    name: "write".into(),
                    arguments: serde_json::json!({ "path": "denied.txt", "content": "no" }),
                },
                reason: "requires approval".into(),
                workspace_root: harness.workspace.path().to_path_buf(),
                user_home: harness.workspace.path().to_path_buf(),
                run_id: run_id.clone(),
                trace_id: trace_id.clone(),
                requested_at: now_utc(),
            });
        let resolved = resolve_approval(
            State(harness.state.clone()),
            Json(ApprovalResolveRequest {
                approval_id,
                approved: false,
            }),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(resolved.status, "denied");
        assert!(!harness.workspace.path().join("denied.txt").exists());
        let history = list_approval_history(State(harness.state.clone()))
            .await
            .unwrap()
            .0;
        assert!(history
            .grants
            .iter()
            .any(|grant| grant.approval_id == resolved.approval_id && !grant.approved));

        let trace = query_trace(
            State(harness.state),
            Json(TraceLookupRequest {
                trace_id,
                run_id: None,
            }),
        )
        .await
        .unwrap()
        .0;
        let kinds = trace
            .events
            .iter()
            .map(|event| event.kind.as_str())
            .collect::<Vec<_>>();
        assert!(kinds.contains(&"approval.resolved"));
        assert!(kinds.contains(&"approval.denied"));
    }

    #[tokio::test]
    async fn approval_resolution_failed_tool_still_returns_follow_up_text() {
        let harness = TestHarness::new();
        let session = resolve_test_session(&harness, PermissionMode::Default).await;
        let approval_id = ApprovalId::from_string("approval_test_failed");
        let run_id = RunId::from_string("run_failed_approval");
        let trace_id = TraceId::from_string("trace_failed_approval");
        harness
            .state
            .pending_approvals
            .lock()
            .expect("approval queue poisoned")
            .push(PendingApproval {
                session_id: Some(session.session_id.clone()),
                approval_id: approval_id.clone(),
                call: ToolCall {
                    call_id: "call_failed".into(),
                    name: "bash".into(),
                    arguments: serde_json::json!({ "command": "echo hi | sort" }),
                },
                reason: "requires approval".into(),
                workspace_root: harness.workspace.path().to_path_buf(),
                user_home: harness.workspace.path().to_path_buf(),
                run_id,
                trace_id,
                requested_at: now_utc(),
            });

        let resolved = resolve_approval(
            State(harness.state),
            Json(ApprovalResolveRequest {
                approval_id,
                approved: true,
            }),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(resolved.status, "failed");
        assert!(resolved.follow_up_text.is_some());
    }

    #[tokio::test]
    async fn transcript_loading_can_limit_to_recent_messages() {
        let harness = TestHarness::new();
        let session = resolve_test_session(&harness, PermissionMode::FullTrust).await;
        for prompt in ["first", "second", "third"] {
            let _ = exec_turn(
                State(harness.state.clone()),
                Json(ExecTurnRequest {
                    session_id: session.session_id.clone(),
                    prompt: prompt.into(),
                }),
            )
            .await
            .unwrap();
        }

        let transcript = load_transcript(
            State(harness.state),
            Json(LoadTranscriptRequest {
                session_id: session.session_id,
                limit: Some(2),
            }),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(transcript.messages.len(), 2);
        let serialized = serde_json::to_string(&transcript.messages).unwrap();
        assert!(serialized.contains("third"));
        assert!(!serialized.contains("first"));
    }

    #[tokio::test]
    async fn trace_lookup_can_filter_by_run_id() {
        let harness = TestHarness::new();
        let trace_id = unio_core::TraceId::from_string("trace_shared");
        let run_1 = RunId::from_string("run_1");
        let run_2 = RunId::from_string("run_2");
        for (run_id, kind) in [
            (run_1.clone(), "tool.completed"),
            (run_2.clone(), "approval.resolved"),
        ] {
            harness
                .state
                .trace_store
                .append(&TraceEvent {
                    trace_id: trace_id.clone(),
                    run_id,
                    kind: kind.into(),
                    message: kind.into(),
                    token_usage: None,
                    recorded_at: now_utc(),
                })
                .unwrap();
        }

        let trace = query_trace(
            State(harness.state),
            Json(TraceLookupRequest {
                trace_id,
                run_id: Some(run_2),
            }),
        )
        .await
        .unwrap()
        .0;

        assert_eq!(trace.events.len(), 1);
        assert_eq!(trace.events[0].kind, "approval.resolved");
        assert_eq!(trace.events[0].run_id, RunId::from_string("run_2"));
    }

    struct TestHarness {
        _home: TempDir,
        workspace: TempDir,
        state: DaemonState,
    }

    impl TestHarness {
        fn new() -> Self {
            let home = tempdir().unwrap();
            let workspace = tempdir().unwrap();
            let paths = test_user_paths(home.path().join(".unio"));
            paths.ensure().unwrap();
            let store = SqliteSessionStore::open(&paths.state_db_file).unwrap();
            let state = DaemonState {
                trace_store: Arc::new(JsonlTraceStore::new(&paths.traces_file)),
                paths,
                store: Arc::new(Mutex::new(store)),
                pending_approvals: Arc::new(Mutex::new(Vec::new())),
                started_at: now_utc(),
            };
            Self {
                _home: home,
                workspace,
                state,
            }
        }
    }

    async fn resolve_test_session(
        harness: &TestHarness,
        permission_mode: PermissionMode,
    ) -> unio_protocol::SessionSummary {
        resolve_session(
            State(harness.state.clone()),
            Json(ResolveSessionRequest {
                workspace_root: harness.workspace.path().to_string_lossy().to_string(),
                permission_mode,
                strategy: SessionResolveStrategy::ReuseWorkspaceLatest,
            }),
        )
        .await
        .unwrap()
        .0
        .session
    }

    fn test_user_paths(root: std::path::PathBuf) -> UserPaths {
        UserPaths {
            daemon_dir: root.join("daemon"),
            daemon_logs_dir: root.join("daemon/logs"),
            instance_file: root.join("daemon/instance.json"),
            sessions_dir: root.join("sessions"),
            state_db_file: root.join("sessions/state.db"),
            transcripts_dir: root.join("sessions/transcripts"),
            traces_dir: root.join("traces"),
            traces_file: root.join("traces/events.jsonl"),
            root,
        }
    }
}
