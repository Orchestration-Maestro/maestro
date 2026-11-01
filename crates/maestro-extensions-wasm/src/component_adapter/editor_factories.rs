//! Editor factories the host runs, and the requests it makes on the authored editors they build.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::rc::Rc;

use super::factories::Terminal;
use super::renderers::with_emitter;
use super::widget_imports::WidgetImports;
use super::{Exports, callbacks};
use crate::{
    CallbackEmitter, ComponentView, EditorComponent, EditorFactory, EditorFactoryOutcome,
    ExtensionResult, Focusable, Style, TextHandler,
};

/// The current editor factory: absent, one this extension set by identity, or another
/// extension's as the host resource that builds with it.
pub(crate) type CurrentFactory = Option<Result<u32, Rc<dyn std::any::Any>>>;

/// What an editor factory's host lends it: the terminal, the editor styles and the
/// keybindings.
pub(crate) struct LentEditor<I: super::providers::EditorImports> {
    /// The lent terminal.
    pub(crate) tui: I::Tui,
    /// The border style, then the completion list styles in field order.
    pub(crate) theme: (I::Style, Vec<I::Style>),
    /// The lent keybindings manager.
    pub(crate) keybindings: I::Keybindings,
}

/// One request the host makes on an authored editor.
pub(crate) enum AuthoredRequest<I: super::providers::EditorImports> {
    /// The text.
    GetText,
    /// Replaces the text.
    SetText(String),
    /// Replaces or removes the submission callback with a lent one.
    SetOnSubmit(Option<I::TextCallback>),
    /// The change callback.
    OnChange,
    /// Replaces or removes the change callback with a lent one.
    SetOnChange(Option<I::TextCallback>),
    /// Adds text to the history.
    AddToHistory(String),
    /// Inserts text at the cursor.
    InsertTextAtCursor(String),
    /// The text with markers expanded.
    GetExpandedText,
    /// The border style.
    BorderColor,
    /// Replaces the border style with a lent one.
    SetBorderColor(I::Style),
    /// Replaces the side padding.
    SetPaddingX(f64),
    /// Replaces the completion row count.
    SetAutocompleteMaxVisible(f64),
    /// Replaces the completion provider with a lent one.
    SetAutocompleteProvider(I::Provider),
}

/// What an authored editor answered.
pub(crate) enum AuthoredAnswer {
    /// The operation ran.
    Done,
    /// The editor lacks the operation.
    Absent,
    /// Text.
    Text(String),
    /// The change callback, when one is installed.
    Handler(Option<TextHandler>),
    /// The border style.
    Style(Style),
}

impl AuthoredAnswer {
    /// `Done` when an optional operation ran, `Absent` when the editor lacks it.
    fn ran(ran: Option<()>) -> Self {
        ran.map_or(Self::Absent, |()| Self::Done)
    }
}

/// An authored editor seen as a component; input reaches the editor's own input operation.
pub(crate) struct EditorView(pub(crate) Rc<dyn EditorComponent>);

impl ComponentView for EditorView {
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.0.render(width, emitter)
    }
    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        EditorComponent::handle_input(self.0.as_ref(), data, emitter)
    }
    fn wants_key_release(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        self.0.wants_key_release(emitter)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        self.0.focusable()
    }
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.0.invalidate(emitter)
    }
}

/// A text callback the host lent, as an author's handler; it runs under the host's own frame.
pub(super) fn lent_text<I: super::providers::EditorImports>(
    imports: &I,
    callback: I::TextCallback,
) -> TextHandler {
    let (imports, callback) = (imports.clone(), Rc::new(callback));
    Rc::new(move |text, _| imports.text_callback(&callback, text))
}

impl<I: super::providers::EditorImports> Exports<I> {
    /// Runs the editor factory under this identity with the lent terminal, styles and
    /// keybindings.
    pub(crate) fn invoke_editor_factory(
        &self,
        handler: u32,
        lent: LentEditor<I>,
        emitter: &I::Emitter,
    ) -> ExtensionResult<EditorFactoryOutcome> {
        let tui = Terminal::lent(&self.imports, lent.tui);
        let theme = super::keys::editor_theme(&self.imports, lent.theme)?;
        let keybindings = super::keys::lend(&self.imports, lent.keybindings);
        let factory = callbacks::find::<EditorFactory>(handler)?;
        with_emitter(&self.imports, emitter, |emitter| {
            factory(tui, theme, keybindings, emitter)
        })
    }

