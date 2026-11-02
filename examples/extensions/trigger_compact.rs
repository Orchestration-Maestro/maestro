//! A turn hook that requests compaction above a usage threshold and a command that requests it immediately.
// maestro-component: MAESTRO_TRIGGER_COMPACT_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::Cell;
use std::rc::Rc;

use maestro_extensions_wasm::{
    CommandOptions, CompactOptions, ExtensionAPI, ExtensionContext, ExtensionFuture,
    ExtensionResult, NotificationType,
};

/// Context size a known estimate must cross for the hook to request compaction.
const COMPACT_THRESHOLD_TOKENS: f64 = 100_000.0;

/// Requests compaction and reports its outcome through the UI the context has at that moment.
fn trigger_compaction(
    ctx: &ExtensionContext,
    custom_instructions: Option<String>,
) -> ExtensionResult<()> {
    if ctx.has_ui()? {
        ctx.ui()?
            .notify("Compaction started", Some(NotificationType::Info))?;
    }
    let complete = ctx.clone();
    let failed = ctx.clone();
    ctx.compact(Some(CompactOptions {
        custom_instructions,
        on_complete: Some(Rc::new(move |_, _| {
            if complete.has_ui()? {
                complete
                    .ui()?
                    .notify("Compaction completed", Some(NotificationType::Info))?;
            }
            Ok(())
        })),
        on_error: Some(Rc::new(move |message, _| {
            if failed.has_ui()? {
                failed.ui()?.notify(
                    &format!("Compaction failed: {message}"),
                    Some(NotificationType::Error),
                )?;
            }
            Ok(())
        })),
    }))
}

/// Records one turn's estimate and requests compaction once it crosses the threshold.
fn turn_end(previous_tokens: &Cell<Option<f64>>, ctx: &ExtensionContext) -> ExtensionResult<()> {
    let Some(tokens) = ctx.get_context_usage()?.and_then(|usage| usage.tokens) else {
        return Ok(());
    };
    let crossed = previous_tokens
        .get()
        .is_some_and(|previous| previous <= COMPACT_THRESHOLD_TOKENS);
    previous_tokens.set(Some(tokens));
    if crossed && tokens > COMPACT_THRESHOLD_TOKENS {
        trigger_compaction(ctx, None)?;
    }
    Ok(())
}

/// Registers the threshold hook and the immediate command.
#[must_use]
pub fn trigger_compact(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let previous_tokens = Rc::new(Cell::new(None));
        api.on(
            "turn_end",
            Rc::new(move |_, ctx| {
                let previous_tokens = previous_tokens.clone();
                Box::pin(async move {
                    turn_end(&previous_tokens, &ctx)?;
                    Ok(None)
                })
            }),
        )?;
        api.register_command(
            "trigger-compact",
            CommandOptions {
                description: Some("Trigger compaction immediately".into()),
                get_argument_completions: None,
                handler: Rc::new(|args, ctx| {
                    Box::pin(async move {
                        let instructions = args.trim();
                        let custom = (!instructions.is_empty()).then(|| instructions.to_owned());
                        trigger_compaction(&ctx, custom)
                    })
                }),
            },
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(trigger_compact);
