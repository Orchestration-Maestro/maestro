//! Play Space Invaders in a custom terminal view with session-entry saves.
// maestro-component: MAESTRO_SPACE_INVADERS_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

#[path = "space_invaders/game.rs"]
mod game;
#[path = "space_invaders/render.rs"]
mod render;

use game::{Bullet, GAME_WIDTH, GameState, KeyState, PLAYER_Y, TICK_MS};
use maestro_extensions_wasm::{
    CallbackEmitter, CommandOptions, Component, ComponentView, CustomDone, CustomFactoryOutcome,
    ExtensionAPI, ExtensionCommandContext, ExtensionComponent, ExtensionFuture, ExtensionResult,
    NotificationType, SessionEntry, TUI, is_key_release, matches_key,
};
use std::{
    cell::RefCell,
    future::{Future, poll_fn},
    rc::Rc,
    task::{Poll, Waker},
    time::Duration,
};

/// Custom entry containing the newest saved game or null after discarding it.
const INVADERS_SAVE_TYPE: &str = "space-invaders-save";
/// Private event channel that starts the independently owned ticking tail.
const TICK_CHANNEL: &str = "space-invaders:run";
/// Writes a complete saved game, or clears the save with null.
type Save = Rc<dyn Fn(Option<&GameState>) -> ExtensionResult<()>>;

/// Simulation state and transient held-key counters.
struct Game {
    /// Persisted game records.
    state: GameState,
    /// Held keys, never restored from a saved game.
    keys: KeyState,
    /// Whether a loaded game awaits its first key.
    paused: bool,
    /// Ticks until the next player shot.
    fire_cooldown: u32,
    /// Ticks since the player's last movement.
    player_move_counter: u32,
    /// Whether the host closed this view.
    closed: bool,
}
/// Custom view retained independently of the command invocation.
struct SpaceInvadersComponent {
    /// Host operations and text/key helpers.
    api: ExtensionAPI,
    /// Frame writer supplied to the custom factory.
    tui: TUI,
    /// Mutable world and input state.
    game: RefCell<Game>,
    /// First accepted close completes the custom view.
    on_close: CustomDone<()>,
    /// Session-entry save operation.
    on_save: Save,
}
impl SpaceInvadersComponent {
    /// Creates a paused saved game or starts a fresh game retaining its high score.
    fn new(
        api: ExtensionAPI,
        tui: TUI,
        on_close: CustomDone<()>,
        on_save: Save,
        saved: Option<GameState>,
    ) -> Self {
        let high_score = saved.as_ref().map_or(0, |s| s.high_score);
        let saved = saved.filter(|s| !s.game_over && !s.victory);
        let paused = saved.is_some();
        Self {
            api,
            tui,
            on_close,
            on_save,
            game: RefCell::new(Game {
                state: saved.unwrap_or_else(|| GameState::initial(high_score, 1)),
                keys: KeyState::default(),
                paused,
                fire_cooldown: 0,
                player_move_counter: 0,
                closed: false,
            }),
        }
    }
    /// Advances the retained world once after its native wait completes.
    fn tick(&self) -> ExtensionResult<()> {
        let mut game = self.game.borrow_mut();
        if game.closed || game.paused || game.state.game_over || game.state.victory {
            return Ok(());
        }
        game.player_move_counter += 1;
        if game.player_move_counter >= 2 {
            game.player_move_counter = 0;
            if game.keys.left && game.state.player.x > 2 {
                game.state.player.x -= 1;
            }
            if game.keys.right && game.state.player.x < GAME_WIDTH - 3 {
                game.state.player.x += 1;
            }
        }
        game.fire_cooldown = game.fire_cooldown.saturating_sub(1);
        if game.keys.fire
            && game.fire_cooldown == 0
            && game
                .state
                .bullets
                .iter()
                .filter(|b| b.direction == -1)
                .count()
                < 2
        {
            let x = game.state.player.x;
            game.state.bullets.push(Bullet {
                x,
                y: PLAYER_Y - 1,
                direction: -1,
            });
            game.fire_cooldown = 8;
        }
        game.state.tick();
        drop(game);
        self.tui.request_render(false)
    }
    /// Releases held keys before dropping the pending timer or closing the view.
    fn dispose(&self) {
        let mut game = self.game.borrow_mut();
        game.keys = KeyState::default();
        game.closed = true;
    }
    /// Applies movement and firing input, including enhanced keyboard releases.
    fn keys(&self, data: &str, released: bool) -> ExtensionResult<()> {
        let left = matches_key(&self.api, data, "left")?
            || matches!(data, "a" | "A")
            || matches_key(&self.api, data, "a")?;
        let right = matches_key(&self.api, data, "right")?
            || matches!(data, "d" | "D")
            || matches_key(&self.api, data, "d")?;
        let fire = matches_key(&self.api, data, "space")?
            || matches!(data, " " | "f" | "F")
            || matches_key(&self.api, data, "f")?;
        let mut game = self.game.borrow_mut();
        if left {
            game.keys.left = !released;
        }
        if right {
            game.keys.right = !released;
        }
        if fire {
            game.keys.fire = !released;
        }
        Ok(())
    }
}
impl ComponentView for SpaceInvadersComponent {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let game = self.game.borrow();
        render::render(&self.api, &game.state, game.paused, width)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
    fn wants_key_release(&self, _: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        Ok(true)
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if self.game.borrow().closed {
            return Ok(());
        }
        let released = is_key_release(&self.api, data)?;
        let escape = matches_key(&self.api, data, "escape")?;
        let quit = matches!(data, "q" | "Q");
        if self.game.borrow().paused && !released {
            if escape || quit {
                self.dispose();
                return (self.on_close)(());
            }
            self.game.borrow_mut().paused = false;
            return self.tui.request_render(false);
        }
        if !released && (escape || quit) {
            let game = self.game.borrow();
            (self.on_save)(if escape { Some(&game.state) } else { None })?;
            drop(game);
            self.dispose();
            return (self.on_close)(());
        }
        self.keys(data, released)?;
        let mut game = self.game.borrow_mut();
        if !released
            && (game.state.game_over || game.state.victory)
            && matches!(data, "r" | "R" | " ")
        {
            let level = if game.state.victory {
                game.state.level + 1
            } else {
                1
            };
            game.state = GameState::initial(game.state.high_score, level);
            game.keys = KeyState::default();
            drop(game);
            (self.on_save)(None)?;
            self.tui.request_render(false)?;
        }
        Ok(())
    }
}

