//! Shows the model, thinking level, context use, working directory and Git branch on the
//! editor's borders, in place of the footer and the working row.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use maestro_extensions_wasm::tools::path_utils;
use maestro_extensions_wasm::utils::TruncateOptions;
use maestro_extensions_wasm::{
    AutocompleteProvider, Background, BackgroundTask, CallbackEmitter, Component, ComponentView,
    ContextUsage, CustomEditor, CustomEditorComponent, Editor, EditorComponent,
    EditorFactoryOutcome, EditorOptions, ExecOptions, ExtensionAPI, ExtensionComponent,
    ExtensionContext, ExtensionEventResult, ExtensionFuture, ExtensionResult, Style, TUI,
    TextHandler, truncate_to_width, visible_width,
};

/// Spinner frames in display order, one every 80 milliseconds.
const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Fits `left` and `right` into one border row `width` cells wide.
///
/// The row keeps a corner cell at each end and at least three fill cells between the
/// texts; the right text gives way first, then the left.
fn fit_border(
    api: &ExtensionAPI,
    border: &Style,
    (left, right): (&str, &str),
    width: u32,
) -> ExtensionResult<String> {
    let width = width as usize;
    match width {
        0 => return Ok(String::new()),
        1 => return border.apply("─"),
        _ => {}
    }
    let room = width.saturating_sub(5);
    let right = clip(api, right, room.saturating_sub(visible_width(api, left)?))?;
    let left = clip(api, left, room.saturating_sub(visible_width(api, &right)?))?;
    let used = 2 + visible_width(api, &left)? + visible_width(api, &right)?;
    let fill = border.apply(&"─".repeat(width.saturating_sub(used)))?;
    let corner = border.apply("─")?;
    Ok(format!("{corner}{left}{fill}{right}{corner}"))
}

/// Cuts `text` to `width` cells without a marker, leaving text that already fits.
fn clip(api: &ExtensionAPI, text: &str, width: usize) -> ExtensionResult<String> {
    if visible_width(api, text)? <= width {
        return Ok(text.to_owned());
    }
    let options = TruncateOptions {
        ellipsis: "",
        pad: false,
    };
    truncate_to_width(api, text, width, options)
}

/// The context estimate as a rounded percentage of the window in thousands of tokens.
fn context_text(usage: Option<ContextUsage>) -> String {
    match usage {
        Some(ContextUsage {
            percent: Some(percent),
            context_window,
            ..
        }) if context_window != 0.0 => format!(
            "ctx {}%/{}k",
            ryu_js::Buffer::new().format(percent.round()),
            ryu_js::Buffer::new().format_to_fixed(context_window / 1000.0, 0)
        ),
        _ => "ctx ?".to_owned(),
    }
}

/// What the borders show, shared by the hooks, the background work and the editor.
#[derive(Default)]
struct Status {
    /// Whether the agent is working.
    working: Cell<bool>,
    /// The spinner frame shown while working.
    frame: Cell<usize>,
    /// The terminal of the editor built last; shutdown clears it.
    tui: RefCell<Option<TUI>>,
}

impl Status {
    /// Asks the editor's terminal to redraw, while there is one.
    fn request_render(&self) -> ExtensionResult<()> {
        self.tui
            .borrow()
            .as_ref()
            .map_or(Ok(()), |tui| tui.request_render(false))
    }
}

/// Background work: the running spinner and the session's branch read.
struct Work {
    /// Hands each piece of work to the host.
    background: Background,
    /// The spinner while the agent works; dropping it stops the frames.
    spinner: RefCell<Option<BackgroundTask>>,
    /// The branch read of the current session.
    branch: RefCell<Option<BackgroundTask>>,
}

/// Advances the spinner every 80 milliseconds and asks for a redraw.
async fn spin(api: ExtensionAPI, status: Rc<Status>) -> ExtensionResult<()> {
    let io = api.native_io();
    loop {
        io.sleep(Duration::from_millis(80), None).await?;
        status.frame.set((status.frame.get() + 1) % FRAMES.len());
        status.request_render()?;
    }
}

/// Reads the current branch once; a failed or empty answer shows no branch.
async fn refresh_branch(
    api: ExtensionAPI,
    status: Rc<Status>,
    cwd: String,
    branch: Rc<RefCell<Option<String>>>,
) -> ExtensionResult<()> {
    let args = ["branch".to_owned(), "--show-current".to_owned()];
    let options = ExecOptions {
        cwd: Some(cwd),
        ..ExecOptions::default()
    };
    let result = api.exec("git", &args, Some(options)).await;
    let name = result.ok().map(|result| result.stdout.trim().to_owned());
    branch.replace(name.filter(|name| !name.is_empty()));
    status.request_render()
}

/// A footer with no rows.
struct EmptyFooter;