    /// Runs one host request on the authored editor under this identity.
    pub(crate) fn invoke_editor(
        &self,
        handler: u32,
        request: AuthoredRequest<I>,
        emitter: &I::Emitter,
    ) -> ExtensionResult<AuthoredAnswer> {
        let editor = callbacks::find::<Rc<dyn EditorComponent>>(handler)?;
        let imports = &self.imports;
        with_emitter(imports, emitter, |emitter| {
            Ok(match request {
                AuthoredRequest::GetText => AuthoredAnswer::Text(editor.get_text(emitter)?),
                AuthoredRequest::SetText(text) => {
                    editor.set_text(&text, emitter)?;
                    AuthoredAnswer::Done
                }
                AuthoredRequest::SetOnSubmit(callback) => {
                    let handler = callback.map(|callback| lent_text(imports, callback));
                    editor.set_on_submit(handler, emitter)?;
                    AuthoredAnswer::Done
                }
                AuthoredRequest::OnChange => AuthoredAnswer::Handler(editor.on_change(emitter)?),
                AuthoredRequest::SetOnChange(callback) => {
                    let handler = callback.map(|callback| lent_text(imports, callback));
                    editor.set_on_change(handler, emitter)?;
                    AuthoredAnswer::Done
                }
                request => optional(imports, editor.as_ref(), request, emitter)?,
            })
        })
    }
}

/// Runs one optional operation, answering `Absent` when the editor lacks it.
fn optional<I: super::providers::EditorImports>(
    imports: &I,
    editor: &dyn EditorComponent,
    request: AuthoredRequest<I>,
    emitter: CallbackEmitter<'_>,
) -> ExtensionResult<AuthoredAnswer> {
    Ok(match request {
        AuthoredRequest::AddToHistory(text) => {
            AuthoredAnswer::ran(editor.add_to_history(&text, emitter)?)
        }
        AuthoredRequest::InsertTextAtCursor(text) => {
            AuthoredAnswer::ran(editor.insert_text_at_cursor(&text, emitter)?)
        }
        AuthoredRequest::GetExpandedText => editor
            .get_expanded_text(emitter)?
            .map_or(AuthoredAnswer::Absent, AuthoredAnswer::Text),
        AuthoredRequest::BorderColor => editor
            .border_color(emitter)?
            .map_or(AuthoredAnswer::Absent, AuthoredAnswer::Style),
        AuthoredRequest::SetBorderColor(style) => {
            let style = super::widgets::host_style(imports, style);
            AuthoredAnswer::ran(editor.set_border_color(style, emitter)?)
        }
        AuthoredRequest::SetPaddingX(padding) => {
            AuthoredAnswer::ran(editor.set_padding_x(padding, emitter)?)
        }
        AuthoredRequest::SetAutocompleteMaxVisible(max_visible) => {
            AuthoredAnswer::ran(editor.set_autocomplete_max_visible(max_visible, emitter)?)
        }
        AuthoredRequest::SetAutocompleteProvider(provider) => {
            let provider = super::providers::lend(imports, provider);
            AuthoredAnswer::ran(editor.set_autocomplete_provider(provider, emitter)?)
        }
        _ => return Err("expected an optional editor request".to_owned()),
    })
}

/// Transfers an editor a factory built to the host: its component, its editor operations and
/// the custom editor it extends, each under an owned identity.
#[cfg(target_arch = "wasm32")]
pub(super) fn transfer<
    I: WidgetImports<
            Callback = crate::bindings::maestro::extension::host::Callback,
            Widget = crate::bindings::maestro::extension::host::Widget,
        >,
>(
    imports: &I,
    outcome: &EditorFactoryOutcome,
) -> ExtensionResult<crate::bindings::maestro::extension::host::EditorBuilt> {
    let custom = match outcome {
        EditorFactoryOutcome::Editor(_) => None,
        EditorFactoryOutcome::CustomEditor(editor) => {
            let widget = editor.custom_editor().widget();
            let host = widget
                .owner::<super::widgets::HostWidget<I>>()
                .ok_or("the custom editor was not built by this host")?;
            Some(host.widget.share())
        }
    };
    let editor = outcome.editor();
    let component = crate::Component::new(EditorView(Rc::clone(&editor)));
    Ok(crate::bindings::maestro::extension::host::EditorBuilt {
        component: callbacks::Identity::<I>::new(imports).transfer(component),
        editor: callbacks::Identity::<I>::new(imports).transfer(editor),
        custom,
    })
}

/// Keeps what an editor factory built under fresh identities, as a test host holds it: the
/// component, the editor operations and the custom editor's widget.
#[cfg(test)]
pub(crate) fn keep_editor<I: WidgetImports, W>(
    imports: &I,
    outcome: &EditorFactoryOutcome,
    widget: impl Fn(&I::Widget) -> W,
) -> ExtensionResult<(u32, u32, Option<W>)> {
    let custom = match outcome {
        EditorFactoryOutcome::Editor(_) => None,
        EditorFactoryOutcome::CustomEditor(editor) => {
            let host = editor
                .custom_editor()
                .widget()
                .owner::<super::widgets::HostWidget<I>>()
                .ok_or("the custom editor was not built by this host")?;
            Some(widget(&host.widget))
        }
    };
    let editor = outcome.editor();
    let component = crate::Component::new(EditorView(Rc::clone(&editor)));
    Ok((keep(imports, component), keep(imports, editor), custom))
}

/// Keeps `object` under a fresh identity whose owner the table holds.
#[cfg(test)]
pub(super) fn keep<I: WidgetImports>(imports: &I, object: impl callbacks::Stored) -> u32 {
    let identity = callbacks::Identity::new(imports);
    let id = identity.id();
    identity.keep(object);
    id
}
