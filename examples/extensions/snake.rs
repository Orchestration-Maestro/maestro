//! Play Snake in a custom terminal view, saving a paused game in session entries.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CallbackEmitter, CommandOptions, Component, ComponentView, CustomDone, CustomFactoryOutcome,
    ExtensionAPI, ExtensionCommandContext, ExtensionComponent, ExtensionFuture, ExtensionResult,
    NotificationType, SessionEntry, Subscription, TUI, keys::matches_key,
};
use std::{cell::RefCell, rc::Rc, time::Duration};
#[path = "snake/game.rs"]
mod game;
#[path = "snake/render.rs"]
mod render;
use game::{Direction, GameState};
/// The event channel handing each game's periodic work to the host.
const CHANNEL: &str = "maestro.example.snake";
/// The custom session entry holding the most recently saved game or null.
const SNAKE_SAVE_TYPE: &str = "snake-save";
#[path = "support/tail.rs"]
mod tail;
use tail::Tail;
/// The state retained while the custom component is visible.
struct SnakeComponent {
    /// The game saved when Escape closes the view.
    state: RefCell<GameState>,
    /// A restored game waits for one non-quitting input before moving.
    paused: RefCell<bool>,
    /// Host operations for persistence, key decoding and native waits.
    api: ExtensionAPI,
    /// Terminal to redraw after each tick or restart.
    tui: TUI,
    /// Completes this custom view once.
    done: CustomDone<()>,
    /// The sole owned tick slot.
    ticks: Tail,
}
impl SnakeComponent {
    /// Installs one tick loop and announces its independent host tail.
    fn start(self: &Rc<Self>, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.ticks
            .start(tick_loop(self.api.clone(), Rc::downgrade(self)));
        emitter.emit(CHANNEL, serde_json::Value::Null)
    }
    /// Stops before saving or closing, leaving a failed save visible to the host.
    fn close(&self, save: Option<serde_json::Value>) -> ExtensionResult<()> {
        self.ticks.stop();
        if let Some(save) = save {
            self.api.append_entry(SNAKE_SAVE_TYPE, Some(save))?;
        }
        (self.done)(())
    }
}
/// Sleeps through the native owner and advances only while the retained view is live.
fn tick_loop(
    api: ExtensionAPI,
    view: std::rc::Weak<SnakeComponent>,
) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let io = api.native_io();
        loop {
            io.sleep(Duration::from_millis(100), None).await?;
            let Some(view) = view.upgrade() else {
                return Ok(());
            };
            if !view.state.borrow().game_over {
                view.state.borrow_mut().tick();
                view.tui.request_render(false)?;
            }
        }
    })
}
/// Shared ownership lets input resume the same view retained by the tick loop.
struct SnakeView(Rc<SnakeComponent>);
impl ComponentView for SnakeView {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        render::render(
            &self.0.api,
            &self.0.state.borrow(),
            *self.0.paused.borrow(),
            width,
        )
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        let view = &self.0;
        let escape = matches_key(&view.api, data, "escape")?;
        let quit = data.eq_ignore_ascii_case("q");
        if *view.paused.borrow() {
            if escape || quit {
                return view.close(None);
            }
            *view.paused.borrow_mut() = false;
            return view.start(emitter);
        }
        if escape {
            let save = serde_json::to_value(&*view.state.borrow()).map_err(|e| e.to_string())?;
            return view.close(Some(save));
        }
        if quit {
            return view.close(Some(serde_json::Value::Null));
        }
        turn(view, data)?;
        if view.state.borrow().game_over && matches!(data, "r" | "R" | " ") {
            let high = view.state.borrow().high_score;
            *view.state.borrow_mut() = GameState::new(high);
            view.api
                .append_entry(SNAKE_SAVE_TYPE, Some(serde_json::Value::Null))?;
            view.tui.request_render(false)?;
        }
        Ok(())
    }
}
/// Decodes the host's arrow keys and the game's authored letter controls.
fn turn(view: &SnakeComponent, data: &str) -> ExtensionResult<()> {
    for (key, letter, direction) in [
        ("up", "w", Direction::Up),
        ("down", "s", Direction::Down),
        ("right", "d", Direction::Right),
        ("left", "a", Direction::Left),
    ] {
        if matches_key(&view.api, data, key)? || data.eq_ignore_ascii_case(letter) {
            view.state.borrow_mut().turn(direction);
            break;
        }
    }
    Ok(())
}
/// Builds a view and a listener scoped to its lifetime, without retaining the view in the bus.
fn build(
    api: ExtensionAPI,
    state: GameState,
    paused: bool,
    input: (TUI, CustomDone<()>, CallbackEmitter<'_>),
) -> ExtensionResult<ExtensionComponent> {
    let ticks = Tail::default();
    let subscription = api.events().on(CHANNEL, ticks.listener())?;
    let view = Rc::new(SnakeComponent {
        state: RefCell::new(state),
        paused: RefCell::new(paused),
        api,
        tui: input.0,
        done: input.1,
        ticks,
    });
    if !paused && let Err(error) = view.start(input.2) {
        dispose(&view, &subscription)?;
        return Err(error);
    }
    let disposing = view.clone();
    Ok(ExtensionComponent {
        component: Component::new(SnakeView(view)),
        dispose: Some(Rc::new(move |_| dispose(&disposing, &subscription))),
        set_expanded: None,
        width: None,
    })
}
/// Cancels pending movement before removing the listener.
fn dispose(view: &SnakeComponent, subscription: &Subscription) -> ExtensionResult<()> {
    view.ticks.stop();
    subscription.unsubscribe()
}
/// Reads the last matching entry from the entire session, including a null clearing entry.
fn saved(ctx: &ExtensionCommandContext) -> ExtensionResult<Option<GameState>> {
    for entry in ctx.session_manager()?.get_entries()?.into_iter().rev() {
        if let SessionEntry::Custom(entry) = entry
            && entry.custom_type == SNAKE_SAVE_TYPE
        {
            return entry.data.present().map_or(Ok(None), |data| {
                serde_json::from_str(data).map_err(|e| e.to_string())
            });
        }
    }
    Ok(None)
}
/// Opens the custom game only in an interactive host.
async fn play(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    if !ctx.has_ui()? {
        ctx.ui()?.notify(
            "Snake requires interactive mode",
            Some(NotificationType::Error),
        )?;
        return Ok(());
    }
    let saved = saved(&ctx)?;
    if let Some(saved) = &saved {
        saved.validate()?;
    }
    let paused = saved.as_ref().is_some_and(|state| !state.game_over);
    let high = saved.as_ref().map_or(0, |state| state.high_score);
    let state = saved
        .filter(|state| !state.game_over)
        .unwrap_or_else(|| GameState::new(high));
    ctx.ui()?
        .custom(
            Rc::new(move |tui, _, _, done, emitter| {
                build(api.clone(), state.clone(), paused, (tui, done, emitter))
                    .map(CustomFactoryOutcome::Ready)
            }),
            None,
        )
        .await?;
    Ok(())
}
/// Registers the opt-in Snake command.
///
/// # Errors
/// Returns the host's registration rejection.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let registered = api.clone();
        api.register_command(
            "snake",
            CommandOptions {
                description: Some("Play Snake!".into()),
                get_argument_completions: None,
                handler: Rc::new(move |_, ctx| Box::pin(play(registered.clone(), ctx))),
            },
        )
    })
}
#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
