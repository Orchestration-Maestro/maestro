//! Registration and invocation of the authored-agent tool.

use crate::{
    agents::{self, AgentConfig, AgentScope},
    runs::{
        Progress, SingleResult, SubagentDetails, TaskItem, fallback, process_child::Run,
        tool_result,
    },
};
use futures_util::{StreamExt, TryStreamExt};
use maestro_extensions_wasm::{
    AbortSignal, AgentToolResult, AgentToolUpdateCallback, ExtensionAPI, ExtensionContext,
    ExtensionResult, Presence, ToolDefinition, ToolMetadata,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// Optional invocation modes and directory trust controls.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Params {
    /// Single-mode agent.
    agent: Option<String>,
    /// Single-mode instructions.
    task: Option<String>,
    /// Single-mode directory override.
    cwd: Option<String>,
    /// Concurrent invocations.
    #[serde(default)]
    tasks: Vec<TaskItem>,
    /// Ordered invocations with prior output substitution.
    #[serde(default)]
    chain: Vec<TaskItem>,
    /// User-only by default.
    #[serde(default)]
    agent_scope: AgentScope,
    /// Interactive project-agent approval is enabled by default.
    confirm_project_agents: Option<bool>,
}

/// Builds the tool over the supplied host capabilities.
pub(crate) fn tool(api: ExtensionAPI) -> ToolDefinition {
    let render_api = api.clone();
    ToolDefinition {
        metadata: ToolMetadata {
            name: "subagent".into(), label: "Subagent".into(),
            description: "Delegate tasks to specialized subagents with isolated context. Modes: single (agent + task), parallel (tasks array), chain (sequential with {previous} placeholder). Default agent scope is user, from the agent directory's agents folder. Set agentScope to both or project to include project-local .maestro/agents definitions.".into(),
            parameters: schema().into(), prompt_snippet: Presence::Missing,
            prompt_guidelines: Presence::Missing, render_shell: Presence::Missing, execution_mode: Presence::Missing,
        },
        prepare_arguments: None,
        render_call: Some(Rc::new(|args, theme, _, _| crate::tui::render::call(args.as_ref(), &theme))),
        render_result: Some(Rc::new(move |result, options, theme, _, _| crate::tui::render::result(&result, options, &theme, &render_api))),
        execute: Rc::new(move |_, params, signal, update, ctx| {
            let api = api.clone();
            Box::pin(async move { execute(api, params, signal, update, ctx).await })
        }),
    }
}

/// Keeps all three invocation modes in one model-facing parameter contract.
fn schema() -> Value {
    let item = json!({"type":"object","required":["agent","task"],"properties":{
        "agent":{"type":"string","description":"Name of the agent to invoke"},
        "task":{"type":"string","description":"Task to delegate; chain tasks accept {previous}"},
        "cwd":{"type":"string","description":"Working directory for the agent process"}}});
    json!({"type":"object","properties":{
        "agent":{"type":"string"},"task":{"type":"string"},"cwd":{"type":"string"},
        "tasks":{"type":"array","items":item},"chain":{"type":"array","items":item},
        "agentScope":{"type":"string","enum":["user","project","both"],"default":"user"},
        "confirmProjectAgents":{"type":"boolean","default":true}}})
}

/// Discovers current definitions before validating or executing one invocation mode.
async fn execute(
    api: ExtensionAPI,
    params: Value,
    signal: Option<AbortSignal>,
    update: Option<AgentToolUpdateCallback>,
    ctx: ExtensionContext,
) -> ExtensionResult<AgentToolResult> {
    let params: Params = serde_json::from_value(params).map_err(|error| error.to_string())?;
    let cwd = ctx.cwd()?;
    let discovery = agents::discover_agents(&api, &cwd, params.agent_scope)?;
    let mut details = SubagentDetails {
        mode: "single".into(),
        agent_scope: params.agent_scope,
        project_agents_dir: discovery.project_agents_dir,
        results: Vec::new(),
    };
    let Some((mode, tasks)) = selection(&params) else {
        return invalid(&discovery.agents, &details);
    };
    details.mode = mode.into();
    if !approved(
        &ctx,
        &params,
        &discovery.agents,
        details.project_agents_dir.as_deref(),
    )
    .await?
    {
        return tool_result(
            "Canceled: project-local agents not approved.".into(),
            &details,
        );
    }
    if mode == "parallel" && tasks.len() > 8 {
        return tool_result(
            format!("Too many parallel tasks ({}). Max is 8.", tasks.len()),
            &details,
        );
    }
    if mode == "parallel" {
        details.results = tasks.iter().map(SingleResult::pending).collect();
    }
    let progress = Progress {
        details: Rc::new(RefCell::new(details)),
        update,
    };
    let run = Run {
        api,
        cwd,
        agents: discovery.agents,
        signal,
    };
    match mode {
        "chain" => chain(&run, &tasks, &progress).await,
        "parallel" => parallel(&run, &tasks, &progress).await,
        _ => single(&run, &tasks[0], &progress).await,
    }
}

