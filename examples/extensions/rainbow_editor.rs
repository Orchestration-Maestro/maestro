//! Highlights `ultrathink` in the editor with an animated rainbow shine.
// maestro-component: MAESTRO_RAINBOW_EDITOR_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::{
    cell::{Cell, RefCell},
    future::poll_fn,
    rc::Rc,
    task::{Poll, Waker},
    time::Duration,
};

use maestro_extensions_wasm::{
    AutocompleteProvider, CallbackEmitter, ComponentView, CustomEditor, CustomEditorComponent,
    EditorComponent, EditorFactoryOutcome, EditorOptions, EventListener, ExtensionAPI,
    ExtensionContext, ExtensionEventResult, ExtensionFuture, ExtensionResult, Style, TUI,
    TextHandler,
};

/// The highlighted word, matched without regard to ASCII case.
const KEYWORD: &str = "ultrathink";
/// Coral, yellow, green, teal, blue, purple and pink, one per character in turn.
const COLORS: [[u8; 3]; 7] = [
    [233, 137, 115],
    [228, 186, 103],
    [141, 192, 122],
    [102, 194, 179],
    [121, 157, 207],
    [157, 134, 195],
    [206, 130, 172],
];
/// Restores the terminal's colors after a highlighted word.
const RESET: &str = "\x1b[0m";
/// The example's channel for handing the animation to the host's event-tail executor.
const CHANNEL: &str = "maestro.example.rainbow-editor";

/// The truecolor foreground `factor` of the way from `rgb` to white.
fn brighten(rgb: [u8; 3], factor: f64) -> String {
    let [r, g, b] = rgb.map(|c| (f64::from(c) + (255.0 - f64::from(c)) * factor).round());
    format!("\x1b[38;2;{r};{g};{b}m")
}

/// Colors each character, brightening the shine position and, less, its two neighbors.
fn colorize(text: &str, shine: Option<usize>) -> String {
    let mut colored = String::new();
    for (index, character) in text.chars().enumerate() {
        let factor = match shine.map(|shine| shine.abs_diff(index)) {
            Some(0) => 0.7,
            Some(1) => 0.35,
            _ => 0.0,
        };
        colored.push_str(&brighten(COLORS[index % COLORS.len()], factor));
        colored.push(character);
    }
    colored + RESET
}

/// Colors every occurrence of the keyword in one rendered row.
fn highlight(row: &str, shine: Option<usize>) -> String {
    let (mut highlighted, mut end) = (String::new(), 0);
    for (start, _) in row.to_ascii_lowercase().match_indices(KEYWORD) {
        highlighted.push_str(&row[end..start]);
        end = start + KEYWORD.len();
        highlighted.push_str(&colorize(&row[start..end], shine));
    }
    highlighted + &row[end..]
}

/// Advances one frame every 60 milliseconds and asks the terminal to redraw.
async fn animate(api: ExtensionAPI, frame: Rc<Cell<usize>>, tui: TUI) -> ExtensionResult<()> {
    let io = api.native_io();
    loop {
        io.sleep(Duration::from_millis(60), None).await?;
        frame.set(frame.get() + 1);
        tui.request_render(false)?;
    }
}

/// One animation's owned frame loop and the host tail that polls it.
#[derive(Default)]
struct Animation {
    /// The frame loop; the host tail drops it, and its pending native wait, once stopped.
    running: Option<ExtensionFuture<'static, ()>>,
    /// Wakes the host tail once the animation stops.
    waker: Option<Waker>,
    /// Whether the editor stopped animating.
    stopped: bool,
}

/// The animation waiting for the event-tail listener to claim it.
type Handoff = Rc<RefCell<Option<Rc<RefCell<Animation>>>>>;

/// Hands each announced animation to the host's independent tail executor exactly once.
fn listener(handoff: Handoff) -> EventListener {
    Rc::new(move |_, _| {
        let Some(animation) = handoff.borrow_mut().take() else {
            return Ok(None);
        };
        Ok(Some(Box::pin(poll_fn(move |cx| {
            let mut animation = animation.borrow_mut();
            if animation.stopped {
                animation.running = None;
                return Poll::Ready(Ok(()));
            }
            animation.waker = Some(cx.waker().clone());
            animation
                .running
                .as_mut()
                .map_or(Poll::Ready(Ok(())), |running| running.as_mut().poll(cx))
        }))))
    })
}

/// The application's custom editor with an animated keyword highlight.
struct RainbowEditor {
    /// The editor whose input, actions and rows this one extends.
    editor: CustomEditor,
    /// The terminal redrawn after each frame.
    tui: TUI,
    /// Host operations for the native frame timer.
    api: ExtensionAPI,
    /// Frames since the editor was built; a restarted animation continues from here.
    frame: Rc<Cell<usize>>,
    /// The running animation.
    animation: RefCell<Option<Rc<RefCell<Animation>>>>,
    /// Where a new animation waits for its host tail.
    handoff: Handoff,
}

