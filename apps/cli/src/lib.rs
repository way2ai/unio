use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

use anyhow::Context;
use clap::{Parser, Subcommand};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Terminal;
use reqwest::Client;
use unio_core::{read_instance_file, DaemonInstance, RunId, SessionId, UserPaths, WorkspacePaths};
use unio_protocol::{
    ApprovalHistoryResponse, ApprovalListResponse, ApprovalResolveRequest, ApprovalResolveResponse,
    DaemonStatus, ExecTurnRequest, ExecTurnResponse, LoadTranscriptRequest, LoadTranscriptResponse,
    ModelsStatus, PendingApprovalSummary, PermissionMode, ResolveSessionRequest,
    ResolveSessionResponse, RunStage, SessionResolveStrategy, SessionSummary, ToolExecuteRequest,
    ToolExecuteResponse, TraceLookupRequest, TraceLookupResponse, TranscriptMessage,
};
use unio_skills::{discover_skills, inject_skill_tools, SkillSource};

#[derive(Debug, Parser)]
#[command(name = "unio")]
#[command(about = "Unio CLI")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<CommandSpec>,
    prompt: Option<String>,
}

#[derive(Debug, Subcommand)]
enum CommandSpec {
    Exec {
        prompt: String,
        #[arg(long, default_value = "default")]
        approval: ApprovalArg,
        #[arg(long)]
        quiet: bool,
    },
    Resume {
        #[arg(long)]
        limit: Option<usize>,
        session_id: Option<String>,
    },
    Sessions,
    Skills,
    Models,
    Status,
    Update,
    Trace {
        trace_id: String,
        #[arg(long)]
        run: Option<String>,
    },
    Tool {
        name: String,
        #[arg(long, default_value = "{}")]
        args: String,
        #[arg(long, default_value = "default")]
        approval: ApprovalArg,
    },
    Approvals {
        #[command(subcommand)]
        command: Option<ApprovalCommand>,
    },
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
    Architecture,
}

#[derive(Debug, Subcommand)]
enum DaemonCommand {
    Start,
    Status,
}

#[derive(Debug, Subcommand)]
enum ApprovalCommand {
    History,
    Approve { approval_id: String },
    Deny { approval_id: String },
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum ApprovalArg {
    Default,
    Auto,
    FullTrust,
}

impl From<ApprovalArg> for PermissionMode {
    fn from(value: ApprovalArg) -> Self {
        match value {
            ApprovalArg::Default => Self::Default,
            ApprovalArg::Auto => Self::Auto,
            ApprovalArg::FullTrust => Self::FullTrust,
        }
    }
}

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Some(CommandSpec::Exec {
            prompt,
            approval,
            quiet,
        }) => run_exec(prompt, approval.into(), ExecOutputMode { quiet }).await,
        Some(CommandSpec::Resume { session_id, limit }) => run_resume(session_id, limit).await,
        Some(CommandSpec::Sessions) => run_sessions().await,
        Some(CommandSpec::Skills) => run_skills(),
        Some(CommandSpec::Models) => run_models().await,
        Some(CommandSpec::Status) => run_status().await,
        Some(CommandSpec::Update) => run_update(),
        Some(CommandSpec::Trace { trace_id, run }) => run_trace(trace_id, run).await,
        Some(CommandSpec::Tool {
            name,
            args,
            approval,
        }) => run_tool(name, args, approval.into()).await,
        Some(CommandSpec::Approvals { command }) => run_approvals(command).await,
        Some(CommandSpec::Daemon { command }) => run_daemon(command).await,
        Some(CommandSpec::Architecture) => {
            println!("cli -> daemon -> agent -> model/tools/security -> storage/trace");
            Ok(())
        }
        None => {
            if let Some(prompt) = cli.prompt {
                run_prompt_or_slash(prompt).await
            } else {
                run_tui().await
            }
        }
    }
}

async fn run_prompt_or_slash(prompt: String) -> anyhow::Result<()> {
    let trimmed = prompt.trim();
    if trimmed == "/approve" {
        return run_guided_approval_resolution(true).await;
    }
    if trimmed == "/deny" {
        return run_guided_approval_resolution(false).await;
    }
    if trimmed == "/trace" {
        return run_guided_trace_query().await;
    }
    if let Some((approved, approval_id)) = parse_approval_slash(trimmed) {
        let client = daemon_client(true).await?;
        return resolve_approval(&client, approval_id, approved).await;
    }
    if let Some((trace_id, run_id)) = parse_trace_slash(trimmed) {
        return run_trace(trace_id, run_id).await;
    }
    if let Some(limit) = parse_resume_slash(trimmed) {
        return run_resume(None, limit).await;
    }
    if let Some(mode) = parse_approval_mode_slash(trimmed) {
        return run_approval_mode_config(mode).await;
    }
    if trimmed == "/new" {
        return run_new_session().await;
    }
    if trimmed.starts_with("/approval ") {
        anyhow::bail!("invalid approval mode; use /approval <default|auto|full-trust>");
    }

    match trimmed {
        "/skills" => run_skills(),
        "/pending" => run_approvals(None).await,
        "/update" => run_update(),
        "?" | "/?" => {
            print_slash_help();
            Ok(())
        }
        _ => {
            if parse_model_slash(trimmed).is_some() {
                return run_model_config().await;
            }
            run_exec(
                prompt,
                PermissionMode::Default,
                ExecOutputMode { quiet: false },
            )
            .await
        }
    }
}

async fn run_tui() -> anyhow::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_tui_loop(&mut terminal).await;
    let cleanup = (|| -> anyhow::Result<()> {
        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
        terminal.show_cursor()?;
        Ok(())
    })();

    result.and(cleanup)
}

