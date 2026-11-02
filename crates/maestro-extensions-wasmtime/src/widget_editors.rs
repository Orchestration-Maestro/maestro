//! Host editors and the application's custom editors, served on the local owner.
use crate::guest_calls::Handle;
use crate::guest_calls::widget_calls::{Answered, Event};
use crate::keys_reads::KeysEndpoint;
use crate::local_resources::effects::TuiEndpoint;
use crate::widget_lists::{Binding, ListRequest, Slot};
use crate::widget_reads::{Paint, Reply};
use maestro_extensions::types::{CustomEditorComponent, EditorFactoryOutcome};
use maestro_extensions::widgets::{
    Editor, EditorComponent, EditorOptions, EditorTheme, ExtensionEditor, ExtensionKeybindings,
    ExtensionWidgets, Focusable, SelectListTheme,
};
use maestro_extensions::{Cancellation, ComponentHandle, TUI};
use std::rc::Rc;
use wasmtime::component::{ComponentType, Lower};

/// Replaces the text of `editor`, or inserts at its cursor.
fn edit(editor: &Editor, text: &str, insert: bool) {
    if insert {
        editor.insert_text_at_cursor(text);
    } else {
        editor.set_text(text);
    }
}

/// What a new editor is built from.
pub(crate) struct EditorBuild {
    /// The lent terminal.
    pub(crate) tui: TuiEndpoint,
    /// The border style.
    pub(crate) border: Paint,
    /// The completion list styles in field order.
    pub(crate) list: Vec<Paint>,
    /// A custom editor's keybindings manager.
    pub(crate) keybindings: Option<KeysEndpoint>,
    /// Side padding and completion rows.
    pub(crate) options: (Option<f64>, Option<f64>),
}

/// An editor request that makes no call back into the instance.
pub(crate) enum Read {
    /// The text.
    GetText,
    /// The text with pasted blocks expanded.
    GetExpandedText,
    /// The lines.
    GetLines,
    /// The cursor.
    GetCursor,
    /// Adds text to the history.
    AddToHistory(String),
    /// The side padding.
    GetPaddingX,
    /// Replaces the side padding.
    SetPaddingX(f64),
    /// The completion rows.
    GetAutocompleteMaxVisible,
    /// Replaces the completion rows.
    SetAutocompleteMaxVisible(f64),
    /// Whether submission is disabled.
    GetDisableSubmit,
    /// Disables or enables submission.
    SetDisableSubmit(bool),
    /// Whether the completion list shows.
    IsShowingAutocomplete,
    /// Whether the editor has the focus.
    GetFocused,
    /// Gives or takes the focus.
    SetFocused(bool),
    /// Replaces the border style.
    SetBorderColor(Paint),
}

/// What an editor request answers.
#[derive(ComponentType, Lower)]
#[component(variant)]
pub(crate) enum EditorAnswer {
    /// Nothing.
    #[component(name = "done")]
    Done,
    /// Text.
    #[component(name = "text")]
    Text(String),
    /// Lines.
    #[component(name = "lines")]
    Lines(Vec<String>),
    /// A line and column.
    #[component(name = "cursor")]
    Cursor((u32, u32)),
    /// A number.
    #[component(name = "number")]
    Number(f64),
    /// A flag.
    #[component(name = "flag")]
    Flag(bool),
}

/// One request on an editor.
pub(crate) enum EditorRequest {
    /// A request without callbacks.
    Read(Read, Reply<EditorAnswer>),
    /// Replaces the text, or inserts it at the cursor; the change handler may run.
    Edit(String, bool, Reply<()>),
    /// Replaces the handler of a custom editor's application action.
    OnAction(String, Handle, Reply<()>),
    /// Lends the border style.
    BorderColor(Reply<crate::widget_reads::StyleEndpoint>),
    /// Replaces the completion provider with an authored one.
    Provider(Handle, Reply<()>),
    /// One editor operation, answering the editor's absence of an optional one.
    Foreign(
        crate::widget_foreign::ForeignRequest,
        Reply<crate::widget_foreign::ForeignAnswered>,
    ),
}

impl EditorRequest {
    /// Returns a catchable failure.
    pub(crate) fn reject(self, error: &str) {
        let error = error.to_owned();
        match self {
            Self::Read(_, reply) => drop(reply.send(Err(error))),
            Self::Edit(_, _, reply) | Self::OnAction(_, _, reply) | Self::Provider(_, reply) => {
                drop(reply.send(Err(error)));
            }
            Self::BorderColor(reply) => drop(reply.send(Err(error))),
            Self::Foreign(_, reply) => drop(reply.send(Err(error))),
        }
    }
}

