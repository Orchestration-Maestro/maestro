//! Host widgets, lent styles and lent highlighting, served on the local owner.
use super::{Capability, Registry, pump::State};
use crate::guest_calls::paints::{Failure, HighlightFn};
use crate::widget_markdown::{highlighting, role};
use crate::widget_reads::{
    HighlighterEndpoint, Lent, LentTheme, Paint, StyleEndpoint, Widget, WidgetEndpoint,
    WidgetMessage,
};
use maestro_extensions::types::UIErrorReporter;
use maestro_extensions::widgets::BorderColor;
use std::rc::Rc;

impl Registry {
    /// The toolkit style a paint operand names; an authored style reports its failures to
    /// `failure`.
    pub(crate) fn paint(&self, paint: Paint, failure: &Failure) -> Result<BorderColor, String> {
        match paint {
            Paint::Host(style) => match self.capability(style.0.id).as_deref() {
                Some(Capability::Style(style)) => Ok(style.clone()),
                _ => Err("extension style resource ended".to_owned()),
            },
            Paint::Authored(handle) => Ok(handle.style(self.clone(), failure.clone())),
        }
    }

    /// The highlighting behind a lent highlighting resource.
    pub(crate) fn highlighter(
        &self,
        highlighter: &HighlighterEndpoint,
    ) -> Result<Rc<HighlightFn>, String> {
        match self.capability(highlighter.0.id).as_deref() {
            Some(Capability::Highlighter(highlight)) => Ok(highlight.clone()),
            _ => Err("extension highlighter resource ended".to_owned()),
        }
    }

    /// The terminal behind a lent terminal resource.
    pub(crate) fn terminal(
        &self,
        tui: &super::effects::TuiEndpoint,
    ) -> Result<maestro_extensions::TUI, String> {
        self.lent_terminal(tui).map(|(tui, _)| tui)
    }

    /// The terminal behind a lent terminal resource and the reporter of failures of the
    /// callbacks it runs, when the UI that lent it has one.
    pub(crate) fn lent_terminal(
        &self,
        tui: &super::effects::TuiEndpoint,
    ) -> Result<(maestro_extensions::TUI, Option<UIErrorReporter>), String> {
        match self.capability(tui.lease.id).as_deref() {
            Some(Capability::Tui(tui, reporter)) => Ok((tui.clone(), reporter.clone())),
            _ => Err("extension terminal resource ended".to_owned()),
        }
    }

    /// The row style behind a lent row style resource.
    pub(crate) fn row_style(
        &self,
        style: &crate::widget_lists::RowStyleEndpoint,
    ) -> Result<Rc<crate::widget_lists::RowStyleFn>, String> {
        match self.capability(style.0.id).as_deref() {
            Some(Capability::RowStyle(style)) => Ok(style.clone()),
            _ => Err("extension row style resource ended".to_owned()),
        }
    }

    /// Lends the closer of an open submenu.
    pub(crate) fn submenu_done(
        &self,
        done: Rc<dyn Fn(Option<String>)>,
    ) -> Result<crate::widget_lists::SubmenuDoneEndpoint, String> {
        self.insert(Capability::SubmenuDone(done))
            .map(crate::widget_lists::SubmenuDoneEndpoint)
    }

    /// Keeps a keybindings manager under a fresh identity.
    pub(crate) fn keep_keybindings(
        &self,
        keybindings: maestro_extensions::widgets::ExtensionKeybindings,
    ) -> Result<std::sync::Arc<super::Lease>, String> {
        self.insert(Capability::Keybindings(keybindings))
    }

    /// The application manager and its reload behind a keybindings resource.
    pub(crate) fn keybindings_manager(
        &self,
        keys: &crate::keys_reads::KeysEndpoint,
    ) -> Result<maestro_extensions::widgets::ExtensionKeybindings, String> {
        match self.capability(keys.0.id).as_deref() {
            Some(Capability::Keybindings(keys)) => Ok(keys.clone()),
            _ => Err("extension keybindings resource ended".to_owned()),
        }
    }

    /// Lends a host style.
    pub(crate) fn lend_style(&self, style: BorderColor) -> Result<StyleEndpoint, String> {
        self.insert(Capability::Style(style)).map(StyleEndpoint)
    }