async fn run_tui_loop(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    let workspace = std::env::current_dir()?;
    let workspace_text = workspace.to_string_lossy().to_string();
    let initial_session = resolve_session(
        &client,
        &workspace,
        PermissionMode::Default,
        SessionResolveStrategy::CreateNew,
    )
    .await?;
    let mut status = fetch_status(&client).await?;
    let mut input = InputBuffer::default();
    let mut scroll = 0_u16;
    let mut current_stage = "idle".to_string();
    let mut current_permission_mode = initial_session.permission_mode;
    let mut active_session_id = initial_session.session_id.clone();
    let mut latest_pending_approval_id: Option<String> = None;
    let mut latest_pending_approval: Option<PendingApprovalSummary> = None;
    let mut resume_picker: Option<ResumePickerState> = None;
    let mut ctrl_c_armed = false;
    let mut selected_file_suggestion = 0_usize;
    let mut selected_slash_suggestion = 0_usize;
    let mut selected_approval_choice = 0_usize;
    let mut messages = Vec::<String>::new();
    let mut model_config_wizard: Option<ModelConfigWizard> = None;
    let file_index = FileReferenceIndex::start(workspace.clone());

    loop {
        let input_before_cursor = input.before_cursor();
        let file_suggestions = file_index.suggestions(input_before_cursor);
        if selected_file_suggestion >= file_suggestions.len() {
            selected_file_suggestion = 0;
        }
        let slash_suggestions = slash_command_suggestions(input_before_cursor);
        if selected_slash_suggestion >= slash_suggestions.len() {
            selected_slash_suggestion = 0;
        }
        draw_tui(
            terminal,
            &status,
            &workspace,
            &current_stage,
            latest_pending_approval_id.as_deref(),
            latest_pending_approval.as_ref(),
            selected_approval_choice,
            &messages,
            &input,
            scroll,
            ctrl_c_armed,
            &file_suggestions,
            selected_file_suggestion,
            &slash_suggestions,
            selected_slash_suggestion,
            model_config_wizard.as_ref(),
        )?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Esc => {
                if resume_picker.take().is_some() {
                    messages.push("system: resume cancelled".into());
                    continue;
                }
                break;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if ctrl_c_armed {
                    break;
                }
                ctrl_c_armed = true;
                continue;
            }
            KeyCode::Backspace => {
                input.backspace();
                ctrl_c_armed = false;
            }
            KeyCode::Delete => {
                input.delete();
                ctrl_c_armed = false;
            }
            KeyCode::Left => {
                input.move_left();
                ctrl_c_armed = false;
            }
            KeyCode::Right => {
                input.move_right();
                ctrl_c_armed = false;
            }
            KeyCode::Home => {
                input.move_home();
                ctrl_c_armed = false;
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.move_home();
                ctrl_c_armed = false;
            }
            KeyCode::End => {
                input.move_end();
                ctrl_c_armed = false;
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.move_end();
                ctrl_c_armed = false;
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.clear();
                ctrl_c_armed = false;
            }
            KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.delete_previous_word();
                ctrl_c_armed = false;
            }
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                input.insert('\n');
                ctrl_c_armed = false;
            }
            KeyCode::Up => {
                if let Some(picker) = resume_picker.as_mut() {
                    picker.selected = picker.selected.saturating_sub(1);
                    upsert_resume_picker_message(&mut messages, picker);
                    continue;
                }
                if input.is_empty()
                    && latest_pending_approval_id.is_some()
                    && file_suggestions.is_empty()
                    && slash_suggestions.is_empty()
                {
                    selected_approval_choice = selected_approval_choice.saturating_sub(1);
                } else if !file_suggestions.is_empty() {
                    selected_file_suggestion = selected_file_suggestion.saturating_sub(1);
                } else if !slash_suggestions.is_empty() {
                    selected_slash_suggestion = selected_slash_suggestion.saturating_sub(1);
                } else {
                    scroll = scroll.saturating_sub(1);
                }
            }
            KeyCode::Down => {
                if let Some(picker) = resume_picker.as_mut() {
                    if picker.selected + 1 < picker.sessions.len() {
                        picker.selected += 1;
                    }
                    upsert_resume_picker_message(&mut messages, picker);
                    continue;
                }
                if input.is_empty()
                    && latest_pending_approval_id.is_some()
                    && file_suggestions.is_empty()
                    && slash_suggestions.is_empty()
                {
                    if selected_approval_choice < 2 {
                        selected_approval_choice += 1;
                    }
                } else if !file_suggestions.is_empty()
                    && selected_file_suggestion + 1 < file_suggestions.len()
                {
                    selected_file_suggestion += 1;
                } else if !slash_suggestions.is_empty()
                    && selected_slash_suggestion + 1 < slash_suggestions.len()
                {
                    selected_slash_suggestion += 1;
                } else if file_suggestions.is_empty() && slash_suggestions.is_empty() {
                    scroll = scroll.saturating_add(1);
                }
            }
            KeyCode::PageUp => {
                scroll = scroll.saturating_sub(8);
            }
            KeyCode::PageDown => {
                scroll = scroll.saturating_add(8);
            }
            KeyCode::Char('1') if input.is_empty() && latest_pending_approval_id.is_some() => {
                ctrl_c_armed = false;
                let approval_id = latest_pending_approval_id.take().unwrap();
                let response = submit_approval_resolution(&client, approval_id, true).await?;
                messages.push(format_approval_resolution(response));
                status = fetch_status(&client).await?;
                if status.pending_approval_count > 0 {
                    let approvals = list_pending_approvals(&client).await?;
                    latest_pending_approval_id = latest_approval_id(&approvals);
                    latest_pending_approval = approvals.pending.first().cloned();
                    selected_approval_choice = 0;
                } else {
                    latest_pending_approval = None;
                }
            }
            KeyCode::Char('2') if input.is_empty() && latest_pending_approval_id.is_some() => {
                ctrl_c_armed = false;
                current_permission_mode = PermissionMode::FullTrust;
                let approval_id = latest_pending_approval_id.take().unwrap();
                let response = submit_approval_resolution(&client, approval_id, true).await?;
                messages.push(format!(
                    "{}\nmode: full-trust for this session",
                    format_approval_resolution(response)
                ));
                status = fetch_status(&client).await?;
                if status.pending_approval_count > 0 {
                    let approvals = list_pending_approvals(&client).await?;
                    latest_pending_approval_id = latest_approval_id(&approvals);
                    latest_pending_approval = approvals.pending.first().cloned();
                    selected_approval_choice = 0;
                } else {
                    latest_pending_approval = None;
                }
            }
            KeyCode::Char('3') if input.is_empty() && latest_pending_approval_id.is_some() => {
                ctrl_c_armed = false;
                let approval_id = latest_pending_approval_id.take().unwrap();
                let response = submit_approval_resolution(&client, approval_id, false).await?;
                messages.push(format_approval_resolution(response));
                status = fetch_status(&client).await?;
                if status.pending_approval_count > 0 {
                    let approvals = list_pending_approvals(&client).await?;
                    latest_pending_approval_id = latest_approval_id(&approvals);
                    latest_pending_approval = approvals.pending.first().cloned();
                    selected_approval_choice = 0;
                } else {
                    latest_pending_approval = None;
                }
            }
            KeyCode::Char('a') if input.is_empty() && latest_pending_approval_id.is_some() => {
                ctrl_c_armed = false;
                let approval_id = latest_pending_approval_id.take().unwrap();
                let response = submit_approval_resolution(&client, approval_id, true).await?;
                messages.push(format_approval_resolution(response));
                status = fetch_status(&client).await?;
                latest_pending_approval = None;
                selected_approval_choice = 0;
            }
            KeyCode::Char('d') if input.is_empty() && latest_pending_approval_id.is_some() => {
                ctrl_c_armed = false;
                let approval_id = latest_pending_approval_id.take().unwrap();
                let response = submit_approval_resolution(&client, approval_id, false).await?;
                messages.push(format_approval_resolution(response));
                status = fetch_status(&client).await?;
                latest_pending_approval = None;
                selected_approval_choice = 0;
            }
            KeyCode::Enter => {
                ctrl_c_armed = false;
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    input.insert('\n');
                    continue;
                }
                if let Some(picker) = resume_picker.take() {
                    if let Some(selected) = picker.sessions.get(picker.selected) {
                        active_session_id = selected.session_id.clone();
                        current_permission_mode = selected.permission_mode;
                        messages.push(format!(
                            "system: switched session to {}",
                            selected.session_id
                        ));
                    } else {
                        messages.push("system: no session selected".into());
                    }
                    continue;
                }
                if input.is_empty() && latest_pending_approval_id.is_some() {
                    let approval_id = latest_pending_approval_id.take().unwrap();
                    match selected_approval_choice {
                        0 => {
                            let response =
                                submit_approval_resolution(&client, approval_id, true).await?;
                            messages.push(format_approval_resolution(response));
                        }
                        1 => {
                            current_permission_mode = PermissionMode::FullTrust;
                            let response =
                                submit_approval_resolution(&client, approval_id, true).await?;
                            messages.push(format!(
                                "{}\nmode: full-trust for this session",
                                format_approval_resolution(response)
                            ));
                        }
                        _ => {
                            let response =
                                submit_approval_resolution(&client, approval_id, false).await?;
                            messages.push(format_approval_resolution(response));
                        }
                    }
                    status = fetch_status(&client).await?;
                    if status.pending_approval_count > 0 {
                        let approvals = list_pending_approvals(&client).await?;
                        latest_pending_approval_id = latest_approval_id(&approvals);
                        latest_pending_approval = approvals.pending.first().cloned();
                    } else {
                        latest_pending_approval_id = None;
                        latest_pending_approval = None;
                    }
                    selected_approval_choice = 0;
                    continue;
                }
                if let Some(path) = file_suggestions.get(selected_file_suggestion) {
                    complete_file_reference(&mut input, path);
                    selected_file_suggestion = 0;
                    continue;
                }
                if let Some(command) = slash_suggestions.get(selected_slash_suggestion) {
                    if input_before_cursor.trim() != command.name {
                        complete_slash_command(&mut input, command.name);
                        selected_slash_suggestion = 0;
                        continue;
                    }
                }
                let prompt = input.as_str().trim().to_string();
                input.clear();
                if let Some(mut wizard) = model_config_wizard.take() {
                    match wizard.accept(&prompt)? {
                        Some(finished) => {
                            let path = write_finished_model_config(finished)?;
                            let models = fetch_models(&client).await?;
                            messages.push(format!(
                                "system: model config\nupdated: {}\n{}",
                                path.display(),
                                format_models_status(&models)
                            ));
                            status = fetch_status(&client).await?;
                        }
                        None => {
                            messages.push(format!("system: model config\n{}", wizard.prompt()));
                            model_config_wizard = Some(wizard);
                        }
                    }
                    continue;
                }
                if prompt.is_empty() {
                    continue;
                }
                if matches!(prompt.as_str(), "exit" | "quit" | "/exit" | "/quit") {
                    break;
                }
                if prompt == "/refresh" {
                    status = fetch_status(&client).await?;
                    messages.push("system: refreshed daemon status".into());
                    continue;
                }
                if prompt == "?" || prompt == "/?" || prompt == "/help" {
                    messages.push(tui_help_text());
                    continue;
                }
                if prompt == "/skills" {
                    messages.push(format!("system: skills\n{}", discovered_skills_text()?));
                    continue;
                }
                if parse_model_slash(&prompt).is_some() {
                    let wizard = start_model_config_wizard()?;
                    let models = fetch_models(&client).await?;
                    messages.push(format!(
                        "system: model\n{}\n\n{}",
                        format_models_status(&models),
                        wizard.prompt()
                    ));
                    model_config_wizard = Some(wizard);
                    continue;
                }
                if prompt == "/update" {
                    messages.push(format_update_status(
                        env!("CARGO_PKG_VERSION"),
                        configured_latest_version(),
                    ));
                    continue;
                }
                if prompt == "/pending" {
                    let approvals = list_pending_approvals(&client).await?;
                    latest_pending_approval_id = latest_approval_id(&approvals);
                    latest_pending_approval = approvals.pending.first().cloned();
                    selected_approval_choice = 0;
                    messages.push(format!(
                        "system: pending approvals\n{}",
                        format_pending_approvals(approvals)
                    ));
                    status = fetch_status(&client).await?;
                    continue;
                }
                if prompt == "/approve" || prompt == "/deny" {
                    let approved = prompt == "/approve";
                    let approvals = list_pending_approvals(&client).await?;
                    latest_pending_approval_id = latest_approval_id(&approvals);
                    latest_pending_approval = approvals.pending.first().cloned();
                    selected_approval_choice = 0;
                    if let Some(id) = latest_pending_approval_id.as_deref() {
                        replace_input_buffer_text(
                            &mut input,
                            &format!("/{} {id}", if approved { "approve" } else { "deny" }),
                        );
                        messages.push(format!(
                            "system: pending approvals\n{}\n\nsystem: staged /{} {id} (press Enter to confirm, or edit id)",
                            format_pending_approvals(approvals),
                            if approved { "approve" } else { "deny" }
                        ));
                    } else {
                        messages.push("system: no pending approvals".into());
                    }
                    status = fetch_status(&client).await?;
                    continue;
                }
                if prompt == "/trace" {
                    let default_trace = status
                        .latest_trace_id
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default();
                    if default_trace.is_empty() {
                        messages.push(
                            "system: trace guide\nno recent trace id found. usage: /trace <trace_id> [run_id]"
                                .into(),
                        );
                    } else {
                        replace_input_buffer_text(&mut input, &format!("/trace {default_trace}"));
                        messages.push(format!(
                            "system: trace guide\nstaged /trace {default_trace} (press Enter to query, or append run id)"
                        ));
                    }
                    continue;
                }
                if let Some(mode) = parse_approval_mode_slash(&prompt) {
                    if let Some(mode) = mode {
                        current_permission_mode = mode;
                        messages.push(format!(
                            "system: approval mode set to {}",
                            format_permission_mode(mode)
                        ));
                    } else {
                        messages.push(format!(
                            "system: approval mode is {}\nusage: /approval <default|auto|full-trust>",
                            format_permission_mode(current_permission_mode)
                        ));
                    }
                    continue;
                }
                if prompt.starts_with("/approval ") {
                    messages.push(
                        "system: invalid approval mode\nusage: /approval <default|auto|full-trust>"
                            .into(),
                    );
                    continue;
                }
                if let Some((approved, approval_id)) = parse_approval_slash(&prompt) {
                    let response =
                        submit_approval_resolution(&client, approval_id, approved).await?;
                    messages.push(format_approval_resolution(response));
                    latest_pending_approval_id = None;
                    latest_pending_approval = None;
                    selected_approval_choice = 0;
                    status = fetch_status(&client).await?;
                    continue;
                }
                if prompt == "/resume" {
                    let sessions = workspace_sessions(&client, &workspace_text).await?;
                    if sessions.is_empty() {
                        messages.push("system: no sessions in current workspace".into());
                        continue;
                    }
                    let picker = ResumePickerState {
                        sessions,
                        selected: 0,
                    };
                    upsert_resume_picker_message(&mut messages, &picker);
                    resume_picker = Some(picker);
                    continue;
                }
                if prompt.starts_with("/resume ") {
                    messages.push(
                        "system: /resume in TUI no longer accepts a limit; use plain /resume"
                            .into(),
                    );
                    continue;
                }
                if prompt == "/new" {
                    let session = resolve_session(
                        &client,
                        &workspace,
                        current_permission_mode,
                        SessionResolveStrategy::CreateNew,
                    )
                    .await?;
                    active_session_id = session.session_id.clone();
                    current_permission_mode = session.permission_mode;
                    messages.push(format!(
                        "system: created and switched to {}",
                        session.session_id
                    ));
                    continue;
                }
                if let Some((trace_id, run_id)) = parse_trace_slash(&prompt) {
                    let response = query_trace(&client, trace_id, run_id).await?;
                    messages.push(format_trace_response(response));
                    continue;
                }
                let file_refs = resolve_prompt_file_references(&prompt, &file_index);
                if !file_refs.references.is_empty() {
                    messages.push(format_file_references(&file_refs.references));
                }
                if !file_refs.unresolved.is_empty() {
                    messages.push(format!(
                        "system: unresolved file references\n{}",
                        file_refs.unresolved.join("\n")
                    ));
                }
                let pending_block_start = messages.len();
                if !prompt.trim().is_empty() {
                    messages.push(format!("> {prompt}"));
                }
                current_stage = "streaming".into();
                messages.push("process: running".into());
                messages.push("Considering... (0s)".into());
                let no_file_suggestions: &[String] = &[];
                let no_slash_suggestions: &[SlashCommandHint] = &[];
                draw_tui(
                    terminal,
                    &status,
                    &workspace,
                    &current_stage,
                    latest_pending_approval_id.as_deref(),
                    latest_pending_approval.as_ref(),
                    selected_approval_choice,
                    &messages,
                    &input,
                    scroll,
                    ctrl_c_armed,
                    no_file_suggestions,
                    0,
                    no_slash_suggestions,
                    0,
                    model_config_wizard.as_ref(),
                )?;
                let run_started_at = std::time::Instant::now();
                let exec_future = submit_exec_for_session(
                    &client,
                    active_session_id.clone(),
                    file_refs.rewritten_prompt.clone(),
                );
                tokio::pin!(exec_future);
                let exec_result = loop {
                    tokio::select! {
                        result = &mut exec_future => break result,
                        _ = tokio::time::sleep(Duration::from_millis(200)) => {
                            if let Some(last) = messages.last_mut() {
                                *last = format_considering_line(run_started_at.elapsed().as_secs());
                            }
                            draw_tui(
                                terminal,
                                &status,
                                &workspace,
                                &current_stage,
                                latest_pending_approval_id.as_deref(),
                                latest_pending_approval.as_ref(),
                                selected_approval_choice,
                                &messages,
                                &input,
                                scroll,
                                ctrl_c_armed,
                                no_file_suggestions,
                                0,
                                no_slash_suggestions,
                                0,
                                model_config_wizard.as_ref(),
                            )?;
                        }
                    }
                };
                match exec_result {
                    Ok(response) => {
                        current_stage = format!("{:?}", response.completed.stage);
                        let trace_id = response.completed.trace_id.to_string();
                        let run_elapsed = run_started_at.elapsed();
                        messages.truncate(pending_block_start);
                        match query_trace(&client, trace_id, None).await {
                            Ok(trace) => {
                                let tool_outputs = load_run_tool_outputs(
                                    &client,
                                    response.completed.session_id.clone(),
                                    response.completed.run_id.clone(),
                                )
                                .await
                                .unwrap_or_default();
                                messages.push(format_friendly_turn_report(
                                    &prompt,
                                    &response,
                                    Some(&trace),
                                    &tool_outputs,
                                    run_elapsed,
                                ))
                            }
                            Err(error) => messages.push(
                                format_friendly_turn_report(
                                    &prompt,
                                    &response,
                                    None,
                                    &[],
                                    run_elapsed,
                                ) + &format!("\ntrace timeline unavailable: {error:#}"),
                            ),
                        }
                    }
                    Err(error) => {
                        current_stage = "failed".into();
                        messages.truncate(pending_block_start);
                        messages.push(format!("error: {error:#}"));
                    }
                }
                status = fetch_status(&client).await?;
                if status.pending_approval_count > 0 {
                    let approvals = list_pending_approvals(&client).await?;
                    latest_pending_approval_id = latest_approval_id(&approvals);
                    latest_pending_approval = approvals.pending.first().cloned();
                    selected_approval_choice = 0;
                } else {
                    latest_pending_approval_id = None;
                    latest_pending_approval = None;
                }
            }
            KeyCode::Char(value) => {
                input.insert(value);
                ctrl_c_armed = false;
            }
            _ => {}
        }

        if messages.len() > 40 {
            let keep_from = messages.len() - 40;
            messages.drain(0..keep_from);
        }
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn draw_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    status: &DaemonStatus,
    workspace: &Path,
    current_stage: &str,
    latest_pending_approval_id: Option<&str>,
    latest_pending_approval: Option<&PendingApprovalSummary>,
    selected_approval_choice: usize,
    messages: &[String],
    input: &InputBuffer,
    scroll: u16,
    ctrl_c_armed: bool,
    file_suggestions: &[String],
    selected_file_suggestion: usize,
    slash_suggestions: &[SlashCommandHint],
    selected_slash_suggestion: usize,
    model_config_wizard: Option<&ModelConfigWizard>,
) -> anyhow::Result<()> {
    let welcome_left = tui_welcome_left(
        status,
        workspace.display().to_string(),
        current_stage,
        latest_pending_approval_id,
    );
    let welcome_right = tui_welcome_right(messages.last().map(String::as_str));
    let message_lines = tui_message_lines(messages);
    let approval_lines = tui_approval_card_lines(latest_pending_approval, selected_approval_choice);
    let input_line = tui_input_line(
        input.as_str(),
        input.cursor(),
        model_config_wizard.map(ModelConfigWizard::input_placeholder),
    );
    let help_lines = tui_bottom_hints(
        input.before_cursor(),
        ctrl_c_armed,
        file_suggestions,
        selected_file_suggestion,
        slash_suggestions,
        selected_slash_suggestion,
        model_config_wizard,
    );

    terminal.draw(|frame| {
        let root = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(12),
                Constraint::Min(3),
                Constraint::Length(3),
                Constraint::Length(7),
            ])
            .split(frame.area());

        let welcome_block = Block::default()
            .title(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!("Unio v{}", env!("CARGO_PKG_VERSION")),
                    Style::default()
                        .fg(tui_accent())
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
            ]))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(tui_accent()));
        let welcome_inner = welcome_block.inner(root[0]);
        frame.render_widget(welcome_block, root[0]);

        let welcome_columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(welcome_inner);
        frame.render_widget(
            Paragraph::new(welcome_left)
                .alignment(ratatui::layout::Alignment::Center)
                .wrap(Wrap { trim: false }),
            welcome_columns[0],
        );
        frame.render_widget(
            Paragraph::new(welcome_right)
                .block(
                    Block::default()
                        .borders(Borders::LEFT)
                        .border_style(Style::default().fg(tui_accent())),
                )
                .wrap(Wrap { trim: false }),
            welcome_columns[1],
        );
        if let Some(approval_lines) = approval_lines {
            let body = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(9)])
                .split(root[1]);
            frame.render_widget(
                Paragraph::new(message_lines.clone())
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    )
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                body[0],
            );
            frame.render_widget(
                Paragraph::new(approval_lines)
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_style(Style::default().fg(tui_accent())),
                    )
                    .wrap(Wrap { trim: false }),
                body[1],
            );
        } else {
            frame.render_widget(
                Paragraph::new(message_lines)
                    .block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_style(Style::default().fg(Color::DarkGray)),
                    )
                    .wrap(Wrap { trim: false })
                    .scroll((scroll, 0)),
                root[1],
            );
        }
        frame.render_widget(
            Paragraph::new(input_line)
                .block(
                    Block::default()
                        .borders(Borders::TOP | Borders::BOTTOM)
                        .border_style(Style::default().fg(Color::DarkGray)),
                )
                .wrap(Wrap { trim: false }),
            root[2],
        );
        frame.render_widget(
            Paragraph::new(help_lines).wrap(Wrap { trim: false }),
            root[3],
        );
    })?;

    Ok(())
}

fn tui_accent() -> Color {
    Color::Rgb(234, 126, 83)
}

fn tui_muted() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn tui_welcome_left(
    status: &DaemonStatus,
    workspace: String,
    stage: &str,
    approval_target: Option<&str>,
) -> Vec<Line<'static>> {
    let context = status
        .latest_context_ratio
        .map(|ratio| format!("{ratio:.3}"))
        .unwrap_or_else(|| "n/a".into());
    let approval_target = approval_target.unwrap_or("none");
    vec![
        Line::from(""),
        Line::from(Span::styled(
            "Welcome back!",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled("UNIO", Style::default().fg(tui_accent()))),
        Line::from(Span::styled(
            "agent runtime",
            Style::default().fg(tui_accent()),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("{} · {}", status.models.model, status.models.provider),
            tui_muted().add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("{} · context {context}", stage),
            tui_muted(),
        )),
        Line::from(Span::styled(
            format!(
                "{} · approval {approval_target}",
                shorten_middle(&workspace, 44)
            ),
            tui_muted(),
        )),
    ]
}

fn tui_welcome_right(recent: Option<&str>) -> Vec<Line<'static>> {
    let recent = recent
        .map(|message| shorten_middle(message, 94))
        .unwrap_or_else(|| "No recent activity".into());
    vec![
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Tips for getting started",
                Style::default()
                    .fg(tui_accent())
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from("  Run /skills to inspect discovered skills."),
        Line::from("  Run /model to configure or switch the active provider."),
        Line::from("  Run /approval to view or set approval mode."),
        Line::from("  Run /pending, then a or d, to resolve pending tool requests."),
        Line::from(Span::styled(
            "  ------------------------------------------------------------",
            Style::default().fg(tui_accent()),
        )),
        Line::from(vec![
            Span::raw("  "),
            Span::styled(
                "Recent activity",
                Style::default()
                    .fg(tui_accent())
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![Span::raw("  "), Span::styled(recent, tui_muted())]),
    ]
}

fn tui_message_lines(messages: &[String]) -> Vec<Line<'static>> {
    if messages.is_empty() {
        return vec![Line::from("")];
    }
    messages
        .iter()
        .flat_map(|message| {
            let mut lines = message.lines().map(styled_message_line).collect::<Vec<_>>();
            lines.push(Line::from(""));
            lines
        })
        .collect()
}

fn tui_approval_card_lines(
    approval: Option<&PendingApprovalSummary>,
    selected_choice: usize,
) -> Option<Vec<Line<'static>>> {
    let approval = approval?;
    let args: serde_json::Value =
        serde_json::from_str(&approval.call_arguments_json).unwrap_or(serde_json::Value::Null);

    let (title, target, preview) = approval_card_content(&approval.tool_name, &args);
    let mut lines = vec![
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(target, tui_muted())),
    ];

    if let Some(preview) = preview {
        lines.push(Line::from(Span::styled(
            "----------------------------------------------",
            tui_muted(),
        )));
        for line in preview.lines().take(8) {
            lines.push(Line::from(Span::styled(
                line.to_string(),
                Style::default().fg(Color::White),
            )));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("Do you want to continue? ({})", approval.approval_id),
        Style::default().fg(Color::White),
    )));
    lines.push(approval_choice_line(0, selected_choice, "1. Yes"));
    lines.push(approval_choice_line(
        1,
        selected_choice,
        "2. Yes, allow all edits during this session",
    ));
    lines.push(approval_choice_line(2, selected_choice, "3. No"));
    Some(lines)
}

fn approval_choice_line(index: usize, selected: usize, text: &str) -> Line<'static> {
    let marker = if index == selected { "> " } else { "  " };
    let style = if index == selected {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        tui_muted()
    };
    Line::from(vec![
        Span::styled(marker.to_string(), Style::default().fg(tui_accent())),
        Span::styled(text.to_string(), style),
    ])
}

