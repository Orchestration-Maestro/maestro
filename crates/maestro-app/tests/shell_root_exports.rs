//! Native shell operations are available from the application root.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_app::{BashOperations, create_local_bash_operations, get_shell_config};
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