    /// Lends editor styles: the border, then the completion list styles in field order.
    pub(crate) fn lend_editor_theme(
        &self,
        theme: maestro_extensions::widgets::EditorTheme,
    ) -> Result<(StyleEndpoint, Vec<StyleEndpoint>), String> {
        let list = Rc::new(theme.select_list);
        let styles = crate::widget_lists::select_roles(&list)
            .into_iter()
            .map(|style| self.lend_style(style))
            .collect::<Result<_, _>>()?;
        Ok((self.lend_style(theme.border_color)?, styles))
    }

    /// Lends a native text callback to an authored editor.
    pub(crate) fn lend_text(
        &self,
        callback: maestro_extensions::widgets::TextCallback,
    ) -> Result<crate::guest_calls::editors::TextCallbackEndpoint, String> {
        self.insert(Capability::Text(callback))
            .map(crate::guest_calls::editors::TextCallbackEndpoint)
    }

    /// The application's custom editor behind a widget resource.
    pub(crate) fn custom_editor(
        &self,
        widget: &WidgetEndpoint,
    ) -> Result<Rc<dyn maestro_extensions::types::CustomEditorComponent>, String> {
        self.widget(widget)?
            .custom_editor()
            .ok_or_else(|| "the widget is no custom editor".to_owned())
    }

    /// Replaces the original UI's editor with the one an authored factory builds, keeping the
    /// factory's identity so the extension can read the factory back, or restores the editor.
    pub(crate) fn set_editor_component(
        &self,
        ui: &maestro_extensions::ExtensionUIContext,
        factory: Option<crate::guest_calls::Handle>,
        api: &crate::api_reads::Endpoint,
    ) -> Result<(), String> {
        let api = self.facade(api)?;
        let factory = factory.map(|factory| {
            let id = factory.id();
            let reporter = api.ui_error_reporter(ui).ok();
            let native = factory.editor_factory(self.clone(), &api, reporter);
            let weak = native.downgrade();
            let identity = super::Identity::EditorFactory(native.downgrade(), id);
            self.resources.borrow_mut().identity(weak.addr(), identity);
            native
        });
        ui.set_editor_component(factory)
            .map_err(|error| error.to_string())
    }

    /// The original UI's current editor factory: one this extension set, by identity, or
    /// another's, lent; absent while its own editor is in use.
    pub(crate) fn editor_component(
        &self,
        ui: &maestro_extensions::ExtensionUIContext,
    ) -> Result<Option<Result<u32, crate::widget_foreign::FactoryEndpoint>>, String> {
        let Some(factory) = ui
            .get_editor_component()
            .map_err(|error| error.to_string())?
        else {
            return Ok(None);
        };
        let key = factory.downgrade().addr();
        if let Some(super::Identity::EditorFactory(weak, id)) =
            self.resources.borrow().identities.get(&key)
            && weak.alive()
        {
            return Ok(Some(Ok(*id)));
        }
        let lent = self.insert(Capability::EditorFactory(factory))?;
        Ok(Some(Err(crate::widget_foreign::FactoryEndpoint(lent))))
    }

    /// The widget behind a widget resource.
    pub(crate) fn widget(&self, widget: &WidgetEndpoint) -> Result<Rc<Widget>, String> {
        match self.capability(widget.0.id).as_deref() {
            Some(Capability::Widget(widget)) => Ok(widget.clone()),
            _ => Err("extension widget resource ended".to_owned()),
        }
    }
}