fn approval_card_content(
    tool_name: &str,
    args: &serde_json::Value,
) -> (String, String, Option<String>) {
    match tool_name {
        "write" => {
            let path = args
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown file")
                .to_string();
            let content = args
                .get("content")
                .and_then(serde_json::Value::as_str)
                .map(|text| text.to_string());
            ("Create file".into(), path, content)
        }
        "edit" => {
            let path = args
                .get("path")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown file")
                .to_string();
            let old_text = args
                .get("old_text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let new_text = args
                .get("new_text")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("");
            let preview = if old_text.is_empty() && new_text.is_empty() {
                None
            } else {
                Some(format!("- {old_text}\n+ {new_text}"))
            };
            ("Edit file".into(), path, preview)
        }
        "bash" => {
            let command = args
                .get("command")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown command")
                .to_string();
            ("Run shell command".into(), command, None)
        }
        other => (
            format!("Run tool `{other}`"),
            approval_target_from_args(args),
            None,
        ),
    }
}

fn approval_target_from_args(args: &serde_json::Value) -> String {
    args.as_object()
        .map(|object| {
            object
                .iter()
                .take(2)
                .map(|(k, v)| {
                    format!(
                        "{k}={}",
                        shorten_middle(v.to_string().trim_matches('"'), 30)
                    )
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "request".into())
}

fn styled_message_line(line: &str) -> Line<'static> {
    if let Some(prompt) = line.strip_prefix("> ") {
        return Line::from(vec![
            Span::styled("> ", Style::default().fg(tui_accent())),
            Span::styled(prompt.to_string(), Style::default().fg(Color::White)),
        ]);
    }
    if line == "process" || line.starts_with("result") {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(tui_accent()),
        ));
    }
    if line.starts_with("  - ") {
        let step = line.trim_start_matches("  - ");
        let dot_color = status_color_for_step(step);
        return status_dot_line("", step, dot_color, false);
    }
    if line.starts_with("    └ ") {
        return Line::from(Span::styled(
            line.to_string(),
            Style::default().fg(Color::Gray),
        ));
    }
    if line.starts_with("Worked for") {
        return status_dot_line("", line, Color::Yellow, false);
    }
    let style = if line.starts_with("user:") {
        Style::default().fg(Color::White)
    } else if line.starts_with("assistant:") {
        Style::default().fg(tui_accent())
    } else if line.starts_with("error:") {
        Style::default().fg(Color::Red)
    } else {
        tui_muted()
    };
    Line::from(Span::styled(line.to_string(), style))
}

fn status_dot_line(prefix: &str, text: &str, dot_color: Color, emphasize: bool) -> Line<'static> {
    let mut text_style = Style::default().fg(Color::White);
    if emphasize {
        text_style = text_style.add_modifier(Modifier::BOLD);
    }
    Line::from(vec![
        Span::styled("● ", Style::default().fg(dot_color)),
        Span::styled(prefix.to_string(), text_style),
        Span::styled(text.to_string(), text_style),
    ])
}

fn status_color_for_step(step: &str) -> Color {
    let lowered = step.to_ascii_lowercase();
    if lowered.contains("[failed]") || lowered.contains("[denied]") {
        Color::Red
    } else if lowered.contains("[approval]") {
        Color::Yellow
    } else if lowered.contains("[running]") {
        Color::Blue
    } else if lowered.contains("[completed]") {
        Color::Green
    } else {
        tui_accent()
    }
}

fn format_considering_line(seconds: u64) -> String {
    format!("Considering... ({seconds}s)")
}

#[derive(Default, Debug, Clone, Eq, PartialEq)]
struct InputBuffer {
    text: String,
    cursor: usize,
}

impl InputBuffer {
    fn as_str(&self) -> &str {
        &self.text
    }

    fn cursor(&self) -> usize {
        self.cursor
    }

    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn before_cursor(&self) -> &str {
        &self.text[..self.cursor]
    }

    fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    fn insert(&mut self, value: char) {
        self.text.insert(self.cursor, value);
        self.cursor += value.len_utf8();
    }

    #[cfg(test)]
    fn insert_str(&mut self, value: &str) {
        self.text.insert_str(self.cursor, value);
        self.cursor += value.len();
    }

    fn replace_before_cursor_from(&mut self, start: usize, value: &str) {
        self.text.replace_range(start..self.cursor, value);
        self.cursor = start + value.len();
    }

    fn backspace(&mut self) {
        let Some(previous) = self.previous_boundary() else {
            return;
        };
        self.text.drain(previous..self.cursor);
        self.cursor = previous;
    }

    fn delete(&mut self) {
        let Some(next) = self.next_boundary() else {
            return;
        };
        self.text.drain(self.cursor..next);
    }

    fn delete_previous_word(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let mut start = self.cursor;
        while let Some(previous) = previous_char_boundary(&self.text, start) {
            let ch = self.text[previous..start].chars().next().unwrap();
            if !ch.is_whitespace() {
                break;
            }
            start = previous;
        }
        while let Some(previous) = previous_char_boundary(&self.text, start) {
            let ch = self.text[previous..start].chars().next().unwrap();
            if ch.is_whitespace() {
                break;
            }
            start = previous;
        }
        self.text.drain(start..self.cursor);
        self.cursor = start;
    }

    fn move_left(&mut self) {
        if let Some(previous) = self.previous_boundary() {
            self.cursor = previous;
        }
    }

    fn move_right(&mut self) {
        if let Some(next) = self.next_boundary() {
            self.cursor = next;
        }
    }

    fn move_home(&mut self) {
        self.cursor = 0;
    }

    fn move_end(&mut self) {
        self.cursor = self.text.len();
    }

    fn previous_boundary(&self) -> Option<usize> {
        previous_char_boundary(&self.text, self.cursor)
    }

    fn next_boundary(&self) -> Option<usize> {
        self.text[self.cursor..]
            .chars()
            .next()
            .map(|value| self.cursor + value.len_utf8())
    }
}

fn previous_char_boundary(value: &str, cursor: usize) -> Option<usize> {
    value[..cursor]
        .char_indices()
        .last()
        .map(|(index, _)| index)
}

fn replace_input_buffer_text(input: &mut InputBuffer, value: &str) {
    input.clear();
    for ch in value.chars() {
        input.insert(ch);
    }
}

fn tui_input_line(input: &str, cursor: usize, placeholder: Option<String>) -> Line<'static> {
    if input.is_empty() {
        return Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::White)),
            Span::styled("|", Style::default().fg(tui_accent())),
            Span::styled(
                placeholder.unwrap_or_else(|| " Try \"explain this repo\" or /skills".into()),
                tui_muted().add_modifier(Modifier::ITALIC),
            ),
        ]);
    }
    let mut spans = vec![Span::styled("> ", Style::default().fg(Color::White))];
    let mut plain = String::new();
    let mut byte_offset = 0;
    let mut cursor_rendered = false;
    for segment in split_preserving_whitespace(input) {
        let segment_len = segment.len();
        if cursor >= byte_offset && cursor <= byte_offset + segment.len() {
            let local_cursor = cursor - byte_offset;
            let (before_cursor, after_cursor) = segment.split_at(local_cursor);
            if is_file_reference_token(before_cursor) {
                if !plain.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut plain),
                        Style::default().fg(Color::White),
                    ));
                }
                spans.push(Span::styled(
                    before_cursor.to_string(),
                    Style::default()
                        .fg(tui_accent())
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                plain.push_str(before_cursor);
                if !plain.is_empty() {
                    spans.push(Span::styled(
                        std::mem::take(&mut plain),
                        Style::default().fg(Color::White),
                    ));
                }
            }
            spans.push(Span::styled("|", Style::default().fg(tui_accent())));
            cursor_rendered = true;
            if !after_cursor.is_empty() {
                if is_file_reference_token(after_cursor)
                    || before_cursor.starts_with('@')
                    || segment.starts_with('@')
                {
                    spans.push(Span::styled(
                        after_cursor.to_string(),
                        Style::default()
                            .fg(tui_accent())
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    plain.push_str(after_cursor);
                }
            }
            byte_offset += segment_len;
            continue;
        }
        if is_file_reference_token(&segment) {
            if !plain.is_empty() {
                spans.push(Span::styled(
                    std::mem::take(&mut plain),
                    Style::default().fg(Color::White),
                ));
            }
            spans.push(Span::styled(
                segment,
                Style::default()
                    .fg(tui_accent())
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            plain.push_str(&segment);
        }
        byte_offset += segment_len;
    }
    if !plain.is_empty() {
        spans.push(Span::styled(plain, Style::default().fg(Color::White)));
    }
    if !cursor_rendered && cursor == input.len() {
        spans.push(Span::styled("|", Style::default().fg(tui_accent())));
    }
    Line::from(spans)
}

#[derive(Clone, Copy)]
struct SlashCommandHint {
    name: &'static str,
    usage: &'static str,
}

#[derive(Clone)]
struct ResumePickerState {
    sessions: Vec<SessionSummary>,
    selected: usize,
}

const SLASH_COMMAND_HINTS: &[SlashCommandHint] = &[
    SlashCommandHint {
        name: "/skills",
        usage: "list skills",
    },
    SlashCommandHint {
        name: "/model",
        usage: "configure model",
    },
    SlashCommandHint {
        name: "/approval",
        usage: "[mode] approval policy",
    },
    SlashCommandHint {
        name: "/pending",
        usage: "review approvals",
    },
    SlashCommandHint {
        name: "/resume",
        usage: "switch workspace session",
    },
    SlashCommandHint {
        name: "/new",
        usage: "create and switch session",
    },
    SlashCommandHint {
        name: "/trace",
        usage: "guided trace lookup",
    },
    SlashCommandHint {
        name: "/refresh",
        usage: "reload status",
    },
    SlashCommandHint {
        name: "/approve",
        usage: "guided approve",
    },
    SlashCommandHint {
        name: "/deny",
        usage: "guided deny",
    },
    SlashCommandHint {
        name: "/update",
        usage: "check version",
    },
    SlashCommandHint {
        name: "/help",
        usage: "show help",
    },
    SlashCommandHint {
        name: "/quit",
        usage: "exit",
    },
];

fn slash_command_suggestions(input: &str) -> Vec<SlashCommandHint> {
    if !input.starts_with('/') || input.contains(char::is_whitespace) {
        return Vec::new();
    }
    let query = input.to_lowercase();
    SLASH_COMMAND_HINTS
        .iter()
        .copied()
        .filter(|command| command.name.starts_with(&query))
        .collect()
}

fn complete_slash_command(input: &mut InputBuffer, command: &str) {
    let Some(start) = input.before_cursor().rfind('/') else {
        return;
    };
    input.replace_before_cursor_from(start, command);
    input.insert(' ');
}

fn tui_bottom_hints(
    input: &str,
    ctrl_c_armed: bool,
    file_suggestions: &[String],
    selected_file_suggestion: usize,
    slash_suggestions: &[SlashCommandHint],
    selected_slash_suggestion: usize,
    model_config_wizard: Option<&ModelConfigWizard>,
) -> Vec<Line<'static>> {
    if ctrl_c_armed {
        return vec![
            Line::from(vec![
                Span::styled("ctrl+c ", Style::default().fg(tui_accent())),
                Span::styled("again to exit Unio", Style::default().fg(Color::White)),
            ]),
            Line::from(Span::styled("press any other key to continue", tui_muted())),
        ];
    }
    if let Some(wizard) = model_config_wizard {
        return tui_model_config_hints(wizard);
    }
    if !file_suggestions.is_empty() {
        return tui_file_suggestion_hints(file_suggestions, selected_file_suggestion);
    }
    if !slash_suggestions.is_empty() {
        return tui_slash_command_hints(slash_suggestions, selected_slash_suggestion);
    }
    match input {
        "?" => tui_user_hints(),
        "@" => tui_file_hints(),
        _ => tui_default_hints(),
    }
}

fn tui_default_hints() -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled("! ", Style::default().fg(Color::White)),
            Span::styled("for bash mode", tui_muted()),
            Span::raw("          "),
            Span::styled("/ ", Style::default().fg(Color::White)),
            Span::styled("for commands", tui_muted()),
            Span::raw("          "),
            Span::styled("ctrl+c ", Style::default().fg(Color::White)),
            Span::styled("to quit", tui_muted()),
        ]),
        Line::from(vec![
            Span::styled("@ ", Style::default().fg(Color::White)),
            Span::styled("for file paths", tui_muted()),
            Span::raw("          "),
            Span::styled("a/d ", Style::default().fg(Color::White)),
            Span::styled("approve prompt: up/down select, enter confirm", tui_muted()),
        ]),
        Line::from(vec![
            Span::styled("up/down ", Style::default().fg(Color::White)),
            Span::styled("to scroll", tui_muted()),
            Span::raw("       "),
            Span::styled("left/right ", Style::default().fg(Color::White)),
            Span::styled("edit", tui_muted()),
            Span::raw("       "),
            Span::styled("enter ", Style::default().fg(Color::White)),
            Span::styled("to send", tui_muted()),
        ]),
        Line::from(vec![
            Span::styled("ctrl+w ", Style::default().fg(Color::White)),
            Span::styled("delete word", tui_muted()),
            Span::raw("     "),
            Span::styled("ctrl+u ", Style::default().fg(Color::White)),
            Span::styled("clear", tui_muted()),
            Span::raw("     "),
            Span::styled("shift+enter ", Style::default().fg(Color::White)),
            Span::styled("newline", tui_muted()),
            Span::raw("     "),
            Span::styled("? ", Style::default().fg(Color::White)),
            Span::styled("for help", tui_muted()),
        ]),
    ]
}

fn tui_model_config_hints(wizard: &ModelConfigWizard) -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled("/model ", Style::default().fg(tui_accent())),
            Span::styled(wizard.step_label(), Style::default().fg(Color::White)),
            Span::raw("  "),
            Span::styled("Enter ", Style::default().fg(Color::White)),
            Span::styled("keeps the shown default", tui_muted()),
        ]),
        Line::from(Span::styled(wizard.selection_hint(), tui_muted())),
        Line::from(vec![
            Span::styled("ctrl+u ", Style::default().fg(Color::White)),
            Span::styled("clear input", tui_muted()),
            Span::raw("     "),
            Span::styled("ctrl+c twice ", Style::default().fg(Color::White)),
            Span::styled("exit", tui_muted()),
        ]),
    ]
}

fn tui_file_suggestion_hints(
    suggestions: &[String],
    selected_file_suggestion: usize,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled("@ ", Style::default().fg(tui_accent())),
        Span::styled(
            "file matches. Up/Down to choose, Enter to insert.",
            tui_muted(),
        ),
    ])];
    for (index, suggestion) in suggestions.iter().take(5).enumerate() {
        let selected = index == selected_file_suggestion;
        lines.push(Line::from(vec![
            Span::styled(
                if selected { "> " } else { "  " },
                Style::default().fg(tui_accent()),
            ),
            Span::styled(
                suggestion.clone(),
                if selected {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    tui_muted()
                },
            ),
        ]));
    }
    lines
}

fn tui_slash_command_hints(
    suggestions: &[SlashCommandHint],
    selected_slash_suggestion: usize,
) -> Vec<Line<'static>> {
    let mut lines = vec![Line::from(vec![
        Span::styled("/ ", Style::default().fg(tui_accent())),
        Span::styled(
            "command matches. Up/Down to choose, Enter to insert.",
            tui_muted(),
        ),
    ])];
    for (index, suggestion) in suggestions.iter().take(5).enumerate() {
        let selected = index == selected_slash_suggestion;
        lines.push(Line::from(vec![
            Span::styled(
                if selected { "> " } else { "  " },
                Style::default().fg(tui_accent()),
            ),
            Span::styled(
                suggestion.name,
                if selected {
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(tui_accent())
                },
            ),
            Span::styled(format!(" {}", suggestion.usage), tui_muted()),
        ]));
    }
    lines
}

fn tui_user_hints() -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled("? ", Style::default().fg(tui_accent())),
            Span::styled(
                "opens help. Ask normally to chat with the agent.",
                tui_muted(),
            ),
        ]),
        Line::from(vec![
            Span::styled("/", Style::default().fg(Color::White)),
            Span::styled(" commands", tui_muted()),
            Span::raw("     "),
            Span::styled("@", Style::default().fg(Color::White)),
            Span::styled(" file paths", tui_muted()),
            Span::raw("     "),
            Span::styled("!", Style::default().fg(Color::White)),
            Span::styled(" bash intent", tui_muted()),
            Span::raw("     "),
            Span::styled("ctrl+c twice", Style::default().fg(Color::White)),
            Span::styled(" exit", tui_muted()),
        ]),
        Line::from(Span::styled(
            "examples: explain this repo | plan a refactor | inspect @README.md",
            tui_muted(),
        )),
    ]
}

fn tui_file_hints() -> Vec<Line<'static>> {
    vec![
        Line::from(vec![
            Span::styled("@", Style::default().fg(tui_accent())),
            Span::styled(" references files or paths in your workspace.", tui_muted()),
        ]),
        Line::from(vec![
            Span::styled("@README.md", Style::default().fg(Color::White)),
            Span::styled("  ", tui_muted()),
            Span::styled("@src/main.rs", Style::default().fg(Color::White)),
            Span::styled("  ", tui_muted()),
            Span::styled(
                "@crates/protocol/src/lib.rs",
                Style::default().fg(Color::White),
            ),
        ]),
        Line::from(Span::styled(
            "type @ at line start or after a space, Up/Down to choose, Enter to insert",
            tui_muted(),
        )),
    ]
}

fn shorten_middle(value: &str, max_chars: usize) -> String {
    let char_count = value.chars().count();
    if char_count <= max_chars {
        return value.into();
    }
    if max_chars <= 3 {
        return "...".into();
    }
    let keep = max_chars - 3;
    let front = keep / 2;
    let back = keep - front;
    let prefix = value.chars().take(front).collect::<String>();
    let suffix = value
        .chars()
        .rev()
        .take(back)
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>();
    format!("{prefix}...{suffix}")
}

fn split_preserving_whitespace(input: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut current_is_whitespace = None;
    for value in input.chars() {
        let is_whitespace = value.is_whitespace();
        if current_is_whitespace == Some(is_whitespace) || current.is_empty() {
            current.push(value);
            current_is_whitespace = Some(is_whitespace);
        } else {
            segments.push(std::mem::take(&mut current));
            current.push(value);
            current_is_whitespace = Some(is_whitespace);
        }
    }
    if !current.is_empty() {
        segments.push(current);
    }
    segments
}