/// The toolkit editor, the application's custom editor, or an editor another extension's
/// factory built.
pub(crate) enum EditorKind {
    /// The toolkit editor.
    Editor(Rc<Editor>),
    /// The application's custom editor.
    Custom(Rc<dyn ExtensionEditor>),
    /// An editor another extension's factory built.
    Foreign(EditorFactoryOutcome),
}

impl EditorKind {
    /// The editor as a toolkit component.
    pub(crate) fn component(&self) -> ComponentHandle {
        self.operations()
    }

    /// The editor through its editor operations.
    pub(crate) fn operations(&self) -> Rc<dyn EditorComponent<Signal = Cancellation>> {
        match self {
            Self::Editor(editor) => editor.clone(),
            Self::Custom(editor) => editor.clone(),
            Self::Foreign(built) => built.editor(),
        }
    }

    /// The application's actions and callbacks, when the editor has them.
    pub(crate) fn custom(&self) -> Option<Rc<dyn CustomEditorComponent>> {
        match self {
            Self::Editor(_) | Self::Foreign(EditorFactoryOutcome::Editor(_)) => None,
            Self::Custom(editor) => Some(editor.clone()),
            Self::Foreign(EditorFactoryOutcome::CustomEditor(editor)) => Some(editor.clone()),
        }
    }

    /// The toolkit editor operations; another extension's editor offers only its editor
    /// operations.
    fn editor(&self) -> Result<&Editor, String> {
        match self {
            Self::Editor(editor) => Ok(editor),
            Self::Custom(editor) => Ok(editor.editor()),
            Self::Foreign(_) => Err("the editor offers only its editor operations".to_owned()),
        }
    }

    /// Builds the editor `build` names.
    pub(crate) fn build(
        build: EditorBuild,
        binding: &Binding<'_>,
        widgets: &dyn ExtensionWidgets,
    ) -> Result<Self, String> {
        let (tui, theme, options, keybindings) = Self::parts(build, binding)?;
        Ok(match keybindings {
            None => Self::Editor(Rc::new(Editor::new(&tui, theme, options))),
            Some(manager) => {
                Self::Custom(widgets.custom_editor(&tui, theme, manager.manager, options))
            }
        })
    }

    /// The terminal, styles, options and keybindings `build` names.
    pub(crate) fn parts(
        build: EditorBuild,
        binding: &Binding<'_>,
    ) -> Result<
        (
            TUI,
            EditorTheme,
            EditorOptions,
            Option<ExtensionKeybindings>,
        ),
        String,
    > {
        let registry = binding.registry;
        let tui = registry.terminal(&build.tui)?;
        let mut list = build
            .list
            .into_iter()
            .map(|paint| registry.paint(paint, binding.failure));
        let mut next = || {
            list.next()
                .unwrap_or_else(|| Err("an editor style is missing".to_owned()))
        };
        let theme = EditorTheme {
            border_color: registry.paint(build.border, binding.failure)?,
            select_list: SelectListTheme {
                selected_prefix: next()?,
                selected_text: next()?,
                description: next()?,
                scroll_info: next()?,
                no_match: next()?,
            },
        };
        let options = EditorOptions {
            padding_x: build.options.0,
            autocomplete_max_visible: build.options.1,
        };
        let keybindings = build
            .keybindings
            .map(|keys| registry.keybindings_manager(&keys))
            .transpose()?;
        Ok((tui, theme, options, keybindings))
    }

    /// Answers one request.
    pub(crate) fn answer(&self, request: EditorRequest, binding: &Binding<'_>) {
        match request {
            EditorRequest::Read(read, reply) => {
                let _ = reply.send(self.read(read, binding));
            }
            EditorRequest::Edit(text, insert, reply) => {
                let edited = self.editor().map(|editor| edit(editor, &text, insert));
                let failure = binding.failure.borrow_mut().take();
                let _ = reply.send(edited.and(failure.map_or(Ok(()), Err)));
            }
            EditorRequest::Provider(handle, reply) => {
                let provider =
                    crate::guest_provider::GuestProvider::new(handle, binding.registry.clone());
                let installed = self.editor();
                let _ = reply.send(installed.map(|editor| {
                    editor.set_autocomplete_provider(Rc::new(provider));
                }));
            }
            EditorRequest::BorderColor(reply) => {
                let style = self.editor().map(Editor::border_color);
                let _ = reply.send(style.and_then(|style| binding.registry.lend_style(style)));
            }
            EditorRequest::OnAction(action, handle, reply) => {
                let _ = reply.send(match self.custom() {
                    Some(editor) => {
                        let handler = binding.notice(handle, || Event::Notice);
                        let old = editor
                            .action_handlers()
                            .borrow_mut()
                            .insert(action, handler);
                        drop(old);
                        Ok(())
                    }
                    None => Err("the editor has no application actions".to_owned()),
                });
            }
            EditorRequest::Foreign(request, reply) => {
                let _ = reply.send(self.foreign(request, binding));
            }
        }
    }

