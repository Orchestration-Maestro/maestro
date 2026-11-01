//! A modal editor in the style of vi.
//!
//! - Escape: insert mode to normal mode; in normal mode it reaches the editor, which
//!   interrupts the agent.
//! - `i`: normal mode to insert mode; `a` moves right first.
//! - `h`, `j`, `k`, `l`: navigation in normal mode.
//! - Control keys such as Ctrl+C and Ctrl+D work in both modes.
// maestro-component: MAESTRO_MODAL_EDITOR_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::{cell::Cell, rc::Rc};

use maestro_extensions_wasm::{
    AutocompleteProvider, CallbackEmitter, ComponentView, CustomEditor, CustomEditorComponent,
    EditorComponent, EditorFactory, EditorFactoryOutcome, EditorOptions, ExtensionAPI,
    ExtensionContext, ExtensionFuture, ExtensionResult, Style, TextHandler, keys::matches_key,
    truncate_to_width, utils::TruncateOptions, visible_width,
};

/// The application's editor with an insert mode and a normal mode.
struct ModalEditor {
    /// The editor this one extends.
    editor: CustomEditor,
    /// Host helpers for key matching and terminal widths.
    api: ExtensionAPI,
    /// Whether normal mode is on; the editor starts in insert mode.
    normal: Cell<bool>,
}

impl ModalEditor {
    /// Handles one input in normal mode.
    fn normal_input(&self, data: &str) -> ExtensionResult<()> {
        let sequence = match data {
            "h" => "\x1b[D",  // left
            "j" => "\x1b[B",  // down
            "k" => "\x1b[A",  // up
            "l" => "\x1b[C",  // right
            "0" => "\x01",    // line start
            "$" => "\x05",    // line end
            "x" => "\x1b[3~", // delete char
            "i" => {
                self.normal.set(false);
                return Ok(());
            }
            "a" => {
                self.normal.set(false);
                "\x1b[C" // append: move right first
            }
            // Control sequences such as Ctrl+C pass on; one printable character is dropped.
            _ if is_printable_character(data) => return Ok(()),
            _ => data,
        };
        self.editor.handle_input(sequence)
    }
}

/// Whether `data` is one character at or above the space, the delete key included.
fn is_printable_character(data: &str) -> bool {
    let mut characters = data.chars();
    matches!((characters.next(), characters.next()), (Some(character), None) if character >= ' ')
}

impl ComponentView for ModalEditor {
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let mut lines = ComponentView::render(&self.editor, width, emitter)?;
        // The mode indicator goes on the bottom border.
        let label = if self.normal.get() {
            " NORMAL "
        } else {
            " INSERT "
        };
        if let Some(last) = lines.last_mut()
            && visible_width(&self.api, last)? >= label.len()
        {
            let room = (width as usize).saturating_sub(label.len());
            let options = TruncateOptions {
                ellipsis: "",
                pad: false,
            };
            *last = truncate_to_width(&self.api, last, room, options)? + label;
        }
        Ok(lines)
    }
    fn invalidate(&self, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        ComponentView::invalidate(&self.editor, emitter)
    }
}

/// Every operation but input belongs to the custom editor this one extends.
impl EditorComponent for ModalEditor {
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if matches_key(&self.api, data, "escape")? {
            if self.normal.get() {
                return self.editor.handle_input(data); // interrupts the agent
            }
            self.normal.set(true);
            return Ok(());
        }
        if self.normal.get() {
            return self.normal_input(data);
        }
        self.editor.handle_input(data)
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

impl CustomEditorComponent for ModalEditor {
    fn custom_editor(&self) -> &CustomEditor {
        &self.editor
    }
}

/// Replaces the host's editor with a modal editor.
fn install(api: &ExtensionAPI, context: &ExtensionContext) -> ExtensionResult<()> {
    let api = api.clone();
    let factory: EditorFactory = Rc::new(move |tui, theme, keybindings, _| {
        let editor = CustomEditor::new(&tui, &theme, &keybindings, EditorOptions::default())?;
        Ok(EditorFactoryOutcome::CustomEditor(Rc::new(ModalEditor {
            editor,
            api: api.clone(),
            normal: Cell::new(false),
        })))
    });
    context.ui()?.set_editor_component(Some(factory))
}

/// Installs the modal editor at every session start.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let handler = api.clone();
        api.on(
            "session_start",
            Rc::new(move |_, context| {
                let api = handler.clone();
                Box::pin(async move { install(&api, &context).map(|()| None) })
            }),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
