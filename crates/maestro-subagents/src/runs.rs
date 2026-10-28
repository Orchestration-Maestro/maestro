//! Child observations and live tool progress.

use crate::agents::AgentScope;
use maestro_extensions_wasm::{
    AgentToolResult, AgentToolUpdateCallback, ExtensionResult, Presence, TextContent, UserBlock,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{cell::RefCell, rc::Rc};

/// Native process isolation and output collection.
pub(crate) mod process_child;

/// One delegated invocation.
#[derive(Clone, Deserialize)]
pub(crate) struct TaskItem {
    /// Authored agent name.
    pub agent: String,
    /// Literal delegated instructions.
    pub task: String,
    /// Optional native working directory.
    pub cwd: Option<String>,
}

/// Additive usage plus the last observed context size.
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Usage {
    /// Input tokens across turns.
    pub input: f64,
    /// Output tokens across turns.
    pub output: f64,
    /// Cached input tokens.
    pub cache_read: f64,
    /// Cached output tokens.
    pub cache_write: f64,
    /// Total reported cost.
    pub cost: f64,
    /// Most recent context size.
    pub context_tokens: f64,
    /// Assistant messages observed.
    pub turns: u64,
}

/// One child with its unmodified messages and provenance.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SingleResult {
    /// Requested name.
    pub agent: String,
    /// Origin, or unknown when discovery found no such name.
    pub agent_source: String,
    /// Instructions after chain substitution.
    pub task: String,
    /// Minus one while running, otherwise the native numeric exit.
    pub exit_code: i32,
    /// Complete observed messages.
    pub messages: Vec<Value>,
    /// Captured error output.
    pub stderr: String,
    /// Accumulated usage.
    pub usage: Usage,
    /// Selected or first reported model.
    pub model: Option<String>,
    /// Last reported stop reason.
    pub stop_reason: Option<String>,
    /// Last reported provider error.
    pub error_message: Option<String>,
    /// One-indexed chain position.
    pub step: Option<usize>,
}
impl SingleResult {
    /// Creates an observation before an agent is resolved or launched.
    pub fn pending(task: &TaskItem) -> Self {
        Self {
            agent: task.agent.clone(),
            agent_source: "unknown".into(),
            task: task.task.clone(),
            exit_code: -1,
            messages: Vec::new(),
            stderr: String::new(),
            usage: Usage::default(),
            model: None,
            stop_reason: None,
            error_message: None,
            step: None,
        }
    }
    /// Selects the first text block of the newest assistant with text.
    pub fn output(&self) -> &str {
        for message in self
            .messages
            .iter()
            .rev()
            .filter(|message| message["role"] == "assistant")
        {
            if let Some(text) = message["content"]
                .as_array()
                .and_then(|parts| parts.iter().find(|part| part["type"] == "text"))
                .and_then(|part| part["text"].as_str())
            {
                return text;
            }
        }
        ""
    }
    /// Includes provider-reported failures even after a successful process exit.
    pub fn failed(&self) -> bool {
        self.exit_code != 0 || matches!(self.stop_reason.as_deref(), Some("error" | "aborted"))
    }
    /// Chooses the most specific supplied failure text.
    pub fn error(&self) -> &str {
        self.error_message
            .as_deref()
            .filter(|text| !text.is_empty())
            .or_else(|| (!self.stderr.is_empty()).then_some(self.stderr.as_str()))
            .or_else(|| (!self.output().is_empty()).then_some(self.output()))
            .unwrap_or("(no output)")
    }
}

/// Results delivered to both the model and the renderer.
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubagentDetails {
    /// Single, parallel or chain.
    pub mode: String,
    /// Admitted directory sources.
    pub agent_scope: AgentScope,
    /// Closest project agents directory.
    pub project_agents_dir: Option<String>,
    /// Results in invocation order.
    pub results: Vec<SingleResult>,
}

/// Current results shared by independently streaming children.
pub(crate) struct Progress {
    /// Owned result state; no borrow is held during a host callback.
    pub details: Rc<RefCell<SubagentDetails>>,
    /// Caller-selected live updates.
    pub update: Option<AgentToolUpdateCallback>,
}
impl Progress {
    /// Replaces one live result before notifying the host.
    pub fn report(&self, index: usize, result: &SingleResult) -> ExtensionResult<()> {
        let details = {
            let mut details = self.details.borrow_mut();
            if index == details.results.len() {
                details.results.push(result.clone());
            } else {
                details.results[index] = result.clone();
            }
            details.clone()
        };
        if let Some(update) = &self.update {
            let text = if details.mode == "parallel" {
                let running = details
                    .results
                    .iter()
                    .filter(|result| result.exit_code == -1)
                    .count();
                format!(
                    "Parallel: {}/{} done, {running} running...",
                    details.results.len() - running,
                    details.results.len()
                )
            } else {
                fallback(result.output(), "(running...)").into()
            };
            update(tool_result(text, &details)?)?;
        }
        Ok(())
    }
}

/// Preserves a nonempty observation or the caller's empty-state text.
pub(crate) fn fallback<'a>(text: &'a str, empty: &'a str) -> &'a str {
    if text.is_empty() { empty } else { text }
}

/// Builds the shared result without adding an ignored error flag.
pub(crate) fn tool_result(
    text: String,
    details: &SubagentDetails,
) -> ExtensionResult<AgentToolResult> {
    Ok(AgentToolResult {
        content: vec![UserBlock::Text(TextContent::new(text))],
        details: Presence::Present(
            serde_json::to_string(details)
                .map_err(|error| error.to_string())?
                .into(),
        ),
        terminate: Presence::Missing,
    })
}
