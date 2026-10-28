//! Literal native child launches with owned cancellation and prompt cleanup.

use super::{Progress, SingleResult, TaskItem};
use crate::agents::AgentConfig;
use maestro_extensions_wasm::{
    AbortSignal, ExtensionAPI, ExtensionResult, Subscription,
    child_process::{ChildProcess, SpawnOptions, Stdio},
    streams::Readable,
    tools::path_utils::{guest_path, make_temp_dir, resolve},
};
use maestro_json::jsonl::JsonlLineSplitter;
use serde_json::Value;
use std::rc::Rc;

/// Host capabilities and directory observations shared by one tool invocation.
pub(crate) struct Run {
    /// Installed guest facade.
    pub api: ExtensionAPI,
    /// Session directory used when a task omits its cwd.
    pub cwd: String,
    /// Freshly discovered definitions.
    pub agents: Vec<AgentConfig>,
    /// Cancellation shared by every owned child.
    pub signal: Option<AbortSignal>,
}

/// Removes private prompt material on every return or dropped future.
struct Prompt {
    /// Native filename passed to the child.
    native: String,
    /// Component filename used only at filesystem calls.
    file: String,
    /// Component directory used for cleanup.
    directory: String,
}
impl Drop for Prompt {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.file);
        let _ = std::fs::remove_dir(&self.directory);
    }
}

/// Retains the child until output drains and removes the caller's abort observer.
struct OwnedChild {
    /// Original native controls, also signalled when a future is dropped early.
    child: Rc<dyn ChildProcess>,
    /// Explicit subscription whose ordinary drop would not remove the observer.
    subscription: Option<Subscription>,
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Some(subscription) = &self.subscription {
            let _ = subscription.unsubscribe();
        }
        let _ = self.child.kill(None);
    }
}

/// Creates a private role file through the existing native mutation queue.
async fn prompt(api: &ExtensionAPI, agent: &AgentConfig) -> ExtensionResult<Option<Prompt>> {
    if agent.system_prompt.trim().is_empty() {
        return Ok(None);
    }
    let directory = make_temp_dir(api, "maestro-subagent-")?;
    let native = resolve(api, &[&directory, "prompt.md"])?;
    let prompt = Prompt {
        file: guest_path(api, &native)?,
        directory: guest_path(api, &directory)?,
        native,
    };
    let (file, body) = (prompt.file.clone(), agent.system_prompt.clone());
    api.with_file_mutation_queue(prompt.native.clone(), move || {
        Box::pin(async move { std::fs::write(file, body).map_err(|error| error.to_string()) })
    })
    .await?;
    Ok(Some(prompt))
}

impl Run {
    /// Runs a known definition or returns a diagnostic without spawning an unknown name.
    pub async fn process_child(
        &self,
        task: &TaskItem,
        step: Option<usize>,
        progress: &Progress,
        index: usize,
    ) -> ExtensionResult<SingleResult> {
        let mut result = SingleResult::pending(task);
        result.step = step;
        let Some(agent) = self.agents.iter().find(|agent| agent.name == task.agent) else {
            let names = self
                .agents
                .iter()
                .map(|agent| format!("\"{}\"", agent.name))
                .collect::<Vec<_>>()
                .join(", ");
            result.exit_code = 1;
            result.stderr = format!(
                "Unknown agent: \"{}\". Available agents: {}.",
                task.agent,
                super::fallback(&names, "none")
            );
            return Ok(result);
        };
        result.agent_source.clone_from(&agent.source);
        result.model.clone_from(&agent.model);
        let role = prompt(&self.api, agent).await?;
        let mut args = vec![
            "--mode".into(),
            "json".into(),
            "-p".into(),
            "--no-session".into(),
        ];
        if let Some(model) = &agent.model {
            args.extend(["--model".into(), model.clone()]);
        }
        if let Some(tools) = &agent.tools {
            args.extend(["--tools".into(), tools.join(",")]);
        }
        if let Some(role) = &role {
            args.extend(["--append-system-prompt".into(), role.native.clone()]);
        }
        args.push(format!("Task: {}", task.task));
        let command = std::env::var("MAESTRO_BIN_PATH")
            .ok()
            .filter(|path| !path.is_empty())
            .unwrap_or_else(|| "maestro".into());
        let options = SpawnOptions {
            cwd: Some(task.cwd.as_ref().unwrap_or(&self.cwd).clone()),
            stdio: [Stdio::Ignore, Stdio::Pipe, Stdio::Pipe],
            ..SpawnOptions::default()
        };
        let io = self.api.native_io();
        match io.spawn(command, args, options).await {
            Ok(child) => self.collect(child, &mut result, progress, index).await?,
            Err(error) => {
                result.stderr = error;
                result.exit_code = 1;
            }
        }
        Ok(result)
    }