impl RainbowEditor {
    /// Starts the frame loop unless it runs, and announces it to the host.
    fn start_animation(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if self.animation.borrow().is_some() {
            return Ok(());
        }
        let running = animate(self.api.clone(), Rc::clone(&self.frame), self.tui.clone());
        let animation = Rc::new(RefCell::new(Animation {
            running: Some(Box::pin(running)),
            ..Animation::default()
        }));
        self.animation.replace(Some(Rc::clone(&animation)));
        self.handoff.replace(Some(animation));
        emitter.emit(CHANNEL, serde_json::Value::Null)
    }

    /// Stops the frame loop and wakes its host tail to drop it.
    fn stop_animation(&self) {
        if let Some(animation) = self.animation.take() {
            let mut animation = animation.borrow_mut();
            animation.stopped = true;
            if let Some(waker) = animation.waker.take() {
                waker.wake();
            }
        }
    }
}

/// A released editor stops its frames instead of redrawing forever. The release runs
/// synchronously, so only the host tail may drop the pending native wait.
impl Drop for RainbowEditor {
    fn drop(&mut self) {
        self.stop_animation();
    }
}

impl ComponentView for RainbowEditor {
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        // Ten shine positions, then ten frames without shine.
        let cycle = self.frame.get() % 20;
        let shine = (cycle < 10).then_some(cycle);
        let rows = ComponentView::render(&self.editor, width, emitter)?;
        Ok(rows.iter().map(|row| highlight(row, shine)).collect())
    }
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        ComponentView::invalidate(&self.editor, emitter)
    }
}

/// Passes every editor operation to the custom editor, watching input for the keyword.
impl EditorComponent for RainbowEditor {
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        EditorComponent::handle_input(&self.editor, data, emitter)?;
        let text = EditorComponent::get_text(&self.editor, emitter)?;
        if text.to_ascii_lowercase().contains(KEYWORD) {
            self.start_animation(emitter)
        } else {
            self.stop_animation();
            Ok(())
        }
    }
    fn get_text(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<String> {
        EditorComponent::get_text(&self.editor, emitter)
    }
    fn set_text(&self, text: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        EditorComponent::set_text(&self.editor, text, emitter)
    }
    fn set_on_submit(
        &self,
        handler: Option<TextHandler>,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        EditorComponent::set_on_submit(&self.editor, handler, emitter)
    }
    fn on_change(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<Option<TextHandler>> {
        EditorComponent::on_change(&self.editor, emitter)
    }
    fn set_on_change(
        &self,
        handler: Option<TextHandler>,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<()> {
        EditorComponent::set_on_change(&self.editor, handler, emitter)
    }
    fn add_to_history(
        &self,
        text: &str,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::add_to_history(&self.editor, text, emitter)
    }
    fn insert_text_at_cursor(
        &self,
        text: &str,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::insert_text_at_cursor(&self.editor, text, emitter)
    }
    fn get_expanded_text(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<Option<String>> {
        EditorComponent::get_expanded_text(&self.editor, emitter)
    }
    fn set_autocomplete_provider(
        &self,
        provider: Rc<dyn AutocompleteProvider>,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::set_autocomplete_provider(&self.editor, provider, emitter)
    }
    fn border_color(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<Option<Style>> {
        EditorComponent::border_color(&self.editor, emitter)
    }
    fn set_border_color(
        &self,
        color: Style,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::set_border_color(&self.editor, color, emitter)
    }
    fn set_padding_x(
        &self,
        padding: f64,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::set_padding_x(&self.editor, padding, emitter)
    }
    fn set_autocomplete_max_visible(
        &self,
        max_visible: f64,
        emitter: CallbackEmitter<'_>,
    ) -> ExtensionResult<Option<()>> {
        EditorComponent::set_autocomplete_max_visible(&self.editor, max_visible, emitter)
    }
}

impl CustomEditorComponent for RainbowEditor {
    fn custom_editor(&self) -> &CustomEditor {
        &self.editor
    }
}

/// Replaces the session's editor with a new rainbow editor.
fn install(
    api: ExtensionAPI,
    handoff: Handoff,
    context: ExtensionContext,
) -> ExtensionFuture<'static, Option<ExtensionEventResult>> {
    Box::pin(async move {
        context
            .ui()?
            .set_editor_component(Some(Rc::new(move |tui, theme, keybindings, _| {
                let options = EditorOptions::default();
                Ok(EditorFactoryOutcome::CustomEditor(Rc::new(RainbowEditor {
                    editor: CustomEditor::new(&tui, &theme, &keybindings, options)?,
                    tui,
                    api: api.clone(),
                    frame: Rc::default(),
                    animation: RefCell::default(),
                    handoff: Rc::clone(&handoff),
                })))
            })))?;
        Ok(None)
    })
}

/// Registers the animation listener and installs the editor on every session start.
///
/// # Errors
/// Returns the host's registration rejection.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let handoff = Handoff::default();
        api.events().on(CHANNEL, listener(Rc::clone(&handoff)))?;
        let owner = api.clone();
        api.on(
            "session_start",
            Rc::new(move |_, context| install(owner.clone(), Rc::clone(&handoff), context)),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