fn is_file_reference_token(token: &str) -> bool {
    token.starts_with('@') && token.len() > 1
}

fn active_file_reference_query(input: &str) -> Option<(usize, String)> {
    let token_start = input
        .char_indices()
        .rev()
        .find(|(_, value)| value.is_whitespace())
        .map(|(index, value)| index + value.len_utf8())
        .unwrap_or(0);
    let token = &input[token_start..];
    let query = token.strip_prefix('@')?;
    if query.contains(char::is_whitespace) {
        return None;
    }
    Some((token_start, query.to_string()))
}

fn complete_file_reference(input: &mut InputBuffer, path: &str) {
    if let Some((token_start, _)) = active_file_reference_query(input.before_cursor()) {
        input.replace_before_cursor_from(token_start, &format!("@{path}"));
        input.insert(' ');
    }
}

#[derive(Clone)]
struct FileReferenceIndex {
    entries: Arc<RwLock<Vec<FileReferenceEntry>>>,
}

impl FileReferenceIndex {
    fn start(workspace: PathBuf) -> Self {
        let index = Self {
            entries: Arc::new(RwLock::new(Vec::new())),
        };
        let entries = Arc::clone(&index.entries);
        thread::spawn(move || loop {
            let next = scan_file_reference_paths(&workspace);
            if let Ok(mut current) = entries.write() {
                *current = next;
            }
            thread::sleep(Duration::from_secs(30));
        });
        index
    }

    #[cfg(test)]
    fn from_paths(paths: Vec<String>) -> Self {
        Self {
            entries: Arc::new(RwLock::new(
                paths.into_iter().map(FileReferenceEntry::new).collect(),
            )),
        }
    }

    fn suggestions(&self, input: &str) -> Vec<String> {
        let Some((_, query)) = active_file_reference_query(input) else {
            return Vec::new();
        };
        let query = query.replace('\\', "/").to_lowercase();
        let Ok(entries) = self.entries.try_read() else {
            return Vec::new();
        };
        let mut best = Vec::<FileReferenceCandidate<'_>>::with_capacity(50);
        for entry in entries.iter() {
            if !query.is_empty() && !entry.normalized.contains(&query) {
                continue;
            }
            let candidate = FileReferenceCandidate {
                rank: entry.rank(&query),
                entry,
            };
            insert_file_reference_candidate(&mut best, candidate);
        }
        best.into_iter()
            .map(|candidate| candidate.entry.path.clone())
            .collect()
    }

    fn resolve_reference(&self, raw_query: &str) -> Option<String> {
        let normalized = normalize_reference_query(raw_query);
        if normalized.is_empty() {
            return None;
        }

        for query in reference_query_candidates(&normalized) {
            let candidates = self.suggestions(&format!("@{query}"));
            if let Some(first) = candidates.first() {
                return Some(first.clone());
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedPromptReferences {
    references: Vec<String>,
    unresolved: Vec<String>,
    rewritten_prompt: String,
}

fn resolve_prompt_file_references(
    input: &str,
    index: &FileReferenceIndex,
) -> ResolvedPromptReferences {
    let mut rewritten = input.to_string();
    let mut references = Vec::new();
    let mut unresolved = Vec::new();

    for token in parse_file_references(input) {
        let clean = token.trim_matches(|c: char| matches!(c, ',' | '，' | '。' | ':' | '：' | ';'));
        if let Some(resolved) = index.resolve_reference(clean) {
            references.push(resolved.clone());
            rewritten = rewritten.replace(&format!("@{token}"), &format!("@{resolved}"));
        } else {
            unresolved.push(format!("@{clean}"));
        }
    }
    references.sort();
    references.dedup();
    unresolved.sort();
    unresolved.dedup();
    ResolvedPromptReferences {
        references,
        unresolved,
        rewritten_prompt: rewritten,
    }
}

fn normalize_reference_query(query: &str) -> String {
    query
        .trim()
        .trim_start_matches('@')
        .trim_start_matches('.')
        .replace('\\', "/")
        .to_lowercase()
}

fn reference_query_candidates(query: &str) -> Vec<String> {
    let mut candidates = vec![query.to_string()];
    if query.contains('-') {
        candidates.push(query.replace('-', "."));
        candidates.push(query.replace('-', ""));
    }
    if query.contains('_') {
        candidates.push(query.replace('_', "."));
    }
    let alnum = query
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '/' || ch == '.' {
                ch
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    if let Some(longest) = alnum.iter().max_by_key(|part| part.len()) {
        candidates.push(longest.clone());
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

#[derive(Clone)]
struct FileReferenceEntry {
    path: String,
    normalized: String,
    basename: String,
}

impl FileReferenceEntry {
    fn new(path: String) -> Self {
        let normalized = path.replace('\\', "/").to_lowercase();
        let basename = normalized
            .rsplit('/')
            .next()
            .unwrap_or(&normalized)
            .to_string();
        Self {
            path,
            normalized,
            basename,
        }
    }

    fn rank(&self, query: &str) -> FileReferenceRank<'_> {
        let tier = if query.is_empty() {
            0
        } else if self.basename == query {
            0
        } else if self.normalized == query {
            1
        } else if self.basename.starts_with(query) {
            2
        } else if self.normalized.starts_with(query) {
            3
        } else if self.basename.contains(query) {
            4
        } else {
            5
        };
        FileReferenceRank {
            tier,
            len: self.normalized.len(),
            normalized: &self.normalized,
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
struct FileReferenceRank<'a> {
    tier: u8,
    len: usize,
    normalized: &'a str,
}

struct FileReferenceCandidate<'a> {
    rank: FileReferenceRank<'a>,
    entry: &'a FileReferenceEntry,
}

fn insert_file_reference_candidate<'a>(
    best: &mut Vec<FileReferenceCandidate<'a>>,
    candidate: FileReferenceCandidate<'a>,
) {
    let position = best
        .binary_search_by(|existing| existing.rank.cmp(&candidate.rank))
        .unwrap_or_else(|position| position);
    if position < 50 {
        best.insert(position, candidate);
        if best.len() > 50 {
            best.pop();
        }
    }
}

#[derive(Clone, Debug)]
struct IgnoreRule {
    pattern: String,
    directory_only: bool,
}

fn scan_file_reference_paths(workspace: &Path) -> Vec<FileReferenceEntry> {
    let mut paths = Vec::new();
    collect_file_reference_paths(workspace, workspace, Vec::new(), &mut paths);
    paths.sort();
    paths.dedup();
    paths.into_iter().map(FileReferenceEntry::new).collect()
}

fn collect_file_reference_paths(
    root: &Path,
    current: &Path,
    inherited_rules: Vec<IgnoreRule>,
    paths: &mut Vec<String>,
) {
    let mut rules = inherited_rules;
    rules.extend(load_ignore_rules(current));
    let entries = match std::fs::read_dir(current) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let is_dir = file_type.is_dir();
        if is_ignored_reference_path(&name, is_dir, &rules) {
            continue;
        }
        if is_dir {
            collect_file_reference_paths(root, &path, rules.clone(), paths);
            continue;
        }
        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };
        paths.push(relative.to_string_lossy().replace('\\', "/"));
    }
}

fn load_ignore_rules(directory: &Path) -> Vec<IgnoreRule> {
    [".gitignore", ".npmignore"]
        .into_iter()
        .flat_map(|name| read_ignore_rules(&directory.join(name)))
        .collect()
}

fn read_ignore_rules(path: &Path) -> Vec<IgnoreRule> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with('!') {
                return None;
            }
            let pattern = line
                .trim_start_matches('/')
                .trim_end_matches('/')
                .to_string();
            if pattern.is_empty() {
                return None;
            }
            Some(IgnoreRule {
                pattern,
                directory_only: line.ends_with('/'),
            })
        })
        .collect()
}

fn is_ignored_reference_path(name: &str, is_dir: bool, rules: &[IgnoreRule]) -> bool {
    if matches!(name, ".gitignore" | ".npmignore") {
        return true;
    }
    if is_dir && (name.starts_with('.') || is_heavy_generated_dir(name)) {
        return true;
    }
    rules.iter().any(|rule| {
        if rule.directory_only && !is_dir {
            return false;
        }
        name == rule.pattern
            || name.ends_with(&format!(".{}", rule.pattern.trim_start_matches("*.")))
            || rule.pattern.split('/').any(|part| part == name)
    })
}

fn is_heavy_generated_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules" | "target" | "dist" | "build" | ".next" | ".cache" | "coverage" | ".unio"
    )
}

fn tui_help_text() -> String {
    [
        "system: help",
        "Prompt prefixes:",
        "/               show slash command hints",
        "?               show user help hints",
        "@path           reference a workspace file path in your prompt",
        "!               mark bash/process intent in natural language",
        "",
        "Slash commands:",
        "/skills         list discovered skills",
        "/model          show active model provider",
        "/approval [mode] show or set approval mode",
        "/pending        show pending approvals",
        "/approve [id]   guided or direct approve",
        "/deny [id]      guided or direct deny",
        "/resume         pick and switch workspace session",
        "/new            create and switch to a new session",
        "/trace [id] [run_id] guided or direct trace query",
        "/refresh        reload daemon status",
        "/update         check configured latest version",
        "/quit           exit Unio",
        "",
        "Keys:",
        "up/down + enter approve prompt selection when input is empty",
        "1 / 2 / 3       direct approve prompt shortcuts",
        "a / d           legacy quick approve/deny for latest pending request",
        "left/right      move inside the input line",
        "home/end        move to start or end",
        "ctrl+w          delete previous word",
        "ctrl+u          clear the prompt",
        "shift+enter     insert newline",
        "up/down         scroll history or choose suggestions",
        "ctrl+c twice    exit Unio",
    ]
    .join("\n")
}

fn parse_file_references(input: &str) -> Vec<String> {
    input
        .split_whitespace()
        .filter_map(|token| token.strip_prefix('@'))
        .filter(|reference| !reference.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn format_file_references(references: &[String]) -> String {
    format!("system: file references\n{}", references.join("\n"))
}

fn print_slash_help() {
    println!("/skills    list discovered skills");
    println!("/model     configure and select model");
    println!("/approval [default|auto|full-trust] show or set approval mode");
    println!("/pending   show pending approvals");
    println!("/resume switch session in TUI; CLI subcommand still prints transcript");
    println!("/new create and switch to a fresh session");
    println!("/trace [id] [run_id] guided or direct trace query");
    println!("/approve [id] guided or direct approve");
    println!("/deny [id]    guided or direct deny");
    println!("/update    check for updates");
    println!("?          show this help");
}

fn parse_trace_slash(input: &str) -> Option<(String, Option<String>)> {
    let rest = input.strip_prefix("/trace ")?;
    let mut parts = rest.split_whitespace();
    let trace_id = parts.next()?.to_string();
    let run_id = parts.next().map(ToOwned::to_owned);
    Some((trace_id, run_id))
}

fn parse_resume_slash(input: &str) -> Option<Option<usize>> {
    if input == "/resume" {
        return Some(None);
    }
    let rest = input.strip_prefix("/resume ")?;
    let limit = rest.trim().parse::<usize>().ok()?;
    Some(Some(limit))
}

fn parse_approval_slash(input: &str) -> Option<(bool, String)> {
    input
        .strip_prefix("/approve ")
        .map(|id| (true, id.trim().to_string()))
        .or_else(|| {
            input
                .strip_prefix("/deny ")
                .map(|id| (false, id.trim().to_string()))
        })
        .filter(|(_, id)| !id.is_empty())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelSwitch {
    provider: String,
    model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelConfigWizard {
    step: ModelConfigStep,
    provider: String,
    model: String,
    base_url: String,
    api_key: String,
    api_key_update: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelConfigStep {
    Provider,
    Model,
    BaseUrl,
    ApiKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FinishedModelConfig {
    provider: String,
    model: String,
    base_url: String,
    api_key_update: Option<String>,
}

impl ModelConfigWizard {
    fn from_config(config: &toml::Value) -> Self {
        let existing = config.get("model");
        let provider = existing
            .and_then(|model| model.get("provider"))
            .and_then(toml::Value::as_str)
            .unwrap_or("mock")
            .to_string();
        let model = existing
            .and_then(|model| model.get("model"))
            .and_then(toml::Value::as_str)
            .unwrap_or_else(|| default_model_for_provider(&provider))
            .to_string();
        let base_url = existing
            .and_then(|model| model.get("base_url"))
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_string();
        let api_key = existing
            .and_then(|model| model.get("api_key"))
            .and_then(toml::Value::as_str)
            .unwrap_or("")
            .to_string();
        Self {
            step: ModelConfigStep::Provider,
            provider,
            model,
            base_url,
            api_key,
            api_key_update: None,
        }
    }

    fn prompt(&self) -> String {
        match self.step {
            ModelConfigStep::Provider => format!(
                "Model setup 1/4 - provider [{}] (openai-compatible, anthropic, mock)",
                self.provider
            ),
            ModelConfigStep::Model => format!("Model setup 2/4 - model [{}]", self.model),
            ModelConfigStep::BaseUrl if self.base_url.is_empty() => {
                "Model setup 3/4 - base_url [empty]".into()
            }
            ModelConfigStep::BaseUrl => {
                format!("Model setup 3/4 - base_url [{}]", self.base_url)
            }
            ModelConfigStep::ApiKey if self.api_key.is_empty() => {
                "Model setup 4/4 - api_key [empty]".into()
            }
            ModelConfigStep::ApiKey => {
                "Model setup 4/4 - api_key [keep existing; enter '-' to clear]".into()
            }
        }
    }

    fn input_placeholder(&self) -> String {
        format!(" {}", self.prompt())
    }

    fn step_label(&self) -> &'static str {
        match self.step {
            ModelConfigStep::Provider => "choose provider",
            ModelConfigStep::Model => "choose model",
            ModelConfigStep::BaseUrl => "set base URL",
            ModelConfigStep::ApiKey => "set API key",
        }
    }

    fn selection_hint(&self) -> &'static str {
        match self.step {
            ModelConfigStep::Provider => {
                "Type openai-compatible, anthropic, or mock. Enter keeps current."
            }
            ModelConfigStep::Model => "Type a model name. Enter keeps current.",
            ModelConfigStep::BaseUrl => "Type a base URL, or Enter to keep/leave empty.",
            ModelConfigStep::ApiKey => "Type a key, Enter keeps existing, '-' clears it.",
        }
    }

    fn accept(&mut self, input: &str) -> anyhow::Result<Option<FinishedModelConfig>> {
        let value = input.trim();
        match self.step {
            ModelConfigStep::Provider => {
                let provider = if value.is_empty() {
                    self.provider.clone()
                } else {
                    value.to_string()
                };
                if !is_supported_model_provider(&provider) {
                    anyhow::bail!(
                        "unsupported provider: {provider}; use openai, openai-compatible, anthropic, or mock"
                    );
                }
                self.provider = provider;
                if self.model == "mock" {
                    self.model = default_model_for_provider(&self.provider).into();
                }
                self.step = ModelConfigStep::Model;
                Ok(None)
            }
            ModelConfigStep::Model => {
                self.model = if value.is_empty() {
                    self.model.clone()
                } else {
                    value.to_string()
                };
                self.step = ModelConfigStep::BaseUrl;
                Ok(None)
            }
            ModelConfigStep::BaseUrl => {
                self.base_url = if value.is_empty() {
                    self.base_url.clone()
                } else {
                    value.to_string()
                };
                self.step = ModelConfigStep::ApiKey;
                Ok(None)
            }
            ModelConfigStep::ApiKey => {
                self.api_key_update = if value == "-" {
                    Some(String::new())
                } else if value.is_empty() {
                    None
                } else {
                    Some(value.to_string())
                };
                Ok(Some(FinishedModelConfig {
                    provider: self.provider.clone(),
                    model: self.model.clone(),
                    base_url: self.base_url.clone(),
                    api_key_update: self.api_key_update.clone(),
                }))
            }
        }
    }
}

impl FinishedModelConfig {
    fn apply_to_config(&self, config: &mut toml::Value) -> anyhow::Result<()> {
        apply_model_switch_to_config(
            config,
            &ModelSwitch {
                provider: self.provider.clone(),
                model: Some(self.model.clone()),
            },
        )?;
        set_optional_model_config_value(config, "base_url", self.base_url.clone())?;
        if let Some(api_key) = &self.api_key_update {
            set_optional_model_config_value(config, "api_key", api_key.clone())?;
        }
        Ok(())
    }
}

fn parse_model_slash(input: &str) -> Option<()> {
    (input.trim() == "/model").then_some(())
}

fn parse_approval_mode_slash(input: &str) -> Option<Option<PermissionMode>> {
    let trimmed = input.trim();
    if trimmed == "/approval" {
        return Some(None);
    }
    let mode = trimmed.strip_prefix("/approval ")?;
    let mode = match mode.trim() {
        "default" => PermissionMode::Default,
        "auto" => PermissionMode::Auto,
        "full-trust" => PermissionMode::FullTrust,
        _ => return None,
    };
    Some(Some(mode))
}

fn format_permission_mode(mode: PermissionMode) -> &'static str {
    match mode {
        PermissionMode::Default => "default",
        PermissionMode::Auto => "auto",
        PermissionMode::FullTrust => "full-trust",
    }
}

fn is_supported_model_provider(provider: &str) -> bool {
    matches!(
        provider,
        "openai" | "openai-compatible" | "anthropic" | "mock"
    )
}

#[derive(Debug, Clone, Copy)]
struct ExecOutputMode {
    quiet: bool,
}

async fn run_exec(
    prompt: String,
    permission_mode: PermissionMode,
    output: ExecOutputMode,
) -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    let response = submit_exec(&client, prompt, permission_mode).await?;
    print_exec_response(&response, output);
    Ok(())
}

async fn submit_exec(
    client: &DaemonClient,
    prompt: String,
    permission_mode: PermissionMode,
) -> anyhow::Result<ExecTurnResponse> {
    let workspace = std::env::current_dir()?;
    let session = resolve_session(
        &client,
        &workspace,
        permission_mode,
        SessionResolveStrategy::CreateNew,
    )
    .await?;
    submit_exec_for_session(client, session.session_id, prompt).await
}

async fn submit_exec_for_session(
    client: &DaemonClient,
    session_id: SessionId,
    prompt: String,
) -> anyhow::Result<ExecTurnResponse> {
    Ok(client
        .post(format!("{}/exec", client.base_url))
        .json(&ExecTurnRequest { session_id, prompt })
        .send()
        .await
        .context("failed to submit exec request to daemon")?
        .error_for_status()
        .context("daemon rejected exec request")?
        .json::<ExecTurnResponse>()
        .await
        .context("failed to decode exec response")?)
}

fn print_exec_response(response: &ExecTurnResponse, output: ExecOutputMode) {
    println!("{}", response.completed.final_text);
    if !output.quiet {
        println!("{}", format_exec_summary(response));
    }
    if response.completed.stage == RunStage::WaitingApproval {
        println!(
            "waiting_approval: use `/pending` or `unio approvals` to review pending approvals"
        );
    }
}

fn format_exec_summary(response: &ExecTurnResponse) -> String {
    let mut lines = vec![
        format!(
            "Done {:?}  {} / {}",
            response.completed.stage, response.completed.provider, response.completed.model
        ),
        format!(
            "Run {}  Trace {}",
            response.completed.run_id, response.completed.trace_id
        ),
        format!(
            "Tokens input={} output={} context_ratio={:.3}",
            response.completed.input_tokens,
            response.completed.output_tokens,
            response.completed.context_ratio
        ),
    ];
    let event_lines = response
        .completed
        .events
        .iter()
        .map(|event| friendly_event_line(event, ""))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if !event_lines.is_empty() {
        lines.push("Activity".into());
        lines.extend(event_lines);
    }
    lines.join("\n")
}

fn friendly_event_line(kind: &str, message: &str) -> String {
    let label = friendly_event_label(kind);
    if label.is_empty() {
        return String::new();
    }
    if message.trim().is_empty() {
        label.into()
    } else {
        format!("{label} {}", friendly_event_message(message))
    }
}

fn friendly_event_label(kind: &str) -> &'static str {
    match kind {
        "tool.started" => "Tool started",
        "tool.completed" => "Tool completed",
        "tool.approval_required" | "approval.requested" => "Approval required",
        "tool.denied" => "Tool denied",
        "tool.failed" => "Tool failed",
        "skill.started" => "Skill started",
        "skill.completed" => "Skill completed",
        "skill.waiting_approval" => "Skill awaiting approval",
        "skill.denied" => "Skill denied",
        "skill.failed" => "Skill failed",
        "approval.resolved" => "Approval resolved",
        "context.budget" => "Context budget",
        "context.compaction_required" => "Context compaction required",
        "planning.completed" => "Planning completed",
        "sub_agent.completed" => "Sub-agent completed",
        "run.completed" | "root_agent.completed" => "",
        _ if kind.contains("approval") => "Approval",
        _ if kind.contains("skill") => "Skill",
        _ if kind.contains("tool") => "Tool",
        _ if kind.contains("context") => "Context",
        _ => "",
    }
}

fn friendly_event_message(message: &str) -> String {
    message
        .trim()
        .replace("skill-tool:", "skill ")
        .replace("skill-tool", "skill")
}

async fn run_resume(session_id: Option<String>, limit: Option<usize>) -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    let sessions = client
        .get(format!("{}/sessions", client.base_url))
        .send()
        .await
        .context("failed to request session list from daemon")?
        .error_for_status()
        .context("daemon rejected session list request")?
        .json::<Vec<SessionSummary>>()
        .await
        .context("failed to decode session list response")?;
    if let Some(session_id) = session_id {
        let session = sessions
            .iter()
            .find(|session| session.session_id.as_str() == session_id)
            .ok_or_else(|| anyhow::anyhow!("session not found: {session_id}"))?;
        print_transcript(&client, session.session_id.clone(), limit).await?;
    } else if let Some(session) = sessions.first() {
        print_transcript(&client, session.session_id.clone(), limit).await?;
    } else {
        println!("no sessions");
    }
    Ok(())
}

