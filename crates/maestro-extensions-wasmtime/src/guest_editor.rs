//! A native editor over an authored editor, reaching it through the original frame.
use crate::guest_calls::{
    Call, Handle,
    editors::{Answered, EditorCall, Request, RetainedEditor, TextCallbackEndpoint},
    paints::Failure,
};
use crate::guest_component::GuestComponent;
use crate::local_resources::Registry;
use maestro_extensions::types::{ActionHandlers, CustomEditorComponent, ExtensionShortcutCallback};
use maestro_extensions::widgets::{
    AutocompleteProvider, BorderColor, EditorComponent, EditorError, Focusable, TextCallback,
};
use maestro_extensions::{Cancellation, Component, EventBus, EventError, InputHandler, Local};
use std::{cell::RefCell, rc::Rc};

/// The native side of an authored editor: its component, its editor operations and, for an
/// editor on the application's custom editor, that custom editor.
pub(crate) struct GuestEditor {
    /// Rendering, input and invalidation through the editor's component identity.
    view: GuestComponent,
    /// The editor operations' owner.
    editor: Handle,
    /// Original capability registry.
    registry: Registry,
    /// Each request gets a fresh frame of the extension's bus.
    events: EventBus<Local>,
    /// The first failure of an operation whose signature cannot return it.
    failure: Failure,
    /// The submission callback the host last installed.
    submit: RefCell<Option<TextCallback>>,
    /// The application's actions and callbacks of the custom editor the authored editor extends.
    custom: Option<Rc<dyn CustomEditorComponent>>,
}

impl GuestEditor {
    /// Retains the editor a factory built.
    pub(crate) fn new(
        built: RetainedEditor,
        registry: Registry,
        events: EventBus<Local>,
        custom: Option<Rc<dyn CustomEditorComponent>>,
    ) -> Self {
        Self {
            view: GuestComponent::new(built.component, registry.clone(), events.clone()),
            editor: built.editor,
            registry,
            events,
            failure: Failure::default(),
            submit: RefCell::default(),
            custom,
        }
    }

    /// Sends one request with a fresh frame of the extension's bus.
    fn request(&self, request: Request) -> Result<Answered, EditorError> {
        let answered = self
            .events
            .invoke_callback(|emitter| -> Result<Answered, EventError> {
                let emitter = self.registry.emitter(emitter.frame())?;
                self.editor
                    .endpoint()
                    .synchronous(&self.registry, |reply| {
                        Ok(Call::Editor(EditorCall {
                            handler: self.editor.clone(),
                            request,
                            emitter,
                            reply,
                        }))
                    })
                    .map_err(Into::into)
            });
        answered.map_err(|error| error.to_string().into())
    }

    /// Sends a request whose operation is optional: `Some(())` when it ran.
    fn optional(&self, request: Request) -> Result<Option<()>, EditorError> {
        match self.request(request)? {
            Answered::Done => Ok(Some(())),
            Answered::Absent => Ok(None),
            _ => Err("the editor answered no completion".into()),
        }
    }

    /// Sends a request that answers nothing.
    fn change(&self, request: Request) -> Result<(), EditorError> {
        match self.request(request)? {
            Answered::Done => Ok(()),
            _ => Err("the editor answered no completion".into()),
        }
    }

    /// Keeps a failure the native signature cannot return, for the next rendering.
    fn keep<T: Default>(&self, result: Result<T, EditorError>) -> T {
        result.unwrap_or_else(|error| {
            self.failure.borrow_mut().get_or_insert(error.to_string());
            T::default()
        })
    }

    /// Lends a callback, or keeps the failure to lend it.
    fn lend(
        &self,
        callback: Option<TextCallback>,
    ) -> Result<Option<TextCallbackEndpoint>, EditorError> {
        callback
            .map(|callback| self.registry.lend_text(callback))
            .transpose()
            .map_err(Into::into)
    }
}

impl Component for GuestEditor {
    fn render(&self, width: usize) -> Vec<String> {
        self.try_render(width).unwrap_or_else(|error| {
            self.failure.borrow_mut().get_or_insert(error.to_string());
            Vec::new()
        })
    }
    fn try_render(&self, width: usize) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        if let Some(error) = self.failure.borrow_mut().take() {
            return Err(error.into());
        }
        self.view.try_render(width)
    }
    fn invalidate(&self) {
        self.view.invalidate();
    }
    fn input_handler(&self) -> Option<&dyn InputHandler> {
        Some(self)
    }
    fn wants_key_release(&self) -> bool {
        self.view.wants_key_release()
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        match &self.custom {
            Some(custom) => custom.focusable(),
            None => self.view.focusable(),
        }
    }
}

impl InputHandler for GuestEditor {
    fn handle_input(&self, data: &str) {
        self.view.handle_input(data);
    }
    fn try_handle_input(&self, data: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.view.try_handle_input(data)
    }
}

impl EditorComponent for GuestEditor {
    type Signal = Cancellation;