/// Requires exactly one nonempty mode, preserving empty-array behaviour.
fn selection(params: &Params) -> Option<(&'static str, Vec<TaskItem>)> {
    let single = params
        .agent
        .as_ref()
        .filter(|name| !name.is_empty())
        .zip(params.task.as_ref().filter(|task| !task.is_empty()));
    let modes = u8::from(single.is_some())
        + u8::from(!params.tasks.is_empty())
        + u8::from(!params.chain.is_empty());
    if modes != 1 {
        return None;
    }
    if !params.chain.is_empty() {
        return Some(("chain", params.chain.clone()));
    }
    if !params.tasks.is_empty() {
        return Some(("parallel", params.tasks.clone()));
    }
    single.map(|(agent, task)| {
        (
            "single",
            vec![TaskItem {
                agent: agent.clone(),
                task: task.clone(),
                cwd: params.cwd.clone(),
            }],
        )
    })
}

/// Asks only for project definitions actually requested in an interactive invocation.
async fn approved(
    ctx: &ExtensionContext,
    params: &Params,
    agents: &[AgentConfig],
    directory: Option<&str>,
) -> ExtensionResult<bool> {
    if matches!(params.agent_scope, AgentScope::User)
        || !params.confirm_project_agents.unwrap_or(true)
        || !ctx.has_ui()?
    {
        return Ok(true);
    }
    let mut names = Vec::new();
    for name in params
        .chain
        .iter()
        .chain(&params.tasks)
        .map(|task| &task.agent)
        .chain(params.agent.iter())
    {
        if !names.contains(name)
            && agents
                .iter()
                .any(|agent| agent.name == *name && agent.source == "project")
        {
            names.push(name.clone());
        }
    }
    if names.is_empty() {
        return Ok(true);
    }
    ctx.ui()?.confirm("Run project-local agents?".into(), format!("Agents: {}\nSource: {}\n\nProject agents are repo-controlled. Only continue for trusted repositories.", names.join(", "), directory.unwrap_or("(unknown)")), None).await
}

/// Returns one child's output while retaining rich failure details.
async fn single(
    run: &Run,
    task: &TaskItem,
    progress: &Progress,
) -> ExtensionResult<AgentToolResult> {
    let result = run.process_child(task, None, progress, 0).await?;
    progress.report(0, &result)?;
    let text = if result.failed() {
        format!(
            "Agent {}: {}",
            result.stop_reason.as_deref().unwrap_or("failed"),
            result.error()
        )
    } else {
        fallback(result.output(), "(no output)").into()
    };
    tool_result(text, &progress.details.borrow())
}

/// Stops a chain at its first failure and substitutes every prior-output placeholder.
async fn chain(
    run: &Run,
    tasks: &[TaskItem],
    progress: &Progress,
) -> ExtensionResult<AgentToolResult> {
    let mut previous = String::new();
    for (index, task) in tasks.iter().enumerate() {
        let task = TaskItem {
            task: task.task.replace("{previous}", &previous),
            ..task.clone()
        };
        let result = run
            .process_child(&task, Some(index + 1), progress, index)
            .await?;
        progress.report(index, &result)?;
        if result.failed() {
            return tool_result(
                format!(
                    "Chain stopped at step {} ({}): {}",
                    index + 1,
                    task.agent,
                    result.error()
                ),
                &progress.details.borrow(),
            );
        }
        previous = result.output().into();
    }
    tool_result(
        fallback(&previous, "(no output)").into(),
        &progress.details.borrow(),
    )
}

/// Reuses the futures stream's bounded concurrency without a separate scheduler.
async fn parallel(
    run: &Run,
    tasks: &[TaskItem],
    progress: &Progress,
) -> ExtensionResult<AgentToolResult> {
    futures_util::stream::iter(tasks.iter().enumerate())
        .map(|(index, task)| async move {
            let result = run.process_child(task, None, progress, index).await?;
            progress.report(index, &result)
        })
        .buffer_unordered(4)
        .try_collect::<Vec<_>>()
        .await?;
    let details = progress.details.borrow();
    let successes = details
        .results
        .iter()
        .filter(|result| !result.failed())
        .count();
    let summaries = details
        .results
        .iter()
        .map(|result| {
            format!(
                "[{}] {}: {}",
                result.agent,
                if result.failed() {
                    "failed"
                } else {
                    "completed"
                },
                fallback(&preview(result.output(), 100), "(no output)")
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    tool_result(
        format!(
            "Parallel: {successes}/{} succeeded\n\n{summaries}",
            tasks.len()
        ),
        &details,
    )
}

/// Clips previews at whole Unicode scalars rather than slicing encoded text.
pub(crate) fn preview(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let mut result: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        result.push_str("...");
    }
    result
}

/// Lists the current names when the caller did not select exactly one mode.
fn invalid(agents: &[AgentConfig], details: &SubagentDetails) -> ExtensionResult<AgentToolResult> {
    let available = agents
        .iter()
        .map(|agent| format!("{} ({})", agent.name, agent.source))
        .collect::<Vec<_>>()
        .join(", ");
    tool_result(
        format!(
            "Invalid parameters. Provide exactly one mode.\nAvailable agents: {}",
            fallback(&available, "none")
        ),
        details,
    )
}