/// One owned cancellable future; clearing it drops any pending host sleep immediately.
#[derive(Default)]
struct TickSlot {
    /// The only periodic operation for this view.
    future: Option<ExtensionFuture<'static, ()>>,
    /// Wakes the listener tail when disposal removes its operation.
    waker: Option<Waker>,
}
impl TickSlot {
    /// Cancels work and wakes the independent listener tail to settle it.
    fn cancel(&mut self) {
        self.future = None;
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
}
/// Starts the host timer only in an `EventBus` listener tail, with one cancellable slot.
fn ticking(
    api: &ExtensionAPI,
    view: Rc<SpaceInvadersComponent>,
    emitter: CallbackEmitter<'_>,
) -> ExtensionResult<maestro_extensions_wasm::ComponentDisposer> {
    let io = api.native_io();
    let ticking = view.clone();
    let slot = Rc::new(RefCell::new(TickSlot {
        future: Some(Box::pin(async move {
            loop {
                io.sleep(Duration::from_millis(TICK_MS), None).await?;
                ticking.tick()?;
            }
        })),
        waker: None,
    }));
    let pending = slot.clone();
    let announced = std::cell::Cell::new(false);
    let subscription = api.events().on(
        TICK_CHANNEL,
        Rc::new(move |_, _| {
            if announced.replace(true) {
                return Ok(None);
            }
            let pending = pending.clone();
            Ok(Some(Box::pin(poll_fn(move |cx| {
                let mut slot = pending.borrow_mut();
                slot.waker = Some(cx.waker().clone());
                slot.future.as_mut().map_or(Poll::Ready(Ok(())), |future| {
                    Future::poll(future.as_mut(), cx)
                })
            }))))
        }),
    )?;
    if let Err(error) = emitter.emit(TICK_CHANNEL, serde_json::Value::Null) {
        slot.borrow_mut().cancel();
        subscription.unsubscribe()?;
        return Err(error);
    }
    Ok(Rc::new(move |_| {
        view.dispose();
        slot.borrow_mut().cancel();
        subscription.unsubscribe()
    }))
}
/// Reads the newest save from all session entries, including an explicit discarded save.
fn saved(ctx: &ExtensionCommandContext) -> ExtensionResult<Option<GameState>> {
    for entry in ctx.session_manager()?.get_entries()?.into_iter().rev() {
        if let SessionEntry::Custom(entry) = entry
            && entry.custom_type == INVADERS_SAVE_TYPE
        {
            return entry.data.present().map_or(Ok(None), |data| {
                serde_json::from_str(data).map_err(|e| e.to_string())
            });
        }
    }
    Ok(None)
}
/// Opens the interactive game using the host's custom view and save-entry operations.
async fn play(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let ui = ctx.ui()?;
    if !ctx.has_ui()? {
        return ui.notify(
            "Space Invaders requires interactive mode",
            Some(NotificationType::Error),
        );
    }
    let saved = saved(&ctx)?;
    ui.custom(
        Rc::new(move |tui, _, _, done, emitter| {
            let saving = api.clone();
            let view = Rc::new(SpaceInvadersComponent::new(
                api.clone(),
                tui,
                done,
                Rc::new(move |state| {
                    saving.append_entry(
                        INVADERS_SAVE_TYPE,
                        Some(serde_json::to_value(state).map_err(|e| e.to_string())?),
                    )
                }),
                saved.clone(),
            ));
            let dispose = ticking(&api, view.clone(), emitter)?;
            Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
                component: Component::new(SharedView(view)),
                dispose: Some(dispose),
                set_expanded: None,
                width: None,
            }))
        }),
        None,
    )
    .await?;
    Ok(())
}
/// Shares the same view with its periodic future and host component handle.
struct SharedView(Rc<SpaceInvadersComponent>);
impl ComponentView for SharedView {
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.0.render(width, emitter)
    }
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.handle_input(data, emitter)
    }
    fn wants_key_release(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        self.0.wants_key_release(emitter)
    }
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.invalidate(emitter)
    }
}
/// Registers the opt-in game command.
///
/// # Errors
/// Returns host registration failures unchanged.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let playing = api.clone();
        api.register_command(
            "invaders",
            CommandOptions {
                description: Some("Play Space Invaders!".into()),
                get_argument_completions: None,
                handler: Rc::new(move |_, ctx| Box::pin(play(playing.clone(), ctx))),
            },
        )
    })
}
#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