    fn get_text(&self) -> String {
        self.keep(self.try_get_text())
    }
    fn set_text(&self, text: &str) {
        self.keep(self.try_set_text(text));
    }
    fn on_submit(&self) -> Option<TextCallback> {
        self.submit.borrow().clone()
    }
    fn set_on_submit(&self, callback: Option<TextCallback>) {
        drop(self.submit.replace(callback.clone()));
        let sent = self
            .lend(callback)
            .and_then(|callback| self.change(Request::SetOnSubmit(callback)));
        self.keep(sent);
    }
    fn on_change(&self) -> Option<TextCallback> {
        let handler = self
            .request(Request::OnChange)
            .and_then(|answer| match answer {
                Answered::Handler(handler) => Ok(handler),
                _ => Err("the editor answered no callback".into()),
            });
        self.keep(handler).map(|handle| self.text(handle))
    }
    fn set_on_change(&self, callback: Option<TextCallback>) {
        let sent = self
            .lend(callback)
            .and_then(|callback| self.change(Request::SetOnChange(callback)));
        self.keep(sent);
    }
    fn add_to_history(&self, text: &str) -> Option<()> {
        self.keep(self.try_add_to_history(text))
    }
    fn insert_text_at_cursor(&self, text: &str) -> Option<()> {
        self.keep(self.try_insert_text_at_cursor(text))
    }
    fn get_expanded_text(&self) -> Option<String> {
        self.keep(self.try_get_expanded_text())
    }
    fn border_color(&self) -> Option<BorderColor> {
        let style = self
            .request(Request::BorderColor)
            .and_then(|answer| match answer {
                Answered::Style(style) => Ok(Some(style)),
                Answered::Absent => Ok(None),
                _ => Err("the editor answered no style".into()),
            });
        let style = self.keep(style)?;
        Some(style.style(self.registry.clone(), self.failure.clone()))
    }
    fn set_border_color(&self, color: BorderColor) -> Option<()> {
        let sent = self
            .registry
            .lend_style(color)
            .map_err(EditorError::from)
            .and_then(|style| self.optional(Request::SetBorderColor(style)));
        self.keep(sent)
    }
    fn set_padding_x(&self, padding: f64) -> Option<()> {
        self.keep(self.try_set_padding_x(padding))
    }
    fn set_autocomplete_max_visible(&self, max_visible: f64) -> Option<()> {
        self.keep(self.try_set_autocomplete_max_visible(max_visible))
    }
    fn try_get_text(&self) -> Result<String, EditorError> {
        match self.request(Request::GetText)? {
            Answered::Text(text) => Ok(text),
            _ => Err("the editor answered no text".into()),
        }
    }
    fn try_set_text(&self, text: &str) -> Result<(), EditorError> {
        self.change(Request::SetText(text.to_owned()))
    }
    fn try_add_to_history(&self, text: &str) -> Result<Option<()>, EditorError> {
        self.optional(Request::AddToHistory(text.to_owned()))
    }
    fn try_insert_text_at_cursor(&self, text: &str) -> Result<Option<()>, EditorError> {
        self.optional(Request::InsertTextAtCursor(text.to_owned()))
    }
    fn try_get_expanded_text(&self) -> Result<Option<String>, EditorError> {
        match self.request(Request::GetExpandedText)? {
            Answered::Text(text) => Ok(Some(text)),
            Answered::Absent => Ok(None),
            _ => Err("the editor answered no text".into()),
        }
    }
    fn set_autocomplete_provider(
        &self,
        provider: Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
    ) -> Option<()> {
        self.keep(self.try_set_autocomplete_provider(provider))
    }
    fn try_set_autocomplete_provider(
        &self,
        provider: Rc<dyn AutocompleteProvider<Signal = Cancellation>>,
    ) -> Result<Option<()>, EditorError> {
        let provider = self.registry.lend_provider(provider)?;
        self.optional(Request::SetAutocompleteProvider(provider))
    }
    fn try_set_padding_x(&self, padding: f64) -> Result<Option<()>, EditorError> {
        self.optional(Request::SetPaddingX(padding))
    }
    fn try_set_autocomplete_max_visible(
        &self,
        max_visible: f64,
    ) -> Result<Option<()>, EditorError> {
        self.optional(Request::SetAutocompleteMaxVisible(max_visible))
    }
}

impl GuestEditor {
    /// The authored change handler as a native callback whose failures this editor keeps.
    fn text(&self, handle: Handle) -> TextCallback {
        let binding = crate::widget_lists::Binding {
            registry: &self.registry,
            events: &self.events,
            failure: &self.failure,
        };
        binding.text(handle)
    }
}

/// The application's actions and special callbacks belong to the custom editor the authored
/// editor extends; a plain authored editor is never offered as a custom editor.
impl CustomEditorComponent for GuestEditor {
    fn action_handlers(&self) -> ActionHandlers {
        self.custom
            .as_ref()
            .map_or_else(ActionHandlers::default, |custom| custom.action_handlers())
    }
    fn on_escape(&self) -> Option<Rc<dyn Fn()>> {
        self.custom.as_ref().and_then(|custom| custom.on_escape())
    }
    fn set_on_escape(&self, callback: Option<Rc<dyn Fn()>>) {
        if let Some(custom) = &self.custom {
            custom.set_on_escape(callback);
        }
    }
    fn on_ctrl_d(&self) -> Option<Rc<dyn Fn()>> {
        self.custom.as_ref().and_then(|custom| custom.on_ctrl_d())
    }
    fn set_on_ctrl_d(&self, callback: Option<Rc<dyn Fn()>>) {
        if let Some(custom) = &self.custom {
            custom.set_on_ctrl_d(callback);
        }
    }
    fn on_paste_image(&self) -> Option<Rc<dyn Fn()>> {
        self.custom
            .as_ref()
            .and_then(|custom| custom.on_paste_image())
    }
    fn set_on_paste_image(&self, callback: Option<Rc<dyn Fn()>>) {
        if let Some(custom) = &self.custom {
            custom.set_on_paste_image(callback);
        }
    }
    fn on_extension_shortcut(&self) -> Option<ExtensionShortcutCallback> {
        self.custom
            .as_ref()
            .and_then(|custom| custom.on_extension_shortcut())
    }
    fn set_on_extension_shortcut(&self, callback: Option<ExtensionShortcutCallback>) {
        if let Some(custom) = &self.custom {
            custom.set_on_extension_shortcut(callback);
        }
    }
}