impl State {
    /// Serves widget construction, widget requests and lent styles without a table borrow.
    pub(super) fn widgets(&self, message: WidgetMessage) {
        let registry = &self.registry;
        match message {
            WidgetMessage::Overlay(message) => self.overlays(message),
            WidgetMessage::FullRedraws(id, reply) => self.full_redraws(id, &reply),
            message @ (WidgetMessage::Writer(..) | WidgetMessage::Drain(..)) => {
                self.frame_writer(message);
            }
            WidgetMessage::GlobalTheme(reply) => {
                let _ = reply.send(registry.theme(registry.widgets.theme()));
            }
            WidgetMessage::New(api, build, reply) => {
                let widget = registry
                    .events_of(api)
                    .and_then(|events| Widget::build(build, registry, &*registry.widgets, events))
                    .and_then(|widget| registry.insert(Capability::Widget(Rc::new(widget))))
                    .map(WidgetEndpoint);
                let _ = reply.send(widget);
            }
            WidgetMessage::Request(id, request) => match registry.capability(id).as_deref() {
                Some(Capability::Widget(widget)) => widget.answer(request, registry),
                _ => request.reject("extension widget resource ended"),
            },
            WidgetMessage::Lent(id, lent) => self.lent(id, lent),
            WidgetMessage::LendTheme(reply) => drop(reply.send(self.lend_theme())),
            WidgetMessage::LendSelect(reply) => drop(reply.send(self.lend_select())),
            WidgetMessage::LendSettings(reply) => {
                let _ = reply.send(self.lend_settings());
            }
            WidgetMessage::NewKeys(build, reply) => {
                let keys = crate::keys_reads::build(build, &*registry.widgets)
                    .and_then(|keys| registry.keybindings(keys));
                let _ = reply.send(keys);
            }
            WidgetMessage::Keys(id, request) => match registry.capability(id).as_deref() {
                Some(Capability::Keybindings(keys)) => request.answer(keys),
                _ => request.reject("extension keybindings resource ended"),
            },
            WidgetMessage::Hint(hint, reply) => {
                let _ = reply.send(crate::keys_reads::hint(hint, &*registry.widgets));
            }
            WidgetMessage::LendEditor(reply) => {
                let _ = reply.send(self.lend_editor());
            }
            WidgetMessage::Suggestions(id, request, admission) => {
                self.suggestions(id, request, admission);
            }
            WidgetMessage::Provider(id, request) => self.provider(id, request),
            WidgetMessage::Foreign(id, build) => {
                let reply = build.reply.clone();
                let _ = reply.send(self.foreign(id, build));
            }
        }
    }

    /// Lends the live theme's selection list styles in field order.
    fn lend_select(&self) -> Result<Vec<StyleEndpoint>, String> {
        let registry = &self.registry;
        let theme = Rc::new(registry.widgets.select_list_theme());
        crate::widget_lists::select_roles(&theme)
            .into_iter()
            .map(|style| registry.insert(Capability::Style(style)).map(StyleEndpoint))
            .collect()
    }

    /// Builds a frame writer's writer widget, or admits draining its terminal's input.
    fn frame_writer(&self, message: WidgetMessage) {
        let registry = &self.registry;
        match message {
            WidgetMessage::Writer(api, tui, reply) => {
                let writer = registry.events_of(api).and_then(|events| {
                    let writer = Widget::writer(registry.lent_terminal(&tui)?, events);
                    registry.insert(Capability::Widget(Rc::new(writer)))
                });
                let _ = reply.send(writer.map(WidgetEndpoint));
            }
            WidgetMessage::Drain(id, limits, reply, admission) => {
                self.drain(id, limits, reply, admission);
            }
            _ => {}
        }
    }

    /// Admits discarding the terminal's pending input until the native drain settles.
    fn drain(
        &self,
        id: u64,
        (max, idle): (Option<f64>, Option<f64>),
        reply: tokio::sync::oneshot::Sender<Result<(), String>>,
        admission: super::Admission,
    ) {
        let tui = match self.registry.capability(id).as_deref() {
            Some(Capability::Widget(widget)) => widget.tui().cloned(),
            _ => None,
        };
        let Some(tui) = tui else {
            let _ = reply.send(Err("the widget is no frame writer".to_owned()));
            return;
        };
        let duration = |ms: Option<f64>| {
            ms.map(|ms| std::time::Duration::try_from_secs_f64(ms / 1000.0))
                .transpose()
                .map_err(|error| error.to_string())
        };
        let limits = duration(max).and_then(|max| Ok((max, duration(idle)?)));
        let draining = limits.map(|(max, idle)| tui.terminal().borrow_mut().drain_input(max, idle));
        match draining {
            Ok(draining) => self.admit(
                Box::pin(async move {
                    let _ = reply.send(draining.await.map_err(|error| error.to_string()));
                }),
                admission,
            ),
            Err(error) => drop(reply.send(Err(error))),
        }
    }

