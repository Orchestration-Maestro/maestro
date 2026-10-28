//! A command that selects static, animated, hidden or default working indicators.
// maestro-component: MAESTRO_WORKING_INDICATOR_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::Cell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionFuture, ExtensionResult, ExtensionUIContext,
    NotificationType, WorkingIndicatorOptions,
};

/// The custom spinner's frames, in display order.
const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// The foreground colours cycled by the custom indicators.
const PASTEL_RAINBOW: [&str; 6] = [
    "\x1b[38;2;255;179;186m",
    "\x1b[38;2;255;223;186m",
    "\x1b[38;2;255;255;186m",
    "\x1b[38;2;186;255;201m",
    "\x1b[38;2;186;225;255m",
    "\x1b[38;2;218;186;255m",
];

/// The indicator selection retained by one loaded factory.
#[derive(Clone, Copy)]
enum Mode {
    /// A single coloured dot.
    Dot,
    /// No inline indicator.
    None,
    /// A four-frame pulse.
    Pulse,
    /// A ten-frame coloured spinner.
    Spinner,
    /// The host's unmodified spinner.
    Default,
}

impl Mode {
    /// The text shown in notifications and status.
    fn description(self) -> &'static str {
        match self {
            Self::Dot => "static dot",
            Self::None => "hidden",
            Self::Pulse => "custom pulse",
            Self::Spinner => "custom spinner",
            Self::Default => "Maestro default spinner",
        }
    }

    /// The frame configuration requested from the host, or its default.
    fn indicator(self) -> Option<WorkingIndicatorOptions> {
        let (frames, interval_ms) = match self {
            Self::Dot => (vec![colorize("●", 0)], None),
            Self::None => (vec![], None),
            Self::Pulse => (
                vec![
                    colorize("·", 0),
                    colorize("•", 2),
                    colorize("●", 4),
                    colorize("•", 5),
                ],
                Some(120.0),
            ),
            Self::Spinner => (
                SPINNER_FRAMES
                    .iter()
                    .enumerate()
                    .map(|(index, frame)| colorize(frame, index))
                    .collect(),
                Some(80.0),
            ),
            Self::Default => return None,
        };
        Some(WorkingIndicatorOptions {
            frames: Some(frames),
            interval_ms,
        })
    }

    /// Requests the indicator before publishing its themed status.
    fn apply(self, ui: &ExtensionUIContext) -> ExtensionResult<()> {
        ui.set_working_indicator(self.indicator())?;
        let status = ui
            .theme()?
            .fg("dim", &format!("Indicator: {}", self.description()))?;
        ui.set_status("working-indicator", Some(&status))
    }
}

/// Colours one frame, restoring only the foreground afterwards.
fn colorize(frame: &str, index: usize) -> String {
    format!(
        "{}{frame}\x1b[39m",
        PASTEL_RAINBOW[index % PASTEL_RAINBOW.len()]
    )
}

/// Registers a startup handler and the command sharing its indicator selection.
///
/// # Errors
/// Returns a host registration refusal unchanged.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let mode = Rc::new(Cell::new(Mode::Spinner));
        let startup = Rc::clone(&mode);
        api.on(
            "session_start",
            Rc::new(move |_event, ctx| {
                let mode = startup.get();
                Box::pin(async move {
                    mode.apply(&ctx.ui()?)?;
                    Ok(None)
                })
            }),
        )?;
        api.register_command("working-indicator", command(mode))
    })
}

/// Selects a mode, queries it, or reports the command's usage without changing it.
fn command(mode: Rc<Cell<Mode>>) -> CommandOptions {
    CommandOptions {
        description: Some(
            "Set the streaming working indicator: dot, pulse, none, spinner, or reset.".into(),
        ),
        get_argument_completions: None,
        handler: Rc::new(move |args, ctx| {
            let mode = Rc::clone(&mode);
            Box::pin(async move {
                let ui = ctx.ui()?;
                let next = match args.trim().to_lowercase().as_str() {
                    "" => {
                        return ui.notify(
                            &format!("Working indicator: {}", mode.get().description()),
                            Some(NotificationType::Info),
                        );
                    }
                    "dot" => Mode::Dot,
                    "none" => Mode::None,
                    "pulse" => Mode::Pulse,
                    "spinner" => Mode::Spinner,
                    "reset" => Mode::Default,
                    _ => {
                        return ui.notify(
                            "Usage: /working-indicator [dot|pulse|none|spinner|reset]",
                            Some(NotificationType::Error),
                        );
                    }
                };
                mode.set(next);
                next.apply(&ui)?;
                ui.notify(
                    &format!("Working indicator set to: {}", next.description()),
                    Some(NotificationType::Info),
                )
            })
        }),
    }
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
