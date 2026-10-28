//! The concrete terminal and standard-stream presentations.
use maestro_app::{
    AgentSessionRuntime, ExportOperations, RpcModeOptions, RpcSessionOptions,
    ToolRenderEnvironment,
    presentation_data::mode_dispatch::{
        InteractiveModeOptions, InteractiveModeRunner, ModeLaunchers,
    },
};
use maestro_theme::{SyntectHighlighter, ThemeState};
use std::{error::Error, rc::Rc};
use tokio::task::LocalSet;

/// Construct the delivered interactive and RPC adapters only when startup selects them.
pub fn launchers(
    local: Rc<LocalSet>,
    themes: &'static Rc<ThemeState>,
    export: Rc<dyn ExportOperations>,
) -> ModeLaunchers {
    let interactive_local = Rc::clone(&local);
    ModeLaunchers {
        interactive: Rc::new(move |runtime, options| {
            interactive(
                runtime,
                options,
                Rc::clone(&interactive_local),
                Rc::clone(themes),
            )
        }),
        rpc: Rc::new(move |runtime| {
            let (local, export) = (Rc::clone(&local), Rc::clone(&export));
            Box::pin(async move {
                let options = RpcModeOptions {
                    workspace: None,
                    session: Some(RpcSessionOptions {
                        runtime,
                        process: Rc::new(maestro_rpc::NativeRpcProcess),
                        theme: themes.theme(),
                        export_operations: export,
                        render_environment: Rc::new(ToolRenderEnvironment {
                            runtime: Rc::new(maestro_tui_crossterm::ProcessTuiRuntime::new(local)),
                            readme_path: maestro_app::configuration::config::get_readme_path()?,
                            live_theme: themes.theme(),
                            highlighter: Rc::new(
                                SyntectHighlighter::new()
                                    .map_err(|error| error as Box<dyn Error>)?,
                            ),
                            terminal_image: maestro_tui::TerminalImage::default(),
                        }),
                    }),
                };
                match maestro_rpc::run_rpc_mode(options).await {
                    Ok(never) => match never {},
                    Err(error) => Err(error.into()),
                }
            })
        }),
    }
}

/// Wire the native terminal and footer effects to the interactive frontend.
fn interactive(
    runtime: Rc<AgentSessionRuntime>,
    options: InteractiveModeOptions,
    local: Rc<LocalSet>,
    themes: Rc<ThemeState>,
) -> Result<Rc<dyn InteractiveModeRunner>, Box<dyn Error>> {
    Ok(maestro_chat::InteractiveMode::new(
        runtime,
        options,
        maestro_cli::input::selector::native_terminal(Rc::clone(&local)),
        themes,
        Rc::new(
            maestro_app::presentation_data::footer_data_provider::NativeFooterOperations::new(
                local,
            ),
        ),
    )?)
}