    /// Reads the original frame writer's redraw counter.
    fn full_redraws(&self, id: u64, reply: &crate::widget_reads::Reply<u64>) {
        let result = match self.registry.capability(id).as_deref() {
            Some(Capability::Tui(tui, _)) => {
                u64::try_from(tui.full_redraws()).map_err(|error| error.to_string())
            }
            _ => Err("extension terminal resource ended".to_owned()),
        };
        let _ = reply.send(result);
    }

    /// Applies a lent style or runs the lent highlighting.
    fn lent(&self, id: u64, lent: Lent) {
        let capability = self.registry.capability(id);
        match (capability.as_deref(), lent) {
            (Some(Capability::Style(style)), Lent::Style(text, reply)) => {
                let _ = reply.send(Ok(style(&text)));
            }
            (Some(Capability::Highlighter(highlight)), Lent::Highlight(code, lang, reply)) => {
                let _ = reply.send(Ok(highlight(&code, lang.as_deref())));
            }
            (Some(Capability::RowStyle(style)), Lent::Row(text, selected, reply)) => {
                let _ = reply.send(Ok(style(&text, selected)));
            }
            (Some(Capability::SubmenuDone(done)), Lent::Done(value, reply)) => {
                done(value);
                let _ = reply.send(Ok(()));
            }
            (Some(Capability::Text(callback)), Lent::Text(text, reply)) => {
                callback(&text);
                let _ = reply.send(Ok(()));
            }
            (_, lent) => lent.reject("extension style resource ended"),
        }
    }

    /// Lends the live theme's editor border and selection list styles.
    fn lend_editor(&self) -> Result<(StyleEndpoint, Vec<StyleEndpoint>), String> {
        let registry = &self.registry;
        registry.lend_editor_theme(registry.widgets.editor_theme())
    }

    /// Lends the live theme's settings list styles.
    fn lend_settings(&self) -> Result<crate::widget_reads::LentSettings, String> {
        let registry = &self.registry;
        let theme = registry.widgets.settings_list_theme();
        let row = |style| {
            registry
                .insert(Capability::RowStyle(style))
                .map(crate::widget_lists::RowStyleEndpoint)
        };
        let style = |style| registry.insert(Capability::Style(style)).map(StyleEndpoint);
        Ok((
            row(theme.label)?,
            row(theme.value)?,
            style(theme.description)?,
            theme.cursor,
            style(theme.hint)?,
        ))
    }

    /// Lends each role of the live theme's Markdown styles and its highlighting.
    fn lend_theme(&self) -> Result<LentTheme, String> {
        let registry = &self.registry;
        let theme = Rc::new(registry.widgets.markdown_theme());
        let styles = (0..14)
            .map(|index| {
                registry
                    .insert(Capability::Style(role(&theme, index)))
                    .map(StyleEndpoint)
            })
            .collect::<Result<_, _>>()?;
        let highlighter = registry
            .insert(Capability::Highlighter(highlighting(&theme)))
            .map(HighlighterEndpoint)?;
        Ok((styles, highlighter))
    }
}

impl State {
    /// Builds an editor with a lent editor factory for the extension whose API is `api`.
    fn foreign(
        &self,
        api: u64,
        build: crate::widget_foreign::ForeignBuild,
    ) -> Result<(WidgetEndpoint, bool), String> {
        let registry = &self.registry;
        let factory = match registry.capability(build.factory.0.id).as_deref() {
            Some(Capability::EditorFactory(factory)) => factory.clone(),
            _ => return Err("extension editor factory resource ended".to_owned()),
        };
        let events = match registry.capability(api).as_deref() {
            Some(Capability::Api(api)) => api.events.clone(),
            _ => return Err("extension API resource ended".to_owned()),
        };
        let failure = Failure::default();
        let binding = crate::widget_lists::Binding {
            registry,
            events: &events,
            failure: &failure,
        };
        let (tui, theme, _, keybindings) =
            crate::widget_editors::EditorKind::parts(build.editor, &binding)?;
        let keybindings = keybindings.ok_or("an editor factory needs keybindings")?;
        let outcome = factory
            .call((tui, theme, keybindings))
            .map_err(|error| error.to_string())?;
        let custom = outcome.as_custom().is_some();
        let editor = crate::widget_editors::EditorKind::Foreign(outcome);
        let widget = Widget::editor(editor, failure, events);
        let widget = registry.insert(Capability::Widget(Rc::new(widget)))?;
        Ok((WidgetEndpoint(widget), custom))
    }
}