impl ComponentView for EmptyFooter {
    fn render(&self, _: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        Ok(Vec::new())
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// The application's custom editor with the status written into its borders.
struct BorderStatusEditor {
    /// The editor whose input, actions and rows this one extends.
    editor: CustomEditor,
    /// Spinner state shared with the hooks.
    status: Rc<Status>,
    /// The branch the session's refresh found.
    branch: Rc<RefCell<Option<String>>>,
    /// Host operations for the thinking level and terminal text.
    api: ExtensionAPI,
    /// The session context read on each render.
    context: ExtensionContext,
}

impl BorderStatusEditor {
    /// The model and thinking level, then the context, directory and branch.
    fn bottom(&self) -> ExtensionResult<(String, String)> {
        let model = self.context.model()?.map_or_else(
            || "no model".to_owned(),
            |model| format!("{}/{}", model.provider, model.id),
        );
        let thinking = self.api.get_thinking_level()?;
        let cwd = self.context.cwd()?;
        // An unknown home leaves the directory as it is.
        let cwd = path_utils::shorten_path(&self.api, &cwd).unwrap_or(cwd);
        let branch = self
            .branch
            .borrow()
            .as_ref()
            .map_or_else(String::new, |branch| format!(" ({branch})"));
        let context = context_text(self.context.get_context_usage()?);
        Ok((
            format!(" {model} · {thinking} "),
            format!(" {context} · {cwd}{branch} "),
        ))
    }
}

impl ComponentView for BorderStatusEditor {
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let mut lines = ComponentView::render(&self.editor, width, emitter)?;
        if lines.len() < 2 {
            return Ok(lines);
        }
        let theme = self.context.ui()?.theme()?;
        let border = Editor::border_color(&self.editor)?;
        let spinner = if self.status.working.get() {
            theme.fg("accent", &format!(" {} ", FRAMES[self.status.frame.get()]))?
        } else {
            String::new()
        };
        lines[0] = fit_border(&self.api, &border, (&spinner, ""), width)?;
        // Completion rows follow the bottom border, so the status waits for the list to close.
        if Editor::is_showing_autocomplete(&self.editor)? {
            return Ok(lines);
        }
        let (left, right) = self.bottom()?;
        let sides = (theme.fg("muted", &left)?, theme.fg("muted", &right)?);
        let last = lines.len() - 1;
        lines[last] = fit_border(&self.api, &border, (&sides.0, &sides.1), width)?;
        Ok(lines)
    }
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        ComponentView::invalidate(&self.editor, emitter)
    }
}

/// Passes every editor operation to the custom editor.
impl EditorComponent for BorderStatusEditor {
    fn get_text(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<String> {
        EditorComponent::get_text(&self.editor, emitter)
    }
    fn set_text(&self, text: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        EditorComponent::set_text(&self.editor, text, emitter)
    }
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        EditorComponent::handle_input(&self.editor, data, emitter)
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

impl CustomEditorComponent for BorderStatusEditor {
    fn custom_editor(&self) -> &CustomEditor {
        &self.editor
    }
}

/// Hides the working row and the footer, reads the branch in the background and installs
/// the editor.
async fn session_start(
    api: ExtensionAPI,
    status: Rc<Status>,
    work: Rc<Work>,
    context: ExtensionContext,
) -> ExtensionResult<()> {
    let ui = context.ui()?;
    ui.set_working_visible(false)?;
    ui.set_footer(Some(Rc::new(|_, _, _, _| {
        Ok(ExtensionComponent {
            component: Component::new(EmptyFooter),
            dispose: None,
            set_expanded: None,
            width: None,
        })
    })))?;
    let branch = Rc::default();
    let refresh = refresh_branch(
        api.clone(),
        Rc::clone(&status),
        context.cwd()?,
        Rc::clone(&branch),
    );
    let read = work.background.spawn(Box::pin(refresh)).await?;
    work.branch.replace(Some(read));
    ui.set_editor_component(Some(Rc::new(move |tui, theme, keybindings, _| {
        let options = EditorOptions {
            padding_x: Some(0.0),
            ..EditorOptions::default()
        };
        let editor = CustomEditor::new(&tui, &theme, &keybindings, options)?;
        status.tui.replace(Some(tui));
        Ok(EditorFactoryOutcome::CustomEditor(Rc::new(
            BorderStatusEditor {
                editor,
                status: Rc::clone(&status),
                branch: Rc::clone(&branch),
                api: api.clone(),
                context: context.clone(),
            },
        )))
    })))
}

/// Applies one lifecycle event to the spinner and the shared state.
fn handle(
    api: ExtensionAPI,
    status: Rc<Status>,
    work: Rc<Work>,
    event: &'static str,
    context: ExtensionContext,
) -> ExtensionFuture<'static, Option<ExtensionEventResult>> {
    Box::pin(async move {
        match event {
            "session_start" => session_start(api, status, work, context).await?,
            "agent_start" => {
                status.working.set(true);
                drop(work.spinner.take());
                let spinner = Box::pin(spin(api.clone(), Rc::clone(&status)));
                let spinner = work.background.spawn(spinner).await?;
                work.spinner.replace(Some(spinner));
                status.request_render()?;
            }
            "agent_end" => {
                status.working.set(false);
                drop(work.spinner.take());
                status.request_render()?;
            }
            _ => {
                drop(work.spinner.take());
                status.tui.take();
            }
        }
        Ok(None)
    })
}

/// Registers the background-work listener and the session and agent hooks.
///
/// # Errors
/// Returns the host's registration rejection.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let status = Rc::<Status>::default();
        let work = Rc::new(Work {
            background: Background::new(api.events())?,
            spinner: RefCell::default(),
            branch: RefCell::default(),
        });
        for event in [
            "agent_start",
            "agent_end",
            "session_shutdown",
            "session_start",
        ] {
            let (owner, status, work) = (api.clone(), Rc::clone(&status), Rc::clone(&work));
            api.on(
                event,
                Rc::new(move |_, context| {
                    handle(
                        owner.clone(),
                        Rc::clone(&status),
                        Rc::clone(&work),
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