    /// Answers a request without callbacks.
    fn read(&self, read: Read, binding: &Binding<'_>) -> Result<EditorAnswer, String> {
        let editor = self.editor()?;
        let count = |value: usize| u32::try_from(value).map_err(|error| error.to_string());
        let answer = match read {
            Read::GetText => EditorAnswer::Text(editor.get_text()),
            Read::GetExpandedText => EditorAnswer::Text(editor.get_expanded_text()),
            Read::GetLines => EditorAnswer::Lines(editor.get_lines()),
            Read::GetCursor => {
                let cursor = editor.get_cursor();
                EditorAnswer::Cursor((count(cursor.line)?, count(cursor.col)?))
            }
            Read::GetPaddingX => EditorAnswer::Number(editor.get_padding_x()),
            Read::GetAutocompleteMaxVisible => {
                EditorAnswer::Number(f64::from(count(editor.get_autocomplete_max_visible())?))
            }
            Read::GetDisableSubmit => EditorAnswer::Flag(editor.disable_submit()),
            Read::IsShowingAutocomplete => EditorAnswer::Flag(editor.is_showing_autocomplete()),
            Read::GetFocused => EditorAnswer::Flag(editor.focus_flag().get()),
            change => {
                self.change(change, binding)?;
                EditorAnswer::Done
            }
        };
        Ok(answer)
    }

    /// Applies a request that changes the editor without callbacks.
    fn change(&self, change: Read, binding: &Binding<'_>) -> Result<(), String> {
        let editor = self.editor()?;
        match change {
            Read::AddToHistory(text) => editor.add_to_history(&text),
            Read::SetPaddingX(padding) => editor.set_padding_x(padding),
            Read::SetAutocompleteMaxVisible(max) => editor.set_autocomplete_max_visible(max),
            Read::SetDisableSubmit(disabled) => editor.set_disable_submit(disabled),
            Read::SetFocused(focused) => editor.focus_flag().set(focused),
            Read::SetBorderColor(paint) => {
                editor.set_border_color(binding.registry.paint(paint, binding.failure)?);
            }
            _ => return Err("expected an editor change".to_owned()),
        }
        Ok(())
    }

    /// Answers a handler change; an editor takes no other list request.
    pub(crate) fn answer_list(&self, request: ListRequest, binding: &Binding<'_>) {
        match request {
            ListRequest::SetHandler(slot, handler, reply) => {
                let _ = reply.send(self.set_handler(slot, handler, binding));
            }
            request => request.reject("the widget is no list"),
        }
    }

    /// Replaces or removes one handler.
    fn set_handler(
        &self,
        slot: Slot,
        handler: Option<Handle>,
        binding: &Binding<'_>,
    ) -> Result<(), String> {
        let notice = |handler: Option<Handle>| {
            handler.map(|handle| binding.notice(handle, || Event::Notice))
        };
        if matches!(slot, Slot::Submit | Slot::Change) {
            let text = handler.map(|handle| binding.text(handle));
            let editor = self.operations();
            if matches!(slot, Slot::Submit) {
                editor.set_on_submit(text);
            } else {
                editor.set_on_change(text);
            }
            return Ok(());
        }
        let editor = self.custom().ok_or("the widget has no such handler")?;
        match slot {
            Slot::Escape => editor.set_on_escape(notice(handler)),
            Slot::CtrlD => editor.set_on_ctrl_d(notice(handler)),
            Slot::PasteImage => editor.set_on_paste_image(notice(handler)),
            Slot::ExtensionShortcut => {
                editor.set_on_extension_shortcut(handler.map(|handle| binding.shortcut(handle)));
            }
            _ => return Err("the widget has no such handler".to_owned()),
        }
        Ok(())
    }
}

impl Binding<'_> {
    /// An authored text handler.
    pub(crate) fn text(&self, handle: Handle) -> Rc<dyn Fn(&str)> {
        let (registry, events, failure) = self.owned();
        Rc::new(move |text: &str| {
            let binding = Binding {
                registry: &registry,
                events: &events,
                failure: &failure,
            };
            binding.call(&handle, Event::Text(text.to_owned()));
        })
    }

    /// An authored interceptor; a failure consumes nothing.
    fn shortcut(&self, handle: Handle) -> Rc<dyn Fn(&str) -> bool> {
        let (registry, events, failure) = self.owned();
        Rc::new(move |data: &str| {
            let binding = Binding {
                registry: &registry,
                events: &events,
                failure: &failure,
            };
            let answer = binding.call(&handle, Event::Shortcut(data.to_owned()));
            matches!(answer, Some(Answered::Consumed(true)))
        })
    }
}