    /// Binds cancellation before concurrently draining both output descriptors.
    async fn collect(
        &self,
        child: Rc<dyn ChildProcess>,
        result: &mut SingleResult,
        progress: &Progress,
        index: usize,
    ) -> ExtensionResult<()> {
        let mut owned = OwnedChild {
            child: Rc::clone(&child),
            subscription: None,
        };
        if let Some(signal) = &self.signal {
            let observed = Rc::clone(&child);
            owned.subscription =
                Some(signal.subscribe(Rc::new(move |_| observed.kill(None).map(|_| ())))?);
            if signal.aborted() {
                child.kill(None)?;
            }
        }
        let stdout = child.stdout().ok_or("Subagent has no stdout pipe")?;
        let stderr = child.stderr().ok_or("Subagent has no stderr pipe")?;
        let ((), stderr) = futures_util::future::try_join(
            read_output(stdout, result, progress, index),
            read_errors(stderr),
        )
        .await?;
        result.stderr = stderr;
        result.exit_code = child.wait().await?.code.unwrap_or(0);
        if self.signal.as_ref().is_some_and(AbortSignal::aborted) {
            return Err("Subagent was aborted".into());
        }
        Ok(())
    }
}

/// Frames bytes once, including a final unterminated line and split UTF-8 sequences.
async fn read_output(
    pipe: Rc<dyn Readable>,
    result: &mut SingleResult,
    progress: &Progress,
    index: usize,
) -> ExtensionResult<()> {
    let mut lines = JsonlLineSplitter::default();
    while let Some(bytes) = pipe.read().await? {
        for line in lines.push(&bytes) {
            observe(&line, result, progress, index)?;
        }
    }
    if let Some(line) = lines.finish() {
        observe(&line, result, progress, index)?;
    }
    Ok(())
}

/// Retains stderr without corrupting multibyte characters split across reads.
async fn read_errors(pipe: Rc<dyn Readable>) -> ExtensionResult<String> {
    let mut bytes = Vec::new();
    while let Some(chunk) = pipe.read().await? {
        bytes.extend(chunk);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Ignores malformed lines and records the two message-bearing child event kinds.
fn observe(
    line: &str,
    result: &mut SingleResult,
    progress: &Progress,
    index: usize,
) -> ExtensionResult<()> {
    let Ok(event) = serde_json::from_str::<Value>(line) else {
        return Ok(());
    };
    if !matches!(
        event["type"].as_str(),
        Some("message_end" | "tool_result_end")
    ) {
        return Ok(());
    }
    let Some(message) = event
        .get("message")
        .filter(|message| maestro_json::is_truthy(message))
    else {
        return Ok(());
    };
    if message["role"] == "assistant" {
        assistant(result, message);
    }
    result.messages.push(message.clone());
    progress.report(index, result)
}

/// Accumulates assistant usage and keeps the last supplied error and context observations.
fn assistant(result: &mut SingleResult, message: &Value) {
    result.usage.turns += 1;
    if let Some(usage) = message
        .get("usage")
        .filter(|usage| maestro_json::is_truthy(usage))
    {
        result.usage.input += number(usage, "input");
        result.usage.output += number(usage, "output");
        result.usage.cache_read += number(usage, "cacheRead");
        result.usage.cache_write += number(usage, "cacheWrite");
        result.usage.context_tokens = number(usage, "totalTokens");
        result.usage.cost += usage["cost"]["total"].as_f64().unwrap_or(0.0);
    }
    if result.model.is_none() {
        result.model = message["model"].as_str().map(str::to_owned);
    }
    if let Some(reason) = message["stopReason"].as_str() {
        result.stop_reason = Some(reason.into());
    }
    if let Some(error) = message["errorMessage"].as_str() {
        result.error_message = Some(error.into());
    }
}

/// Reads a native token count, treating absent usage fields as zero.
fn number(usage: &Value, key: &str) -> u64 {
    usage[key].as_u64().unwrap_or(0)
}