async fn print_transcript(
    client: &DaemonClient,
    session_id: unio_core::SessionId,
    limit: Option<usize>,
) -> anyhow::Result<()> {
    let response = load_transcript(client, session_id, limit).await?;
    println!("{}", format_transcript_response(response));
    Ok(())
}

async fn load_transcript(
    client: &DaemonClient,
    session_id: unio_core::SessionId,
    limit: Option<usize>,
) -> anyhow::Result<LoadTranscriptResponse> {
    let response = client
        .post(format!("{}/sessions/transcript", client.base_url))
        .json(&LoadTranscriptRequest { session_id, limit })
        .send()
        .await
        .context("failed to request session transcript from daemon")?
        .error_for_status()
        .context("daemon rejected session transcript request")?
        .json::<LoadTranscriptResponse>()
        .await
        .context("failed to decode session transcript response")?;
    Ok(response)
}

fn format_transcript_response(response: LoadTranscriptResponse) -> String {
    let mut lines = vec![format!(
        "{}\t{}\t{}",
        response.session.session_id, response.session.title, response.session.workspace_root
    )];
    if response.messages.is_empty() {
        lines.push("no transcript messages".into());
        return lines.join("\n");
    }
    for message in response
        .messages
        .iter()
        .rev()
        .take(12)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        lines.push(format_transcript_message(message));
    }
    lines.join("\n")
}

fn format_resume_picker(state: &ResumePickerState) -> String {
    let mut lines = vec![
        "system: resume session".to_string(),
        "Use Up/Down to select, Enter to switch, Esc to cancel.".to_string(),
    ];
    for (index, session) in state.sessions.iter().enumerate() {
        let marker = if index == state.selected { ">" } else { " " };
        lines.push(format!(
            "{marker} {}  {}",
            session.session_id,
            session.updated_at.to_rfc3339()
        ));
    }
    lines.join("\n")
}

fn upsert_resume_picker_message(messages: &mut Vec<String>, state: &ResumePickerState) {
    let picker = format_resume_picker(state);
    if let Some(last) = messages.last_mut() {
        if last.starts_with(
            "system: resume session\nUse Up/Down to select, Enter to switch, Esc to cancel.",
        ) {
            *last = picker;
            return;
        }
    }
    messages.push(picker);
}

fn format_transcript_message(message: &TranscriptMessage) -> String {
    match message {
        TranscriptMessage::User { content, .. } => format!("user: {content}"),
        TranscriptMessage::Assistant { content, .. } => format!("assistant: {content}"),
        TranscriptMessage::Tool {
            tool_name, content, ..
        } => format!("tool[{tool_name}]: {content}"),
    }
}

async fn run_sessions() -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    let sessions = client
        .get(format!("{}/sessions", client.base_url))
        .send()
        .await
        .context("failed to request session list from daemon")?
        .error_for_status()
        .context("daemon rejected session list request")?
        .json::<Vec<SessionSummary>>()
        .await
        .context("failed to decode session list response")?;
    for session in sessions {
        println!(
            "{}\t{}\t{}",
            session.session_id, session.title, session.workspace_root
        );
    }
    Ok(())
}

async fn run_models() -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    let status = fetch_models(&client).await?;
    println!("{}", format_models_status(&status));
    Ok(())
}

async fn run_model_config() -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    let status = fetch_models(&client).await?;
    println!("{}", format_models_status(&status));
    let path = write_prompted_model_config()?;
    println!("updated: {}", path.display());
    run_models().await
}

async fn run_guided_approval_resolution(approved: bool) -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    let approvals = list_pending_approvals(&client).await?;
    if approvals.pending.is_empty() {
        println!("no pending approvals");
        return Ok(());
    }
    println!("{}", format_pending_approvals(approvals.clone()));
    let default_id = approvals.pending[0].approval_id.to_string();
    let selected = prompt_with_default("approval_id", &default_id)?;
    resolve_approval(&client, selected, approved).await
}

async fn run_guided_trace_query() -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    let status = fetch_status(&client).await?;
    let default_trace = status
        .latest_trace_id
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default();
    let trace_id = prompt_with_default("trace_id", &default_trace)?;
    if trace_id.trim().is_empty() {
        anyhow::bail!("trace_id is required");
    }
    let run_id = prompt_with_default("run_id (optional)", "")?;
    let run_id = if run_id.trim().is_empty() {
        None
    } else {
        Some(run_id)
    };
    run_trace(trace_id, run_id).await
}

async fn run_approval_mode_config(mode: Option<PermissionMode>) -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    let workspace = std::env::current_dir()?;
    if let Some(mode) = mode {
        let session = resolve_session(
            &client,
            &workspace,
            mode,
            SessionResolveStrategy::ReuseWorkspaceLatest,
        )
        .await?;
        println!(
            "approval_mode: {}",
            format_permission_mode(session.permission_mode)
        );
    } else {
        let current_mode = fetch_workspace_permission_mode(&client, &workspace)
            .await?
            .unwrap_or_default();
        println!("approval_mode: {}", format_permission_mode(current_mode));
        println!("usage: /approval <default|auto|full-trust>");
    }
    Ok(())
}

async fn run_new_session() -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    let workspace = std::env::current_dir()?;
    let session = resolve_session(
        &client,
        &workspace,
        PermissionMode::Default,
        SessionResolveStrategy::CreateNew,
    )
    .await?;
    println!(
        "new_session: {}\t{}\t{}",
        session.session_id, session.title, session.workspace_root
    );
    Ok(())
}

async fn fetch_workspace_permission_mode(
    client: &DaemonClient,
    workspace: &Path,
) -> anyhow::Result<Option<PermissionMode>> {
    let workspace = workspace.to_string_lossy().to_string();
    let sessions = workspace_sessions(client, &workspace).await?;
    Ok(sessions
        .into_iter()
        .next()
        .map(|session| session.permission_mode))
}

async fn workspace_sessions(
    client: &DaemonClient,
    workspace_root: &str,
) -> anyhow::Result<Vec<SessionSummary>> {
    let sessions = client
        .get(format!("{}/sessions", client.base_url))
        .send()
        .await
        .context("failed to request session list from daemon")?
        .error_for_status()
        .context("daemon rejected session list request")?
        .json::<Vec<SessionSummary>>()
        .await
        .context("failed to decode session list response")?;
    Ok(sessions
        .into_iter()
        .filter(|session| session.workspace_root == workspace_root)
        .collect())
}

async fn fetch_models(client: &DaemonClient) -> anyhow::Result<ModelsStatus> {
    Ok(client
        .get(format!("{}/models", client.base_url))
        .send()
        .await
        .context("failed to request model status from daemon")?
        .error_for_status()?
        .json::<ModelsStatus>()
        .await
        .context("failed to decode daemon model status")?)
}

fn format_models_status(status: &ModelsStatus) -> String {
    [
        format!("provider: {}", status.provider),
        format!("model: {}", status.model),
        format!("fallback_to_mock: {}", status.fallback_to_mock),
    ]
    .join("\n")
}

fn start_model_config_wizard() -> anyhow::Result<ModelConfigWizard> {
    let paths = UserPaths::current()?;
    let path = paths.root.join("config.toml");
    let config = read_unio_config(&path)?;
    Ok(ModelConfigWizard::from_config(&config))
}

fn write_finished_model_config(config_update: FinishedModelConfig) -> anyhow::Result<PathBuf> {
    let paths = UserPaths::current()?;
    paths.ensure()?;
    let path = paths.root.join("config.toml");
    let mut config = read_unio_config(&path)?;
    config_update.apply_to_config(&mut config)?;
    std::fs::write(&path, toml::to_string_pretty(&config)?).map_err(|error| {
        if error.kind() == io::ErrorKind::PermissionDenied {
            anyhow::anyhow!(
                "permission denied writing {}. Configure model via env vars or run with file write permission.",
                path.display()
            )
        } else {
            error.into()
        }
    })?;
    Ok(path)
}

fn write_prompted_model_config() -> anyhow::Result<PathBuf> {
    let paths = UserPaths::current()?;
    paths.ensure()?;
    let path = paths.root.join("config.toml");
    let mut config = read_unio_config(&path)?;
    let existing = config.get("model");

    let current_provider = existing
        .and_then(|model| model.get("provider"))
        .and_then(toml::Value::as_str)
        .unwrap_or("mock")
        .to_string();
    let current_model = existing
        .and_then(|model| model.get("model"))
        .and_then(toml::Value::as_str)
        .map(ToOwned::to_owned);
    let current_base_url = existing
        .and_then(|model| model.get("base_url"))
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_string();
    let current_api_key = existing
        .and_then(|model| model.get("api_key"))
        .and_then(toml::Value::as_str)
        .unwrap_or("")
        .to_string();

    let provider = prompt_with_default("provider", &current_provider)?;
    if !is_supported_model_provider(&provider) {
        anyhow::bail!(
            "unsupported provider: {provider}; use openai, openai-compatible, anthropic, or mock"
        );
    }

    let default_model = default_model_for_provider(&provider);
    let current_model = current_model.as_deref().unwrap_or(default_model);
    let model = prompt_with_default("model", current_model)?;
    let model_switch = ModelSwitch {
        provider,
        model: Some(model),
    };
    apply_model_switch_to_config(&mut config, &model_switch)?;

    let base_url = prompt_with_default("base_url", &current_base_url)?;
    set_optional_model_config_value(&mut config, "base_url", base_url)?;

    let api_key_prompt = if current_api_key.is_empty() {
        "api_key"
    } else {
        "api_key (Enter keeps existing, '-' clears)"
    };
    let api_key = prompt_with_default(api_key_prompt, "")?;
    if api_key == "-" {
        set_optional_model_config_value(&mut config, "api_key", String::new())?;
    } else if !api_key.is_empty() {
        set_optional_model_config_value(&mut config, "api_key", api_key)?;
    }

    std::fs::write(&path, toml::to_string_pretty(&config)?).map_err(|error| {
        if error.kind() == io::ErrorKind::PermissionDenied {
            anyhow::anyhow!(
                "permission denied writing {}. Configure model via env vars or run with file write permission.",
                path.display()
            )
        } else {
            error.into()
        }
    })?;
    Ok(path)
}

fn read_unio_config(path: &Path) -> anyhow::Result<toml::Value> {
    if !path.exists() {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
}

fn apply_model_switch_to_config(
    config: &mut toml::Value,
    model_switch: &ModelSwitch,
) -> anyhow::Result<()> {
    let model_table = model_config_table_mut(config)?;
    model_table.insert(
        "provider".into(),
        toml::Value::String(model_switch.provider.clone()),
    );
    model_table.insert(
        "model".into(),
        toml::Value::String(
            model_switch
                .model
                .clone()
                .unwrap_or_else(|| default_model_for_provider(&model_switch.provider).into()),
        ),
    );
    Ok(())
}

fn set_optional_model_config_value(
    config: &mut toml::Value,
    key: &str,
    value: String,
) -> anyhow::Result<()> {
    let model_table = model_config_table_mut(config)?;
    if value.trim().is_empty() {
        model_table.remove(key);
    } else {
        model_table.insert(key.into(), toml::Value::String(value));
    }
    Ok(())
}

fn model_config_table_mut(
    config: &mut toml::Value,
) -> anyhow::Result<&mut toml::map::Map<String, toml::Value>> {
    if !config.is_table() {
        *config = toml::Value::Table(toml::map::Map::new());
    }
    let root = config
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("config root must be a TOML table"))?;
    let model = root
        .entry("model")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
    if !model.is_table() {
        *model = toml::Value::Table(toml::map::Map::new());
    }
    model
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("model config must be a TOML table"))
}

fn default_model_for_provider(provider: &str) -> &'static str {
    match provider {
        "openai" | "openai-compatible" => "gpt-4o-mini",
        "anthropic" => "claude-3-5-sonnet-latest",
        _ => "mock",
    }
}

fn prompt_with_default(label: &str, default: &str) -> anyhow::Result<String> {
    if default.is_empty() {
        print!("{label}: ");
    } else {
        print!("{label} [{default}]: ");
    }
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let value = input.trim().to_string();
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value)
    }
}

