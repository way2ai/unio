use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use unio_core::{RunId, TraceId};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub context_ratio: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceEvent {
    pub trace_id: TraceId,
    pub run_id: RunId,
    pub kind: String,
    pub message: String,
    pub token_usage: Option<TokenUsage>,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceSummary {
    pub trace_id: TraceId,
    pub run_id: RunId,
    pub status: String,
    pub event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TracePersistedSummary {
    pub latest_trace_id: Option<TraceId>,
    pub event_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextBudgetEvent {
    pub kind: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct JsonlTraceStore {
    path: std::path::PathBuf,
}

impl JsonlTraceStore {
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn append(&self, event: &TraceEvent) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, event)?;
        use std::io::Write;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(())
    }

    pub fn summarize(&self) -> anyhow::Result<TracePersistedSummary> {
        if !self.path.exists() {
            return Ok(TracePersistedSummary {
                latest_trace_id: None,
                event_count: 0,
            });
        }
        let content = std::fs::read_to_string(&self.path)?;
        let mut latest = None;
        let mut count = 0usize;
        for line in content.lines().filter(|line| !line.trim().is_empty()) {
            let event: TraceEvent = match serde_json::from_str(line) {
                Ok(event) => event,
                Err(_) => continue,
            };
            latest = Some(event.trace_id);
            count += 1;
        }
        Ok(TracePersistedSummary {
            latest_trace_id: latest,
            event_count: count,
        })
    }

    pub fn events_by_trace_id(&self, trace_id: &TraceId) -> anyhow::Result<Vec<TraceEvent>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&self.path)?;
        let mut events = Vec::new();
        for line in content.lines().filter(|line| !line.trim().is_empty()) {
            let event: TraceEvent = match serde_json::from_str(line) {
                Ok(event) => event,
                Err(_) => continue,
            };
            if &event.trace_id == trace_id {
                events.push(event);
            }
        }
        Ok(events)
    }
}

pub fn context_warning_level(context_ratio: f32) -> &'static str {
    if context_ratio >= 0.90 {
        "critical"
    } else if context_ratio >= 0.85 {
        "compress"
    } else if context_ratio >= 0.70 {
        "summarize"
    } else {
        "normal"
    }
}

pub fn context_budget_events(context_ratio: f32) -> Vec<ContextBudgetEvent> {
    let level = context_warning_level(context_ratio);
    if level == "normal" {
        return Vec::new();
    }

    let mut events = vec![ContextBudgetEvent {
        kind: format!("context.{level}"),
        message: format!("context ratio is {:.1}%", context_ratio * 100.0),
    }];

    if context_ratio >= 0.70 {
        events.push(ContextBudgetEvent {
            kind: "context.summary_requested".into(),
            message: "context summary should be prepared before the next long run".into(),
        });
    }
    if context_ratio >= 0.85 {
        events.push(ContextBudgetEvent {
            kind: "context.compression_checkpoint".into(),
            message: "context compression checkpoint recorded for resume and future compaction"
                .into(),
        });
    }
    if context_ratio >= 0.90 {
        events.push(ContextBudgetEvent {
            kind: "context.critical".into(),
            message: "context is near capacity; future runs should compact before adding work"
                .into(),
        });
    }

    events
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::{
        context_budget_events, context_warning_level, JsonlTraceStore, TokenUsage, TraceEvent,
    };
    use unio_core::{RunId, TraceId};

    #[test]
    fn context_ratio_thresholds_match_target_design() {
        assert_eq!(context_warning_level(0.69), "normal");
        assert_eq!(context_warning_level(0.70), "summarize");
        assert_eq!(context_warning_level(0.85), "compress");
        assert_eq!(context_warning_level(0.90), "critical");
    }

    #[test]
    fn context_budget_events_escalate_with_ratio() {
        assert!(context_budget_events(0.69).is_empty());
        assert_eq!(
            context_budget_events(0.70)
                .iter()
                .map(|event| event.kind.as_str())
                .collect::<Vec<_>>(),
            vec!["context.summarize", "context.summary_requested"]
        );
        assert!(context_budget_events(0.85)
            .iter()
            .any(|event| event.kind == "context.compression_checkpoint"));
        assert!(context_budget_events(0.90)
            .iter()
            .any(|event| event.kind == "context.critical"));
    }

    #[test]
    fn jsonl_trace_store_persists_events() {
        let dir = tempdir().unwrap();
        let store = JsonlTraceStore::new(dir.path().join("events.jsonl"));
        store
            .append(&TraceEvent {
                trace_id: TraceId::from_string("trace_1"),
                run_id: RunId::from_string("run_1"),
                kind: "run.completed".into(),
                message: "done".into(),
                token_usage: Some(TokenUsage {
                    input_tokens: 1,
                    output_tokens: 2,
                    context_ratio: 0.2,
                }),
                recorded_at: chrono::Utc::now(),
            })
            .unwrap();

        let summary = store.summarize().unwrap();
        assert_eq!(summary.event_count, 1);
        assert_eq!(summary.latest_trace_id.unwrap().as_str(), "trace_1");

        let events = store
            .events_by_trace_id(&TraceId::from_string("trace_1"))
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "run.completed");
    }
}
