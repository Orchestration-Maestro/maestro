//! Animates the terminal title while the agent works, restoring it when work stops.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::{cell::RefCell, future::poll_fn, rc::Rc, task::Waker, time::Duration};

use maestro_extensions_wasm::{
    EventListener, ExtensionAPI, ExtensionContext, ExtensionEventResult, ExtensionFuture,
    ExtensionResult, Subscription, tools::path_utils,
};

/// Frames in display order, one every 80 milliseconds.
const BRAILLE_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// The example's channel for handing periodic work to the host's event-tail executor.
const CHANNEL: &str = "maestro.example.titlebar-spinner";

/// One animation's owned future and the host task waiting to poll it again.
#[derive(Default)]
struct Animation {
    /// Dropping this future also drops its pending native wait.
    running: Option<ExtensionFuture<'static, ()>>,
    /// Wakes the host tail when the lifecycle handler cancels the animation.
    waker: Option<Waker>,
    /// Keeps another copy's channel emission from handing this animation off twice.
    claimed: bool,
}

/// Reads the current name and directory, without caching either between frames.
fn base_title(api: &ExtensionAPI, context: &ExtensionContext) -> ExtensionResult<String> {
    let basename = path_utils::basename(api, &context.cwd()?)?;
    // Matches maestro-chat/src/interactive_mode/events.rs:489-497 and configuration/config/mod.rs:24.
    Ok(
        match api.get_session_name()?.filter(|name| !name.is_empty()) {
            Some(name) => format!("Maestro - {name} - {basename}"),
            None => format!("Maestro - {basename}"),
        },
    )
}

/// Waits before each frame, leaving scheduling and timer disposal with native I/O.
fn animate(api: ExtensionAPI, context: ExtensionContext) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let io = api.native_io();
        for frame in BRAILLE_FRAMES.iter().cycle() {
            io.sleep(Duration::from_millis(80), None).await?;
            context
                .ui()?
                .set_title(&format!("{frame} {}", base_title(&api, &context)?))?;
        }
        Ok(())
    })
}

/// The animation retained by the lifecycle hooks.
type Current = Rc<RefCell<Option<Rc<RefCell<Animation>>>>>;

/// Hands one cancellable animation to the host's independent tail executor.
fn listener(state: Current) -> EventListener {
    Rc::new(move |_, _| {
        let Some(animation) = state.borrow().clone() else {
            return Ok(None);
        };
        if std::mem::replace(&mut animation.borrow_mut().claimed, true) {
            return Ok(None);
        }
        Ok(Some(Box::pin(poll_fn(move |cx| {
            let mut animation = animation.borrow_mut();
            animation.waker = Some(cx.waker().clone());
            let result = animation
                .running
                .as_mut()
                .map_or(std::task::Poll::Ready(Ok(())), |running| {
                    running.as_mut().poll(cx)
                });
            if result.is_ready() {
                animation.running = None;
                animation.waker = None;
            }
            result
        }))))
    })
}

/// Applies one lifecycle event to the animation slot shared by all three hooks.
fn handle(
    api: ExtensionAPI,
    current: Current,
    subscription: Subscription,
    event: &'static str,
    context: ExtensionContext,
) -> ExtensionFuture<'static, Option<ExtensionEventResult>> {
    Box::pin(async move {
        if let Some(animation) = current.borrow_mut().take() {
            let mut animation = animation.borrow_mut();
            animation.running = None;
            if let Some(waker) = animation.waker.take() {
                waker.wake();
            }
        }
        if event == "session_shutdown" {
            subscription.unsubscribe()?;
        }
        context.ui()?.set_title(&base_title(&api, &context)?)?;
        if event == "agent_start" {
            current.replace(Some(Rc::new(RefCell::new(Animation {
                running: Some(animate(api.clone(), context)),
                ..Animation::default()
            }))));
            if let Err(error) = api.events().emit(CHANNEL, serde_json::Value::Null).await {
                current.borrow_mut().take();
                return Err(error);
            }
        }
        Ok(None)
    })
}

/// Registers start, end and shutdown hooks using host-owned event tails for periodic work.
///
/// # Errors
/// Returns registration failures unchanged.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let current = Current::default();
        let subscription = api.events().on(CHANNEL, listener(current.clone()))?;
        for event in ["agent_start", "agent_end", "session_shutdown"] {
            let (owner, current, subscription) =
                (api.clone(), current.clone(), subscription.clone());
            api.on(
                event,
                Rc::new(move |_, context| {
                    handle(
                        owner.clone(),
                        current.clone(),
                        subscription.clone(),
                        event,
                        context,
                    )
                }),
            )?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
