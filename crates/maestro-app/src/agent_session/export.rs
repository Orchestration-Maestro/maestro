//! HTML and JSONL exports of the session.
use super::{AgentSession, state::thrown};
use crate::utils::lock::read;
use chrono::{SecondsFormat, Utc};
use maestro_agent::agent_loop::tool_definitions;
use maestro_export::{
    ExportOperations, ExportOptions, ExportState, ToolHtmlRendererDeps, create_tool_html_renderer,
    export_session_to_html,
};
use maestro_extensions::ExtensionExecution;
use maestro_models::{DiagnosticErrorInfo, Local, Tool};
use maestro_session::{CURRENT_SESSION_VERSION, session_manager::ReadonlySessionManager};
use maestro_tools::ToolRenderEnvironment;
use serde_json::{Value, json};
use std::rc::Rc;

impl<E: ExtensionExecution> AgentSession<E> {
    /// Writes the session as an HTML document with the agent's prompt and tools, the saved
    /// theme and custom tool rendering; returns the written path.
    ///
    /// Renderers receive the live theme handle: one that reads it without a published theme
    /// keeps the standard rendering, and one that never reads it still draws.
    ///
    /// # Errors
    /// Returns the export failure, such as for a session only in memory.
    pub fn export_to_html(
        &self,
        output_path: Option<&str>,
        operations: &dyn ExportOperations,
        environment: Rc<ToolRenderEnvironment>,
    ) -> Result<String, DiagnosticErrorInfo> {
        let theme_name = self.settings_manager.borrow().get_theme();
        let (system_prompt, tools) = self.export_state()?;
        let manager = self.session_manager.borrow();
        let renderer = create_tool_html_renderer(ToolHtmlRendererDeps {
            get_tool_definition: |name: &str| {
                self.get_tool_definition(name)
                    .map(|tool| tool.renderer_binding())
                    .transpose()
            },
            theme: environment.live_theme.clone(),
            cwd: manager.get_cwd(),
            width: None,
            environment,
        });
        export_session_to_html(
            &*manager,
            Some(ExportState {
                system_prompt: Some(&system_prompt),
                tools: Some(&tools),
            }),
            Some(ExportOptions {
                output_path,
                theme_name: theme_name.as_deref(),
                tool_renderer: Some(&renderer),
            }),
            operations,
        )
        // An export failure may hold a renderer failure that cannot cross threads, so it is
        // reported by its text.
        .map_err(|error| thrown(error.to_string()))
    }

    /// The prompt and resolved tool declarations an export records, read without holding
    /// the agent state while descriptions resolve.
    fn export_state(&self) -> Result<(String, Vec<Tool>), DiagnosticErrorInfo> {
        let (system_prompt, tools) = {
            let state = read(self.agent.state());
            (state.system_prompt.clone(), Rc::clone(state.tools()))
        };
        Ok((system_prompt, tool_definitions::<Local>(&tools)?))
    }

    /// Writes a fresh header and the current branch, chained in order, as JSONL; returns
    /// the resolved path.
    ///
    /// # Errors
    /// Returns a working-directory, encoding or write failure.
    pub fn export_to_jsonl(
        &self,
        output_path: Option<&str>,
    ) -> Result<String, DiagnosticErrorInfo> {
        let manager = self.session_manager.borrow();
        let header = json!({"type": "session", "version": CURRENT_SESSION_VERSION, "id": manager.get_session_id(), "timestamp": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true), "cwd": manager.get_cwd()});
        let mut text = maestro_json::jsonl::serialize_json_line(&header).map_err(thrown)?;
        let mut previous = Value::Null;
        for entry in manager.get_branch(None).map_err(thrown)? {
            let mut linear = entry.clone();
            linear["parentId"] = previous;
            previous = entry.get("id").cloned().unwrap_or(Value::Null);
            text.push_str(&maestro_json::jsonl::serialize_json_line(&linear).map_err(thrown)?);
        }
        drop(manager);
        let stamp = Utc::now()
            .to_rfc3339_opts(SecondsFormat::Millis, true)
            .replace([':', '.'], "-");
        let default_name = format!("session-{stamp}.jsonl");
        let path = maestro_path::resolve_with(
            &[&[output_path.unwrap_or(&default_name)]],
            maestro_resources::drive_directories,
            maestro_resources::current_directory,
        )
        .map_err(thrown)?
        .concat();
        std::fs::create_dir_all(maestro_path::dirname(&path)).map_err(thrown)?;
        std::fs::write(&path, text).map_err(thrown)?;
        Ok(path)
    }
}