async fn run_status() -> anyhow::Result<()> {
    let client = daemon_client(false).await?;
    let workspace = std::env::current_dir()?;
    let status = fetch_status(&client).await?;
    println!(
        "{}",
        format_daemon_status(&status, workspace.display().to_string())
    );
    Ok(())
}

fn format_daemon_status(status: &DaemonStatus, workspace: String) -> String {
    let mut lines = vec![
        format!("pid: {}", status.pid),
        format!("http: {}", status.http_url),
        format!("started_at: {}", status.started_at),
        format!("workspace: {workspace}"),
        format!("sessions: {}", status.session_count),
        format!("pending_approvals: {}", status.pending_approval_count),
    ];
    if let Some(session_id) = &status.latest_session_id {
        lines.push(format!("latest_session: {session_id}"));
    }
    if let Some(trace_id) = &status.latest_trace_id {
        lines.push(format!("latest_trace: {trace_id}"));
    }
    if let Some(context_ratio) = status.latest_context_ratio {
        lines.push(format!("latest_context_ratio: {context_ratio:.3}"));
    }
    lines.push(format!("provider: {}", status.models.provider));
    lines.push(format!("model: {}", status.models.model));
    lines.push(format!(
        "fallback_to_mock: {}",
        status.models.fallback_to_mock
    ));
    lines.join("\n")
}

async fn fetch_status(client: &DaemonClient) -> anyhow::Result<DaemonStatus> {
    Ok(client
        .get(format!("{}/status", client.base_url))
        .send()
        .await
        .context("failed to request daemon status")?
        .error_for_status()
        .context("daemon rejected status request")?
        .json::<DaemonStatus>()
        .await
        .context("failed to decode daemon status")?)
}

fn run_update() -> anyhow::Result<()> {
    println!(
        "{}",
        format_update_status(env!("CARGO_PKG_VERSION"), configured_latest_version())
    );
    Ok(())
}

fn format_update_status(current: &str, latest: Option<String>) -> String {
    let mut lines = vec![format!("current_version: {current}")];
    match latest {
        Some(version) => {
            lines.push(format!("latest_version: {version}"));
            lines.push(format!("update_available: {}", version != current));
        }
        None => {
            lines.push("latest_version: unknown".into());
            lines.push("update_available: unknown".into());
            lines.push("source: set UNIO_LATEST_VERSION to enable local update checks".into());
        }
    }
    lines.join("\n")
}

fn configured_latest_version() -> Option<String> {
    std::env::var("UNIO_LATEST_VERSION")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn run_trace(trace_id: String, run_id: Option<String>) -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    let response = query_trace(&client, trace_id, run_id).await?;
    println!("{}", format_trace_response(response));
    Ok(())
}

async fn query_trace(
    client: &DaemonClient,
    trace_id: String,
    run_id: Option<String>,
) -> anyhow::Result<TraceLookupResponse> {
    Ok(client
        .post(format!("{}/traces/query", client.base_url))
        .json(&TraceLookupRequest {
            trace_id: unio_core::TraceId::from_string(trace_id),
            run_id: run_id.map(unio_core::RunId::from_string),
        })
        .send()
        .await
        .context("failed to submit trace lookup request to daemon")?
        .error_for_status()
        .context("daemon rejected trace lookup request")?
        .json::<TraceLookupResponse>()
        .await
        .context("failed to decode trace lookup response")?)
}

