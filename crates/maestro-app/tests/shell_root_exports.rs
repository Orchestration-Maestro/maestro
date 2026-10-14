//! Native shell operations are available from the application root.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_app::{
    BashOperations, create_bash_tool_definition, create_local_bash_operations, get_shell_config,
};
use std::sync::Arc;

#[test]
fn shell_operations_are_available_from_application_root() {
    let config = get_shell_config(Some(".")).unwrap();
    assert_eq!(config.shell, ".");
    assert_eq!(config.args, ["-c"]);
    let _: Arc<dyn BashOperations> =
        create_local_bash_operations(String::new(), Some(config.shell));
}

/// Supplied execution reached through the root factory, not an application wrapper.
struct Supplied;
impl BashOperations for Supplied {
    fn exec<'a>(
        &'a self,
        command: &'a str,
        cwd: &'a str,
        options: maestro_tools::BashExecOptions<'a>,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<maestro_tools::BashExecResult, maestro_tools::BashError>,
                > + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            assert_eq!(command, "root command");
            assert_eq!(cwd, "/root");
            assert_eq!(options.timeout, Some(0.25));
            assert!(options.env.is_some());
            (options.on_data)(b"root output");
            Ok(maestro_tools::BashExecResult {
                exit_code: Some(0.0),
            })
        })
    }
}

#[test]
fn bash_factories_are_available_from_application_root() {
    use maestro_app::{
        BashSpawnContext, BashSpawnHook, BashToolDetails, BashToolOptions, create_bash_tool,
        create_bash_tool_definition,
    };
    let hook: BashSpawnHook = Arc::new(|context: BashSpawnContext| Ok(context));
    let options = || BashToolOptions {
        operations: Some(Arc::new(Supplied)),
        spawn_hook: Some(hook.clone()),
        ..BashToolOptions::default()
    };
    let definition =
        create_bash_tool_definition::<()>("/root".into(), "/root-bin".into(), options());
    assert_eq!(
        definition.definition.parameters["required"],
        serde_json::json!(["command"])
    );
    let details = BashToolDetails {
        truncation: None,
        full_output_path: None,
    };
    assert_eq!(
        serde_json::to_value(details).unwrap(),
        serde_json::json!({})
    );
    #[cfg(unix)]
    {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = runtime
            .block_on((definition.execute)(
                "call".into(),
                maestro_app::BashToolInput {
                    command: "root command".into(),
                    timeout: Some(0.25),
                },
                None,
                None,
                None,
            ))
            .unwrap();
        assert_root_result((result.content, result.details));
        let wrapped = create_bash_tool("/root".into(), "/root-bin".into(), options());
        let result = runtime
            .block_on((wrapped.execute)(
                "call".into(),
                serde_json::json!({"command":"root command", "timeout":0.25}),
                None,
                None,
            ))
            .unwrap();
        assert_root_result((result.content, result.details));
    }
}

/// Check either factory's content with one shared assertion.
#[cfg(unix)]
fn assert_root_result<D>(result: (Vec<maestro_models::UserBlock>, Option<D>)) {
    assert!(result.1.is_none());
    assert_eq!(
        serde_json::to_value(result.0).unwrap(),
        serde_json::json!([{ "type":"text", "text":"root output" }])
    );
}

#[test]
fn bash_rendering_exports_share_the_domain_owners() {
    use maestro_app::{
        BashRenderState, BashToolDetails, BashToolInput, BashToolOptions, VisualTruncateResult,
        create_bash_tool_definition, truncate_to_visual_lines,
    };
    let definition: maestro_tools::ToolDefinition<
        (),
        BashToolInput,
        BashToolDetails,
        BashRenderState,
    > = create_bash_tool_definition(String::new(), String::new(), BashToolOptions::default());
    assert!(definition.render_call.is_some());
    assert!(definition.render_result.is_some());
    let state = BashRenderState::default();
    let _: maestro_tools::BashRenderState = state;
    let result: VisualTruncateResult = truncate_to_visual_lines("a\nb\nc", 2, 4, None);
    assert_eq!(result.visual_lines, ["b   ", "c   "]);
    assert_eq!(result.skipped_count, 1);
    let _: maestro_tools::VisualTruncateResult = result;
}

#[test]
fn renderer_bindings_are_available_from_application_root() {
    use maestro_app::{
        EditRenderState, ToolDef, ToolDefinition, ToolRenderCall, ToolRenderContext,
        ToolRenderEnvironment, ToolRenderPhase, ToolRenderResult, ToolRenderResultCallback,
        ToolRenderResultLike, ToolRenderResultOptions, ToolRenderShell, ToolRendererBinding,
        ToolRendererState, get_text_output,
    };
    let state = ToolRendererState::default();
    assert_eq!(
        *state.state::<serde_json::Value>().borrow(),
        serde_json::json!({})
    );
    let _native = state.state::<EditRenderState>();
    let binding: ToolRendererBinding = ToolDef::Bash(create_bash_tool_definition::<()>(
        "/work".into(),
        "/unused".into(),
        maestro_app::BashToolOptions::default(),
    ))
    .renderer_binding()
    .unwrap();
    assert!(binding.render_call.is_some() && binding.render_result.is_some());
    let _: Option<ToolDef> = None;
    let _: Option<ToolDefinition> = None;
    let _: Option<ToolRenderCall<ToolRendererState>> = binding.render_call;
    let _: Option<ToolRenderResult<serde_json::Value, ToolRendererState>> = None;
    let _: Option<ToolRenderResultCallback<serde_json::Value, ToolRendererState>> =
        binding.render_result;
    let _: Option<ToolRenderContext<'_, ToolRendererState>> = None;
    let _: Option<ToolRenderEnvironment> = None;
    let _: Option<ToolRenderPhase> = None;
    let _: Option<ToolRenderResultOptions> = None;
    let _: Option<ToolRenderShell> = binding.render_shell;
    let result = ToolRenderResultLike::<serde_json::Value> {
        content: &[],
        details: None,
        terminate: None,
        is_error: None,
    };
    assert_eq!(
        get_text_output(Some(&result), false, &maestro_tui::TerminalImage::default()).unwrap(),
        ""
    );
}
