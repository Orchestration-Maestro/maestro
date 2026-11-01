//! The final startup handoff to a selected presentation.
use super::bootstrap::AppMode;
use crate::print::print_mode::{PrintMode, PrintModeOptions, run_print_mode};
use maestro_app::configuration::timings::Timings;
use maestro_app::presentation_data::mode_dispatch::{InteractiveModeOptions, ModeLaunchers};
use maestro_app::{AgentSessionRuntime, is_truthy_env_flag};
use maestro_theme::{Theme, ThemeState};
use std::{error::Error, io::Write, rc::Rc};

/// Startup inputs retained until the selected mode consumes them.
pub struct ModeDispatch {
    /// The already selected presentation.
    pub app_mode: AppMode,
    /// The observed benchmark flag, without trimming or normalization.
    pub startup_benchmark: Option<String>,
    /// The prompt inputs and existing presentation configuration.
    pub interactive_options: InteractiveModeOptions,
    /// The scope captured by startup before reporting diagnostics.
    pub scoped_models: Vec<maestro_app::ScopedModel>,
}
/// Caller-owned streams and timing observations.
pub struct DispatchOutput<'a> {
    /// Ordinary output, routed by the caller's existing output owner.
    pub stdout: &'a mut dyn Write,
    /// Diagnostic output.
    pub stderr: &'a mut dyn Write,
    /// The caller's existing timing samples.
    pub timings: &'a mut Timings,
    /// The same monotonic clock used for the prior samples.
    pub now: &'a dyn Fn() -> f64,
    /// Whether the selected streams support terminal styling.
    pub styles_enabled: bool,
}
/// Writes the visible model scope in its existing order.
fn report_scope(
    runtime: &AgentSessionRuntime,
    request: &ModeDispatch,
    output: &mut DispatchOutput<'_>,
) -> std::io::Result<()> {
    let session = runtime.session();
    let models = &request.scoped_models;
    if models.is_empty()
        || (!request.interactive_options.verbose
            && session.settings_manager().borrow().get_quiet_startup())
    {
        return Ok(());
    }
    let list = model_list(models);
    let hint = Theme::gray("(Ctrl+P to cycle)", output.styles_enabled);
    let line = Theme::dim(
        &format!("Model scope: {list} {hint}"),
        output.styles_enabled,
    );
    writeln!(output.stdout, "{line}")
}
/// Hands startup to the selected mode, returning its requested process status.
///
/// # Errors
/// Returns the selected mode or output error at its original phase.
pub async fn dispatch_mode(
    runtime: Rc<AgentSessionRuntime>,
    request: ModeDispatch,
    launchers: &ModeLaunchers,
    themes: &ThemeState,
    output: &mut DispatchOutput<'_>,
) -> Result<i32, Box<dyn Error>> {
    let benchmark = is_truthy_env_flag(request.startup_benchmark.as_deref());
    if benchmark && !request.app_mode.is_interactive() {
        let line = Theme::red(
            "Error: MAESTRO_STARTUP_BENCHMARK only supports interactive mode",
            output.styles_enabled,
        );
        writeln!(output.stderr, "{line}")?;
        return Ok(1);
    }
    match request.app_mode {
        AppMode::Rpc => {
            output.timings.print_timings(output.stderr)?;
            (launchers.rpc)(runtime).await?;
        }
        AppMode::Interactive => {
            report_scope(&runtime, &request, output)?;
            run_interactive(runtime, request, launchers, themes, output).await?;
        }
        AppMode::Print | AppMode::Json => {
            output.timings.print_timings(output.stderr)?;
            let options = request.interactive_options;
            let status = run_print_mode(
                runtime,
                PrintModeOptions {
                    mode: if request.app_mode == AppMode::Json {
                        PrintMode::Json
                    } else {
                        PrintMode::Text
                    },
                    messages: options.initial_messages,
                    initial_message: options.initial_message,
                    initial_images: options.initial_images,
                },
            )
            .await?;
            themes.stop_theme_watcher();
            maestro_app::output_guard::restore_stdout();
            return Ok(status);
        }
    }
    Ok(0)
}

/// Drives the selected interactive owner without adding cleanup phases.
async fn run_interactive(
    runtime: Rc<AgentSessionRuntime>,
    request: ModeDispatch,
    launchers: &ModeLaunchers,
    themes: &ThemeState,
    output: &mut DispatchOutput<'_>,
) -> Result<(), Box<dyn Error>> {
    let benchmark = is_truthy_env_flag(request.startup_benchmark.as_deref());
    let mode = (launchers.interactive)(runtime, request.interactive_options)?;
    if benchmark {
        Rc::clone(&mode).init().await?;
        output.timings.time("interactiveMode.init", output.now);
        output.timings.print_timings(output.stderr)?;
        mode.stop()?;
        themes.stop_theme_watcher();
        output.stdout.flush()?;
        output.stderr.flush()?;
    } else {
        output.timings.print_timings(output.stderr)?;
        mode.run().await?;
    }
    Ok(())
}

/// Formats the captured scope in one walk, keeping each read guard local.
fn model_list<'a>(models: impl IntoIterator<Item = &'a maestro_app::ScopedModel>) -> String {
    models
        .into_iter()
        .map(|scoped| {
            let model = scoped
                .model
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let thinking = scoped
                .thinking_level
                .as_ref()
                .map_or_else(String::new, |thinking| format!(":{thinking}"));
            format!("{}{thinking}", model.id)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests;