fn format_trace_response(response: TraceLookupResponse) -> String {
    let mut lines = vec![format!("trace: {}", response.trace_id)];
    if response.events.is_empty() {
        lines.push("no trace events".into());
        return lines.join("\n");
    }
    let mut current_run = None;
    for event in response.events {
        let run_id = event.run_id.to_string();
        if current_run.as_deref() != Some(run_id.as_str()) {
            current_run = Some(run_id.clone());
            lines.push(format!("run: {run_id}"));
        }
        lines.push(format!("  {}\t{}", event.recorded_at, event.kind));
        if let Some(usage) = event.token_usage {
            lines.push(format!(
                "  tokens: input={} output={} context_ratio={:.3}",
                usage.input_tokens, usage.output_tokens, usage.context_ratio
            ));
        }
        if !event.message.is_empty() {
            lines.push(format!("  {}", event.message));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
fn format_trace_timeline(response: &TraceLookupResponse) -> String {
    if response.events.is_empty() {
        return format!("timeline: {} has no events", response.trace_id);
    }
    let mut lines = vec![format!("Run timeline {}", response.trace_id)];
    for event in response
        .events
        .iter()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
    {
        let mut line = friendly_event_line(&event.kind, &event.message);
        if line.is_empty() {
            line = friendly_event_message(&event.message);
        }
        if line.is_empty() {
            continue;
        }
        if let Some(usage) = &event.token_usage {
            line.push_str(&format!(
                " input={} output={} ratio={:.3}",
                usage.input_tokens, usage.output_tokens, usage.context_ratio
            ));
        }
        lines.push(line);
    }
    lines.join("\n")
}

fn format_friendly_turn_report(
    prompt: &str,
    response: &ExecTurnResponse,
    trace: Option<&TraceLookupResponse>,
    tool_outputs: &[(String, String)],
    wall_elapsed: Duration,
) -> String {
    let mut lines = Vec::new();
    if !prompt.trim().is_empty() {
        lines.push(format!("> {prompt}"));
    }
    lines.push(String::new());
    let steps = trace
        .map(|t| build_tool_steps(t, &response.completed.run_id))
        .unwrap_or_default();
    if steps.is_empty() {
        lines.push("process".into());
        let timeline = fallback_timeline_lines(response);
        if timeline.is_empty() {
            lines.push("  - [running] Running".into());
        } else {
            lines.extend(timeline.into_iter().map(|line| format!("  - {line}")));
        }
    } else {
        for step in steps {
            lines.push(format!("  - [{}] {}", step.status, step.tool));
            if let Some(output) = tool_outputs
                .iter()
                .find(|(name, _)| name == &step.tool)
                .map(|(_, content)| content)
            {
                lines.push(format!("    └ {}", summarize_single_line(output)));
            }
            lines.push(format!(
                "    └ Took {}",
                format_seconds(step.duration_seconds)
            ));
        }
    }

    if !is_waiting_approval_response(response) {
        lines.push(String::new());
        lines.push("result".into());
        for line in response.completed.final_text.lines() {
            lines.push(format!("  {line}"));
        }
    }

    lines.push(String::new());
    lines.push(format!(
        "Worked for {}",
        worked_for_text(response, trace, wall_elapsed)
    ));
    lines.join("\n")
}

fn is_waiting_approval_response(response: &ExecTurnResponse) -> bool {
    matches!(response.completed.stage, RunStage::WaitingApproval)
        || response
            .completed
            .events
            .iter()
            .any(|event| event == "approval.requested" || event == "tool.approval_required")
}

#[derive(Debug, Clone)]
struct ToolStep {
    tool: String,
    status: String,
    duration_seconds: i64,
}

fn build_tool_steps(trace: &TraceLookupResponse, run_id: &RunId) -> Vec<ToolStep> {
    let mut started: std::collections::HashMap<String, _> = std::collections::HashMap::new();
    let mut steps = Vec::new();
    for event in trace.events.iter().filter(|event| &event.run_id == run_id) {
        if event.kind == "tool.started" {
            started.insert(event.message.clone(), event.recorded_at);
            continue;
        }
        let status = match event.kind.as_str() {
            "tool.completed" => "completed",
            "tool.denied" => "denied",
            "approval.requested" | "tool.approval_required" => "approval",
            "tool.failed" => "failed",
            _ => continue,
        };
        let tool = event
            .message
            .split(':')
            .next()
            .unwrap_or(event.message.as_str())
            .trim()
            .to_string();
        let duration_seconds = started
            .get(&tool)
            .map(|start| (event.recorded_at - *start).num_seconds().max(0))
            .unwrap_or(0);
        steps.push(ToolStep {
            tool,
            status: status.into(),
            duration_seconds,
        });
    }
    steps
}

fn summarize_single_line(content: &str) -> String {
    let first = content.lines().next().unwrap_or("").trim();
    if first.is_empty() {
        "ok".into()
    } else {
        shorten_middle(first, 90)
    }
}

fn format_seconds(seconds: i64) -> String {
    if seconds < 60 {
        format!("{seconds}s")
    } else {
        format!("{}m {}s", seconds / 60, seconds % 60)
    }
}

async fn load_run_tool_outputs(
    client: &DaemonClient,
    session_id: SessionId,
    run_id: RunId,
) -> anyhow::Result<Vec<(String, String)>> {
    let transcript = load_transcript(client, session_id, Some(80)).await?;
    Ok(transcript
        .messages
        .into_iter()
        .filter_map(|message| match message {
            TranscriptMessage::Tool {
                run_id: message_run_id,
                tool_name,
                content,
                ..
            } if message_run_id == run_id => Some((tool_name, content)),
            _ => None,
        })
        .collect())
}

fn fallback_timeline_lines(response: &ExecTurnResponse) -> Vec<String> {
    response
        .completed
        .events
        .iter()
        .map(|event| friendly_event_line(event, ""))
        .filter(|line| !line.is_empty())
        .collect()
}

fn worked_for_text(
    response: &ExecTurnResponse,
    trace: Option<&TraceLookupResponse>,
    wall_elapsed: Duration,
) -> String {
    let wall_secs = wall_elapsed.as_secs();
    let wall_display_secs = if wall_secs == 0 && wall_elapsed.as_millis() > 0 {
        1
    } else {
        wall_secs
    };

    let trace_secs = trace
        .and_then(|trace| {
            let run_id = &response.completed.run_id;
            let mut times = trace
                .events
                .iter()
                .filter(|event| &event.run_id == run_id)
                .map(|event| event.recorded_at)
                .collect::<Vec<_>>();
            if times.is_empty() {
                return None;
            }
            times.sort();
            Some((times[times.len() - 1] - times[0]).num_seconds().max(0) as u64)
        })
        .unwrap_or(0);

    format_seconds(std::cmp::max(trace_secs, wall_display_secs) as i64)
}

async fn run_tool(
    name: String,
    args: String,
    permission_mode: PermissionMode,
) -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    let arguments = parse_tool_args(&args)?;
    let workspace_root = std::env::current_dir()?.to_string_lossy().to_string();
    let response =
        execute_tool_request(&client, name, arguments, permission_mode, workspace_root).await?;
    println!("{}", format_tool_execution_response(response));
    Ok(())
}

async fn execute_tool_request(
    client: &DaemonClient,
    name: String,
    arguments: serde_json::Value,
    permission_mode: PermissionMode,
    workspace_root: String,
) -> anyhow::Result<ToolExecuteResponse> {
    Ok(client
        .post(format!("{}/tools/execute", client.base_url))
        .json(&ToolExecuteRequest {
            name,
            arguments,
            permission_mode,
            workspace_root: Some(workspace_root),
        })
        .send()
        .await
        .context("failed to submit tool execution request to daemon")?
        .error_for_status()
        .context("daemon rejected tool execution request")?
        .json::<ToolExecuteResponse>()
        .await
        .context("failed to decode tool execution response")?)
}

fn format_tool_execution_response(response: ToolExecuteResponse) -> String {
    let mut lines = vec![friendly_tool_status(&response.status).to_string()];
    if let Some(approval_id) = response.approval_id {
        lines.push(format!("Approval {approval_id}"));
    }
    if let Some(reason) = response.reason {
        lines.push(format!("Reason {reason}"));
    }
    if let Some(content) = response.content {
        lines.push(content);
    }
    lines.join("\n")
}

fn friendly_tool_status(status: &str) -> &'static str {
    match status {
        "completed" => "Tool completed",
        "approval_required" => "Approval required",
        "denied" => "Tool denied",
        "failed" => "Tool failed",
        _ => "Tool",
    }
}

async fn run_approvals(command: Option<ApprovalCommand>) -> anyhow::Result<()> {
    let client = daemon_client(true).await?;
    match command {
        None => {
            let response = list_pending_approvals(&client).await?;
            print_pending_approvals(response);
            Ok(())
        }
        Some(ApprovalCommand::History) => run_approval_history(&client).await,
        Some(ApprovalCommand::Approve { approval_id }) => {
            resolve_approval(&client, approval_id, true).await
        }
        Some(ApprovalCommand::Deny { approval_id }) => {
            resolve_approval(&client, approval_id, false).await
        }
    }
}

async fn list_pending_approvals(client: &DaemonClient) -> anyhow::Result<ApprovalListResponse> {
    Ok(client
        .get(format!("{}/approvals", client.base_url))
        .send()
        .await
        .context("failed to request pending approvals from daemon")?
        .error_for_status()
        .context("daemon rejected pending approvals request")?
        .json::<ApprovalListResponse>()
        .await
        .context("failed to decode pending approvals response")?)
}

fn print_pending_approvals(response: ApprovalListResponse) {
    println!("{}", format_pending_approvals(response));
}

fn format_pending_approvals(response: ApprovalListResponse) -> String {
    if response.pending.is_empty() {
        return "no pending approvals".into();
    }
    response
        .pending
        .into_iter()
        .map(|approval| {
            format!(
                "{}\t{}\t{}\t{}\t{}",
                approval.approval_id,
                approval.tool_name,
                approval.reason,
                approval.workspace_root,
                approval.requested_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn latest_approval_id(response: &ApprovalListResponse) -> Option<String> {
    response
        .pending
        .first()
        .map(|approval| approval.approval_id.to_string())
}

async fn run_approval_history(client: &DaemonClient) -> anyhow::Result<()> {
    let response = fetch_approval_history(client).await?;
    println!("{}", format_approval_history(response));
    Ok(())
}

async fn fetch_approval_history(client: &DaemonClient) -> anyhow::Result<ApprovalHistoryResponse> {
    Ok(client
        .get(format!("{}/approvals/history", client.base_url))
        .send()
        .await
        .context("failed to request approval history from daemon")?
        .error_for_status()
        .context("daemon rejected approval history request")?
        .json::<ApprovalHistoryResponse>()
        .await
        .context("failed to decode approval history response")?)
}

fn format_approval_history(response: ApprovalHistoryResponse) -> String {
    if response.grants.is_empty() {
        return "no approval history".into();
    }
    response
        .grants
        .into_iter()
        .map(|grant| {
            format!(
                "{}\t{}\t{}\t{}\t{}\t{}",
                grant.approval_id,
                grant.tool_name,
                if grant.approved { "approved" } else { "denied" },
                grant.reason.unwrap_or_default(),
                grant.workspace_root,
                grant.resolved_at
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

async fn resolve_approval(
    client: &DaemonClient,
    approval_id: String,
    approved: bool,
) -> anyhow::Result<()> {
    let response = submit_approval_resolution(client, approval_id, approved).await?;
    println!("{}", format_approval_resolution(response));
    Ok(())
}

async fn submit_approval_resolution(
    client: &DaemonClient,
    approval_id: String,
    approved: bool,
) -> anyhow::Result<ApprovalResolveResponse> {
    let response = client
        .post(format!("{}/approvals/resolve", client.base_url))
        .json(&ApprovalResolveRequest {
            approval_id: unio_core::ApprovalId::from_string(approval_id),
            approved,
        })
        .send()
        .await
        .context("failed to submit approval resolution to daemon")?;
    Ok(response
        .error_for_status()
        .context("daemon rejected approval resolution")?
        .json::<ApprovalResolveResponse>()
        .await
        .context("failed to decode approval resolution response")?)
}

fn format_approval_resolution(response: ApprovalResolveResponse) -> String {
    let mut lines = vec![
        format!("approval_id: {}", response.approval_id),
        format!("status: {}", response.status),
    ];
    if let Some(reason) = response.reason {
        lines.push(format!("reason: {reason}"));
    }
    if let Some(content) = response.content {
        lines.push(content);
    }
    if let Some(follow_up_text) = response.follow_up_text {
        lines.push("result".into());
        lines.push(follow_up_text);
    }
    lines.join("\n")
}

async fn run_daemon(command: DaemonCommand) -> anyhow::Result<()> {
    match command {
        DaemonCommand::Start => {
            let instance = ensure_daemon().await?;
            println!("{}", instance.http_url);
        }
        DaemonCommand::Status => {
            let instance =
                current_instance()?.ok_or_else(|| anyhow::anyhow!("daemon not running"))?;
            println!("{}", instance.http_url);
        }
    }
    Ok(())
}

fn run_skills() -> anyhow::Result<()> {
    println!("{}", discovered_skills_text()?);
    Ok(())
}

fn discovered_skills_text() -> anyhow::Result<String> {
    let workspace = std::env::current_dir()?;
    let user_home = user_home()?;
    let paths = WorkspacePaths::new(user_home, workspace);
    let skills = discover_skills(&paths)?;
    let tools = inject_skill_tools(&skills);
    if skills.is_empty() {
        return Ok("no skills discovered".into());
    }
    Ok(skills
        .iter()
        .zip(tools.iter())
        .map(|(skill, tool)| {
            format!(
                "{}\t{}\t{}\t{}",
                tool.name,
                skill_source_label(skill.source),
                tool.description,
                tool.skill_path.display()
            )
        })
        .collect::<Vec<_>>()
        .join("\n"))
}

fn skill_source_label(source: SkillSource) -> &'static str {
    match source {
        SkillSource::Workspace => "workspace",
        SkillSource::User => "user",
    }
}

async fn resolve_session(
    client: &DaemonClient,
    workspace: &std::path::Path,
    permission_mode: PermissionMode,
    strategy: SessionResolveStrategy,
) -> anyhow::Result<SessionSummary> {
    let response = client
        .post(format!("{}/sessions/resolve", client.base_url))
        .json(&ResolveSessionRequest {
            workspace_root: workspace.to_string_lossy().to_string(),
            permission_mode,
            strategy,
        })
        .send()
        .await
        .context("failed to resolve workspace session through daemon")?
        .error_for_status()
        .context("daemon rejected session resolve request")?
        .json::<ResolveSessionResponse>()
        .await
        .context("failed to decode session resolve response")?;
    Ok(response.session)
}

struct DaemonClient {
    http: Client,
    base_url: String,
}

impl DaemonClient {
    fn get(&self, url: String) -> reqwest::RequestBuilder {
        self.http.get(url)
    }

    fn post(&self, url: String) -> reqwest::RequestBuilder {
        self.http.post(url)
    }
}

async fn daemon_client(auto_start: bool) -> anyhow::Result<DaemonClient> {
    let instance = if auto_start {
        ensure_daemon()
            .await
            .context("failed to start or connect to unio daemon")?
    } else {
        current_instance()
            .context("failed to read daemon instance file")?
            .ok_or_else(|| anyhow::anyhow!("daemon not running; run `unio daemon start`"))?
    };
    Ok(DaemonClient {
        http: Client::new(),
        base_url: instance.http_url,
    })
}

async fn ensure_daemon() -> anyhow::Result<DaemonInstance> {
    if let Some(instance) = current_instance()? {
        if daemon_reachable(&instance.http_url).await {
            return Ok(instance);
        }
    }

    let daemon_binary = daemon_binary_path()?;
    Command::new(&daemon_binary)
        .arg("127.0.0.1:7878")
        .spawn()
        .with_context(|| format!("failed to spawn daemon binary: {}", daemon_binary.display()))?;

    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(250)).await;
        if let Some(instance) = current_instance()? {
            if daemon_reachable(&instance.http_url).await {
                return Ok(instance);
            }
        }
    }

    anyhow::bail!("failed to start daemon; expected instance at 127.0.0.1:7878")
}

async fn daemon_reachable(base_url: &str) -> bool {
    Client::new()
        .get(format!("{base_url}/status"))
        .send()
        .await
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

fn current_instance() -> anyhow::Result<Option<DaemonInstance>> {
    let paths = UserPaths::current()?;
    read_instance_file(&paths)
}

fn daemon_binary_path() -> anyhow::Result<PathBuf> {
    let current_exe = std::env::current_exe()?;
    let exe_name = if cfg!(windows) {
        "unio-daemon.exe"
    } else {
        "unio-daemon"
    };
    let sibling = current_exe.with_file_name(exe_name);
    if sibling.exists() {
        return Ok(sibling);
    }
    Ok(PathBuf::from(exe_name))
}

fn user_home() -> anyhow::Result<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("failed to resolve user home"))
}

fn parse_tool_args(value: &str) -> anyhow::Result<serde_json::Value> {
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

#[cfg(test)]
mod tests {
    use super::{
        active_file_reference_query, apply_model_switch_to_config, complete_file_reference,
        configured_latest_version, format_approval_history, format_daemon_status,
        format_exec_summary, format_file_references, format_friendly_turn_report,
        format_models_status, format_pending_approvals, format_tool_execution_response,
        format_trace_response, format_trace_timeline, format_transcript_response,
        format_update_status, latest_approval_id, parse_approval_mode_slash, parse_approval_slash,
        parse_file_references, parse_model_slash, parse_resume_slash, parse_trace_slash,
        print_exec_response, reference_query_candidates, resolve_prompt_file_references,
        scan_file_reference_paths, shorten_middle, skill_source_label, slash_command_suggestions,
        tui_bottom_hints, tui_help_text, tui_input_line, tui_message_lines, tui_welcome_right, Cli,
        CommandSpec, ExecOutputMode, FileReferenceIndex, InputBuffer, ModelConfigWizard,
        ModelSwitch,
    };
    use chrono::Utc;
    use clap::Parser;
    use std::time::Duration;
    use tempfile::tempdir;
    use unio_core::{ApprovalId, RunId, SessionId, TraceId};
    use unio_protocol::{
        ApprovalGrantSummary, ApprovalHistoryResponse, ApprovalListResponse, DaemonStatus,
        ExecTurnResponse, LoadTranscriptResponse, ModelsStatus, PendingApprovalSummary,
        PermissionMode, RunStage, SessionSummary, ToolExecuteResponse, TraceEventRecord,
        TraceLookupResponse, TraceTokenUsage, TranscriptMessage, TurnCompleted, TurnStarted,
    };
    use unio_skills::SkillSource;

    #[test]
    fn skill_source_labels_are_cli_stable() {
        assert_eq!(skill_source_label(SkillSource::Workspace), "workspace");
        assert_eq!(skill_source_label(SkillSource::User), "user");
    }

    #[test]
    fn empty_update_version_is_ignored() {
        std::env::set_var("UNIO_LATEST_VERSION", " ");
        assert_eq!(configured_latest_version(), None);
        std::env::remove_var("UNIO_LATEST_VERSION");
    }

    #[test]
    fn parses_approval_slash_commands() {
        assert_eq!(
            parse_approval_slash("/approve approval_1"),
            Some((true, "approval_1".into()))
        );
        assert_eq!(
            parse_approval_slash("/deny approval_2"),
            Some((false, "approval_2".into()))
        );
        assert_eq!(parse_approval_slash("/approve "), None);
    }

    #[test]
    fn parses_approval_mode_slash_commands() {
        assert_eq!(parse_approval_mode_slash("/approval"), Some(None));
        assert_eq!(
            parse_approval_mode_slash("/approval default"),
            Some(Some(PermissionMode::Default))
        );
        assert_eq!(
            parse_approval_mode_slash("/approval auto"),
            Some(Some(PermissionMode::Auto))
        );
        assert_eq!(
            parse_approval_mode_slash("/approval full-trust"),
            Some(Some(PermissionMode::FullTrust))
        );
        assert_eq!(parse_approval_mode_slash("/approval nope"), None);
    }

    #[test]
    fn parses_trace_slash_command() {
        assert_eq!(
            parse_trace_slash("/trace trace_1"),
            Some(("trace_1".into(), None))
        );
        assert_eq!(
            parse_trace_slash("/trace trace_1 run_1"),
            Some(("trace_1".into(), Some("run_1".into())))
        );
        assert_eq!(parse_trace_slash("/trace "), None);
    }

    #[test]
    fn parses_resume_slash_limit() {
        assert_eq!(parse_resume_slash("/resume"), Some(None));
        assert_eq!(parse_resume_slash("/resume 12"), Some(Some(12)));
        assert_eq!(parse_resume_slash("/resume nope"), None);
    }

    #[test]
    fn parses_only_model_slash_command() {
        assert_eq!(parse_model_slash("/model"), Some(()));
        assert_eq!(parse_model_slash("/models"), None);
        assert_eq!(parse_model_slash("/model config"), None);
        assert_eq!(
            parse_model_slash("/model use openai-compatible gpt-4o-mini"),
            None
        );
    }

    #[test]
    fn model_switch_config_preserves_existing_base_url_and_api_key() {
        let mut config = toml::Value::Table(toml::toml! {
            [model]
            provider = "openai-compatible"
            model = "gpt-4.1"
            base_url = "https://example.test/v1"
            api_key = "file-key"
        });

        apply_model_switch_to_config(
            &mut config,
            &ModelSwitch {
                provider: "anthropic".into(),
                model: Some("claude-3-5-sonnet-latest".into()),
            },
        )
        .unwrap();

        assert_eq!(config["model"]["provider"].as_str(), Some("anthropic"));
        assert_eq!(
            config["model"]["model"].as_str(),
            Some("claude-3-5-sonnet-latest")
        );
        assert_eq!(
            config["model"]["base_url"].as_str(),
            Some("https://example.test/v1")
        );
        assert_eq!(config["model"]["api_key"].as_str(), Some("file-key"));
    }

    #[test]
    fn model_config_wizard_collects_values_and_updates_config() {
        let mut config = toml::Value::Table(toml::toml! {
            [model]
            provider = "mock"
            model = "mock"
        });
        let mut wizard = ModelConfigWizard::from_config(&config);

        assert!(wizard.prompt().contains("provider"));
        assert_eq!(wizard.accept("openai-compatible").unwrap(), None);
        assert!(wizard.prompt().contains("model"));
        assert_eq!(wizard.accept("gpt-4o-mini").unwrap(), None);
        assert!(wizard.prompt().contains("base_url"));
        assert_eq!(wizard.accept("https://api.openai.com/v1").unwrap(), None);
        let finished = wizard.accept("sk-test").unwrap().unwrap();

        finished.apply_to_config(&mut config).unwrap();
        assert_eq!(
            config["model"]["provider"].as_str(),
            Some("openai-compatible")
        );
        assert_eq!(config["model"]["model"].as_str(), Some("gpt-4o-mini"));
        assert_eq!(
            config["model"]["base_url"].as_str(),
            Some("https://api.openai.com/v1")
        );
        assert_eq!(config["model"]["api_key"].as_str(), Some("sk-test"));
    }

    #[test]
    fn cli_parses_exec_quiet_mode() {
        let cli = Cli::try_parse_from(["unio", "exec", "hello", "--quiet"]).unwrap();

        let Some(CommandSpec::Exec { prompt, quiet, .. }) = cli.command else {
            panic!("expected exec command");
        };
        assert_eq!(prompt, "hello");
        assert!(quiet);
    }

    #[test]
    fn cli_parses_resume_limit_and_trace_run_filter() {
        let resume = Cli::try_parse_from(["unio", "resume", "--limit", "9"]).unwrap();
        let Some(CommandSpec::Resume { session_id, limit }) = resume.command else {
            panic!("expected resume command");
        };
        assert_eq!(session_id, None);
        assert_eq!(limit, Some(9));

        let trace = Cli::try_parse_from(["unio", "trace", "trace_1", "--run", "run_1"]).unwrap();
        let Some(CommandSpec::Trace { trace_id, run }) = trace.command else {
            panic!("expected trace command");
        };
        assert_eq!(trace_id, "trace_1");
        assert_eq!(run.as_deref(), Some("run_1"));
    }

    #[test]
    fn cli_accepts_slash_compatible_prompt_without_subcommand() {
        let cli = Cli::try_parse_from(["unio", "/skills"]).unwrap();

        assert_eq!(cli.prompt.as_deref(), Some("/skills"));
        assert!(cli.command.is_none());
    }

    #[test]
    fn quiet_exec_output_mode_is_constructible() {
        let response = ExecTurnResponse {
            started: TurnStarted {
                session_id: SessionId::from_string("session_1"),
                conversation_id: unio_protocol::ConversationId::new(),
                run_id: RunId::from_string("run_1"),
                stage: RunStage::Streaming,
            },
            completed: TurnCompleted {
                session_id: SessionId::from_string("session_1"),
                run_id: RunId::from_string("run_1"),
                trace_id: TraceId::from_string("trace_1"),
                stage: RunStage::Completed,
                final_text: "ok".into(),
                events: vec!["root_agent.completed".into()],
                provider: "mock".into(),
                model: "mock".into(),
                input_tokens: 1,
                output_tokens: 1,
                context_ratio: 0.1,
            },
        };

        print_exec_response(&response, ExecOutputMode { quiet: true });
    }

    #[test]
    fn exec_summary_includes_trace_and_context() {
        let response = ExecTurnResponse {
            started: TurnStarted {
                session_id: SessionId::from_string("session_1"),
                conversation_id: unio_protocol::ConversationId::new(),
                run_id: RunId::from_string("run_1"),
                stage: RunStage::Streaming,
            },
            completed: TurnCompleted {
                session_id: SessionId::from_string("session_1"),
                run_id: RunId::from_string("run_1"),
                trace_id: TraceId::from_string("trace_1"),
                stage: RunStage::Completed,
                final_text: "ok".into(),
                events: vec![],
                provider: "mock".into(),
                model: "mock".into(),
                input_tokens: 3,
                output_tokens: 5,
                context_ratio: 0.25,
            },
        };

        let metadata = format_exec_summary(&response);

        assert!(metadata.contains("Trace trace_1"));
        assert!(metadata.contains("context_ratio=0.250"));
    }

    #[test]
    fn exec_summary_formats_tool_and_approval_events_friendly() {
        let response = ExecTurnResponse {
            started: TurnStarted {
                session_id: SessionId::from_string("session_1"),
                conversation_id: unio_protocol::ConversationId::new(),
                run_id: RunId::from_string("run_1"),
                stage: RunStage::Streaming,
            },
            completed: TurnCompleted {
                session_id: SessionId::from_string("session_1"),
                run_id: RunId::from_string("run_1"),
                trace_id: TraceId::from_string("trace_1"),
                stage: RunStage::WaitingApproval,
                final_text: "Tool execution is waiting for approval: approval_1".into(),
                events: vec![
                    "root_agent.completed".into(),
                    "tool.completed".into(),
                    "tool.approval_required".into(),
                    "skill.completed".into(),
                ],
                provider: "openai-compatible".into(),
                model: "gpt-4o-mini".into(),
                input_tokens: 12,
                output_tokens: 7,
                context_ratio: 0.42,
            },
        };

        let summary = format_exec_summary(&response);

        assert!(summary.contains("Done WaitingApproval"));
        assert!(summary.contains("Tool completed"));
        assert!(summary.contains("Approval required"));
        assert!(summary.contains("Skill completed"));
        assert!(!summary.contains("events:"));
    }

    #[test]
    fn pending_approval_formatter_handles_empty_list() {
        let output = format_pending_approvals(ApprovalListResponse { pending: vec![] });

        assert_eq!(output, "no pending approvals");
    }

    #[test]
    fn latest_approval_id_uses_first_pending_item() {
        let response = ApprovalListResponse {
            pending: vec![PendingApprovalSummary {
                approval_id: ApprovalId::from_string("approval_1"),
                tool_call_id: "tool_call_1".into(),
                tool_name: "write".into(),
                call_arguments_json: "{\"path\":\"README.md\",\"content\":\"hi\"}".into(),
                reason: "requires approval".into(),
                workspace_root: "F:/repo".into(),
                requested_at: Utc::now(),
            }],
        };

        assert_eq!(latest_approval_id(&response), Some("approval_1".into()));
    }

    #[test]
    fn tui_help_lists_runtime_commands() {
        let help = tui_help_text();

        assert!(help.contains("/approval"));
        assert!(help.contains("/pending"));
        assert!(help.contains("/trace [id] [run_id]"));
        assert!(help.contains("/skills"));
        assert!(help.contains("@path"));
        assert!(help.contains("ctrl+c twice"));
    }

    #[test]
    fn parses_file_references_without_validating_files() {
        let references =
            parse_file_references("inspect @README.md and @crates/protocol/src/lib.rs");

        assert_eq!(
            references,
            vec![
                "README.md".to_string(),
                "crates/protocol/src/lib.rs".to_string()
            ]
        );
        assert!(parse_file_references("email a@b.com").is_empty());
        assert_eq!(parse_file_references("@README.md"), vec!["README.md"]);
    }

    #[test]
    fn resolves_typos_like_dot_cargo_lock_to_cargo_lock() {
        let index = FileReferenceIndex::from_paths(vec![
            "Cargo.lock".into(),
            "README.md".into(),
            "crates/protocol/src/lib.rs".into(),
        ]);
        let resolved = resolve_prompt_file_references("介绍 @.cargo-lock 的内容", &index);

        assert_eq!(resolved.references, vec!["Cargo.lock".to_string()]);
        assert!(resolved.unresolved.is_empty());
        assert!(resolved.rewritten_prompt.contains("@Cargo.lock"));
    }

    #[test]
    fn query_candidates_include_dot_dash_variants() {
        let variants = reference_query_candidates("cargo-lock");
        assert!(variants.contains(&"cargo-lock".to_string()));
        assert!(variants.contains(&"cargo.lock".to_string()));
        assert!(variants.contains(&"cargolock".to_string()));
    }

    #[test]
    fn file_reference_formatter_lists_references() {
        let output = format_file_references(&["README.md".into(), "src/main.rs".into()]);

        assert!(output.contains("system: file references"));
        assert!(output.contains("README.md"));
        assert!(output.contains("src/main.rs"));
    }

    #[test]
    fn file_reference_query_accepts_line_start_or_leading_space() {
        assert_eq!(
            active_file_reference_query("inspect @src/ma"),
            Some((8, "src/ma".into()))
        );
        assert_eq!(
            active_file_reference_query("@src/ma"),
            Some((0, "src/ma".into()))
        );
        assert_eq!(active_file_reference_query("email a@b.com"), None);
    }

    #[test]
    fn file_reference_completion_inserts_reference_and_space() {
        let mut input = InputBuffer::default();
        input.insert_str("inspect @src/ma");

        complete_file_reference(&mut input, "src/main.rs");

        assert_eq!(input.as_str(), "inspect @src/main.rs ");
    }

    #[test]
    fn input_buffer_moves_cursor_and_inserts_in_middle() {
        let mut input = InputBuffer::default();
        input.insert_str("helo");
        input.move_left();
        input.insert('l');

        assert_eq!(input.as_str(), "hello");
        assert_eq!(input.cursor(), 4);
    }

    #[test]
    fn input_buffer_deletes_word_and_clears_line() {
        let mut input = InputBuffer::default();
        input.insert_str("hello world  ");

        input.delete_previous_word();
        assert_eq!(input.as_str(), "hello ");

        input.clear();
        assert!(input.is_empty());
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn input_buffer_supports_multiline_prompts() {
        let mut input = InputBuffer::default();
        input.insert_str("first");
        input.insert('\n');
        input.insert_str("second");

        assert_eq!(input.as_str(), "first\nsecond");
    }

    #[test]
    fn completion_preserves_text_after_cursor() {
        let mut input = InputBuffer::default();
        input.insert_str("inspect @ma please");
        for _ in 0.." please".len() {
            input.move_left();
        }

        complete_file_reference(&mut input, "src/main.rs");

        assert_eq!(input.as_str(), "inspect @src/main.rs  please");
    }

    #[test]
    fn file_reference_index_suggestions_match_workspace_paths() {
        let index = FileReferenceIndex::from_paths(vec![
            "README.md".into(),
            "src/main.rs".into(),
            "src/lib.rs".into(),
        ]);

        let suggestions = index.suggestions("inspect @main");

        assert_eq!(suggestions, vec!["src/main.rs".to_string()]);
    }

    #[test]
    fn file_reference_index_ranks_exact_basename_first() {
        let index = FileReferenceIndex::from_paths(vec![
            "deps/unio.exe".into(),
            "unio.exe".into(),
            "target/debug/unio.exe".into(),
        ]);

        let suggestions = index.suggestions("@unio.exe");

        assert_eq!(suggestions.first().map(String::as_str), Some("unio.exe"));
    }

    #[test]
    fn file_reference_scanner_prunes_ignored_and_heavy_dirs() {
        let temp = tempdir().unwrap();
        std::fs::create_dir_all(temp.path().join("src")).unwrap();
        std::fs::create_dir_all(temp.path().join("node_modules/pkg")).unwrap();
        std::fs::create_dir_all(temp.path().join(".git")).unwrap();
        std::fs::create_dir_all(temp.path().join("ignored")).unwrap();
        std::fs::write(temp.path().join(".gitignore"), "ignored/\n*.log\n").unwrap();
        std::fs::write(temp.path().join("src/main.rs"), "").unwrap();
        std::fs::write(temp.path().join("debug.log"), "").unwrap();
        std::fs::write(temp.path().join("node_modules/pkg/index.js"), "").unwrap();
        std::fs::write(temp.path().join(".git/config"), "").unwrap();
        std::fs::write(temp.path().join("ignored/file.txt"), "").unwrap();

        let paths = scan_file_reference_paths(temp.path())
            .into_iter()
            .map(|entry| entry.path)
            .collect::<Vec<_>>();

        assert_eq!(paths, vec!["src/main.rs".to_string()]);
    }

    #[test]
    fn hybrid_startup_panels_include_tips_and_placeholder() {
        let tips = tui_welcome_right(None);
        let tip_text = tips
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(tip_text.contains("Tips for getting started"));
        assert!(tip_text.contains("No recent activity"));
        assert!(tui_input_line("", 0, None).to_string().contains("Try"));
        assert!(tui_bottom_hints("", false, &[], 0, &[], 0, None)
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n")
            .contains("for commands"));
    }

    #[test]
    fn hybrid_bottom_hints_follow_input_prefixes() {
        let slash_suggestions = slash_command_suggestions("/");
        let slash = tui_bottom_hints("/", false, &[], 0, &slash_suggestions, 0, None)
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let question = tui_bottom_hints("?", false, &[], 0, &[], 0, None)
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let file = tui_bottom_hints("@", false, &[], 0, &[], 0, None)
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let matches = tui_bottom_hints(
            "inspect @ma",
            false,
            &["src/main.rs".into()],
            0,
            &[],
            0,
            None,
        )
        .iter()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        let exit = tui_bottom_hints("", true, &[], 0, &[], 0, None)
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(slash.contains("/skills"));
        assert!(question.contains("examples:"));
        assert!(file.contains("@README.md"));
        assert!(matches.contains("src/main.rs"));
        assert!(exit.contains("again to exit Unio"));
    }

    #[test]
    fn model_config_wizard_owns_input_placeholder_and_hints() {
        let config = toml::Value::Table(toml::toml! {
            [model]
            provider = "mock"
            model = "mock"
        });
        let wizard = ModelConfigWizard::from_config(&config);

        assert!(tui_input_line("", 0, Some(wizard.input_placeholder()))
            .to_string()
            .contains("Model setup 1/4"));
        let hints = tui_bottom_hints("", false, &[], 0, &[], 0, Some(&wizard))
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(hints.contains("choose provider"));
        assert!(hints.contains("Enter keeps"));
    }

    #[test]
    fn slash_command_suggestions_filter_by_prefix() {
        let suggestions = slash_command_suggestions("/re")
            .into_iter()
            .map(|command| command.name)
            .collect::<Vec<_>>();

        assert_eq!(suggestions, vec!["/resume", "/refresh"]);
        assert!(slash_command_suggestions("/resume 5").is_empty());
        let model_suggestions = slash_command_suggestions("/mo")
            .into_iter()
            .map(|command| command.name)
            .collect::<Vec<_>>();
        assert_eq!(model_suggestions, vec!["/model"]);
    }

    #[test]
    fn hybrid_message_lines_style_roles_as_text_rows() {
        let lines = tui_message_lines(&[
            "> create readme".into(),
            "  - Tool completed write README.md".into(),
            "user: hello".into(),
            "assistant: hi".into(),
            "error: no".into(),
        ]);
        let text = lines
            .iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("> create readme"));
        assert!(text.contains("Tool completed write README.md"));
        assert!(text.contains("user: hello"));
        assert!(text.contains("assistant: hi"));
        assert!(text.contains("error: no"));
    }

    #[test]
    fn shorten_middle_keeps_edges() {
        let shortened = shorten_middle("abcdefghijklmnopqrstuvwxyz", 10);

        assert_eq!(shortened.chars().count(), 10);
        assert!(shortened.starts_with("abc"));
        assert!(shortened.ends_with("wxyz"));
    }

    #[test]
    fn trace_timeline_marks_context_and_tool_events() {
        let response = TraceLookupResponse {
            trace_id: TraceId::from_string("trace_1"),
            events: vec![
                TraceEventRecord {
                    run_id: RunId::from_string("run_1"),
                    kind: "tool.completed".into(),
                    message: "read".into(),
                    token_usage: None,
                    recorded_at: Utc::now(),
                },
                TraceEventRecord {
                    run_id: RunId::from_string("run_1"),
                    kind: "context.budget".into(),
                    message: "checkpoint".into(),
                    token_usage: Some(TraceTokenUsage {
                        input_tokens: 10,
                        output_tokens: 4,
                        context_ratio: 0.85,
                    }),
                    recorded_at: Utc::now(),
                },
            ],
        };

        let timeline = format_trace_timeline(&response);

        assert!(timeline.contains("Tool completed read"));
        assert!(timeline.contains("Context budget checkpoint"));
        assert!(timeline.contains("ratio=0.850"));
    }

    #[test]
    fn friendly_turn_report_includes_input_process_result_and_duration() {
        let now = Utc::now();
        let response = ExecTurnResponse {
            started: TurnStarted {
                session_id: SessionId::from_string("session_1"),
                conversation_id: unio_protocol::ConversationId::new(),
                run_id: RunId::from_string("run_1"),
                stage: RunStage::Streaming,
            },
            completed: TurnCompleted {
                session_id: SessionId::from_string("session_1"),
                run_id: RunId::from_string("run_1"),
                trace_id: TraceId::from_string("trace_1"),
                stage: RunStage::Completed,
                final_text: "created README.md".into(),
                events: vec!["tool.completed".into()],
                provider: "mock".into(),
                model: "mock".into(),
                input_tokens: 1,
                output_tokens: 1,
                context_ratio: 0.1,
            },
        };
        let trace = TraceLookupResponse {
            trace_id: TraceId::from_string("trace_1"),
            events: vec![
                TraceEventRecord {
                    run_id: RunId::from_string("run_1"),
                    kind: "tool.started".into(),
                    message: "write".into(),
                    token_usage: None,
                    recorded_at: now,
                },
                TraceEventRecord {
                    run_id: RunId::from_string("run_1"),
                    kind: "tool.completed".into(),
                    message: "write".into(),
                    token_usage: None,
                    recorded_at: now + chrono::Duration::seconds(2),
                },
            ],
        };

        let output = format_friendly_turn_report(
            "创建 README",
            &response,
            Some(&trace),
            &[],
            Duration::from_secs(2),
        );

        assert!(!output.contains("goal:"));
        assert!(output.contains("[completed] write"));
        assert!(output.contains("Took 2s"));
        assert!(output.contains("result"));
        assert!(output.contains("created README.md"));
        assert!(output.contains("Worked for 2s"));
    }

    #[test]
    fn friendly_turn_report_omits_goal_when_prompt_is_empty() {
        let response = ExecTurnResponse {
            started: TurnStarted {
                session_id: SessionId::from_string("session_1"),
                conversation_id: unio_protocol::ConversationId::new(),
                run_id: RunId::from_string("run_1"),
                stage: RunStage::Streaming,
            },
            completed: TurnCompleted {
                session_id: SessionId::from_string("session_1"),
                run_id: RunId::from_string("run_1"),
                trace_id: TraceId::from_string("trace_1"),
                stage: RunStage::Completed,
                final_text: "ok".into(),
                events: vec!["tool.completed".into()],
                provider: "mock".into(),
                model: "mock".into(),
                input_tokens: 1,
                output_tokens: 1,
                context_ratio: 0.1,
            },
        };

        let output =
            format_friendly_turn_report("", &response, None, &[], Duration::from_millis(500));

        assert!(!output.contains("goal:"));
        assert!(!output.contains("\n> "));
        assert!(output.contains("process"));
        assert!(output.contains("result"));
        assert!(output.contains("Worked for 1s"));
    }

    #[test]
    fn friendly_turn_report_hides_result_when_waiting_approval() {
        let now = Utc::now();
        let response = ExecTurnResponse {
            started: TurnStarted {
                session_id: SessionId::from_string("session_1"),
                conversation_id: unio_protocol::ConversationId::new(),
                run_id: RunId::from_string("run_1"),
                stage: RunStage::Streaming,
            },
            completed: TurnCompleted {
                session_id: SessionId::from_string("session_1"),
                run_id: RunId::from_string("run_1"),
                trace_id: TraceId::from_string("trace_1"),
                stage: RunStage::WaitingApproval,
                final_text: "Tool execution is waiting for approval: approval_1".into(),
                events: vec!["approval.requested".into()],
                provider: "mock".into(),
                model: "mock".into(),
                input_tokens: 1,
                output_tokens: 1,
                context_ratio: 0.1,
            },
        };
        let trace = TraceLookupResponse {
            trace_id: TraceId::from_string("trace_1"),
            events: vec![
                TraceEventRecord {
                    run_id: RunId::from_string("run_1"),
                    kind: "tool.started".into(),
                    message: "bash".into(),
                    token_usage: None,
                    recorded_at: now,
                },
                TraceEventRecord {
                    run_id: RunId::from_string("run_1"),
                    kind: "approval.requested".into(),
                    message: "bash: approval_1".into(),
                    token_usage: None,
                    recorded_at: now + chrono::Duration::seconds(1),
                },
            ],
        };

        let output = format_friendly_turn_report(
            "有多少个文件",
            &response,
            Some(&trace),
            &[],
            Duration::from_secs(1),
        );

        assert!(output.contains("[approval] bash"));
        assert!(!output.contains("\nresult\n"));
        assert!(!output.contains("Tool execution is waiting for approval"));
    }

    #[test]
    fn full_trace_formatter_groups_events_by_run() {
        let response = TraceLookupResponse {
            trace_id: TraceId::from_string("trace_1"),
            events: vec![TraceEventRecord {
                run_id: RunId::from_string("run_1"),
                kind: "approval.resolved".into(),
                message: "approved: write".into(),
                token_usage: None,
                recorded_at: Utc::now(),
            }],
        };

        let output = format_trace_response(response);

        assert!(output.contains("trace: trace_1"));
        assert!(output.contains("run: run_1"));
        assert!(output.contains("approval.resolved"));
    }

    #[test]
    fn transcript_formatter_keeps_recent_message_roles() {
        let output = format_transcript_response(LoadTranscriptResponse {
            session: SessionSummary {
                session_id: SessionId::from_string("session_1"),
                title: "repo".into(),
                workspace_root: "F:/repo".into(),
                permission_mode: PermissionMode::Default,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                last_run_id: Some(RunId::from_string("run_1")),
            },
            messages: vec![
                TranscriptMessage::User {
                    session_id: SessionId::from_string("session_1"),
                    run_id: RunId::from_string("run_1"),
                    content: "hello".into(),
                    recorded_at: Utc::now(),
                },
                TranscriptMessage::Assistant {
                    session_id: SessionId::from_string("session_1"),
                    run_id: RunId::from_string("run_1"),
                    content: "hi".into(),
                    reasoning_content: None,
                    recorded_at: Utc::now(),
                },
                TranscriptMessage::Tool {
                    session_id: SessionId::from_string("session_1"),
                    run_id: RunId::from_string("run_1"),
                    tool_call_id: "tool_call_1".into(),
                    tool_name: "read".into(),
                    content: "README".into(),
                    recorded_at: Utc::now(),
                },
            ],
        });

        assert!(output.contains("session_1\trepo\tF:/repo"));
        assert!(output.contains("user: hello"));
        assert!(output.contains("assistant: hi"));
        assert!(output.contains("tool[read]: README"));
    }

    #[test]
    fn model_status_formatter_is_cli_and_tui_friendly() {
        let output = format_models_status(&ModelsStatus {
            provider: "mock".into(),
            model: "mock".into(),
            fallback_to_mock: true,
        });

        assert!(output.contains("provider: mock"));
        assert!(output.contains("fallback_to_mock: true"));
    }

    #[test]
    fn update_formatter_reports_unknown_source() {
        let output = format_update_status("0.1.0", None);

        assert!(output.contains("latest_version: unknown"));
        assert!(output.contains("UNIO_LATEST_VERSION"));
    }

    #[test]
    fn daemon_status_formatter_includes_runtime_summary() {
        let output = format_daemon_status(
            &DaemonStatus {
                pid: 42,
                http_url: "http://127.0.0.1:7878".into(),
                started_at: Utc::now(),
                session_count: 2,
                pending_approval_count: 1,
                latest_session_id: Some(SessionId::from_string("session_1")),
                latest_trace_id: Some(TraceId::from_string("trace_1")),
                latest_context_ratio: Some(0.7),
                models: ModelsStatus {
                    provider: "mock".into(),
                    model: "mock".into(),
                    fallback_to_mock: true,
                },
            },
            "F:/repo".into(),
        );

        assert!(output.contains("pid: 42"));
        assert!(output.contains("workspace: F:/repo"));
        assert!(output.contains("latest_context_ratio: 0.700"));
    }

    #[test]
    fn tool_execution_formatter_includes_approval_and_content() {
        let output = format_tool_execution_response(ToolExecuteResponse {
            status: "approval_required".into(),
            content: Some("content".into()),
            reason: Some("write requires approval".into()),
            approval_id: Some(ApprovalId::from_string("approval_1")),
        });

        assert!(output.contains("Approval required"));
        assert!(output.contains("approval_1"));
        assert!(output.contains("write requires approval"));
        assert!(output.contains("content"));
    }

    #[test]
    fn approval_history_formatter_marks_approved_and_denied_grants() {
        let output = format_approval_history(ApprovalHistoryResponse {
            grants: vec![
                ApprovalGrantSummary {
                    approval_id: ApprovalId::from_string("approval_1"),
                    tool_call_id: "tool_call_1".into(),
                    tool_name: "write".into(),
                    workspace_root: "F:/repo".into(),
                    approved: true,
                    reason: Some("ok".into()),
                    resolved_at: Utc::now(),
                },
                ApprovalGrantSummary {
                    approval_id: ApprovalId::from_string("approval_2"),
                    tool_call_id: "tool_call_2".into(),
                    tool_name: "bash".into(),
                    workspace_root: "F:/repo".into(),
                    approved: false,
                    reason: Some("denied by user".into()),
                    resolved_at: Utc::now(),
                },
            ],
        });

        assert!(output.contains("approval_1\twrite\tapproved"));
        assert!(output.contains("approval_2\tbash\tdenied"));
    }
}
