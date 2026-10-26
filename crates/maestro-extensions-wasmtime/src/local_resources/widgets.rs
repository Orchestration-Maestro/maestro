//! Host widgets, lent styles and lent highlighting, served on the local owner.
use super::{Capability, Registry, pump::State};
use crate::guest_calls::paints::{Failure, HighlightFn};
use crate::widget_markdown::{highlighting, role};
use crate::widget_reads::{
    HighlighterEndpoint, Lent, LentTheme, Paint, StyleEndpoint, Widget, WidgetEndpoint,
    WidgetMessage,
};
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
        match self.capability(tui.lease.id).as_deref() {
            Some(Capability::Tui(tui)) => Ok(tui.clone()),
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

    /// The manager behind a keybindings resource.
    pub(crate) fn keybindings_manager(
        &self,
        keys: &crate::keys_reads::KeysEndpoint,
    ) -> Result<maestro_extensions::KeybindingsManager, String> {
        match self.capability(keys.0.id).as_deref() {
            Some(Capability::Keybindings(keys)) => Ok(keys.manager.clone()),
            _ => Err("extension keybindings resource ended".to_owned()),
        }
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
            WidgetMessage::New(api, build, reply) => {
                let events = match registry.capability(api).as_deref() {
                    Some(Capability::Api(api)) => Ok(api.events.clone()),
                    _ => Err("extension API resource ended".to_owned()),
                };
                let widget = events
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
            WidgetMessage::LendTheme(reply) => {
                let _ = reply.send(self.lend_theme());
            }
            WidgetMessage::LendSelect(reply) => {
                let theme = Rc::new(registry.widgets.select_list_theme());
                let styles = crate::widget_lists::select_roles(&theme)
                    .into_iter()
                    .map(|style| registry.insert(Capability::Style(style)).map(StyleEndpoint))
                    .collect();
                let _ = reply.send(styles);
            }
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
        }
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
            (_, lent) => lent.reject("extension style resource ended"),
        }
    }

    /// Lends the live theme's editor border and selection list styles.
    fn lend_editor(&self) -> Result<(StyleEndpoint, Vec<StyleEndpoint>), String> {
        let registry = &self.registry;
        let theme = registry.widgets.editor_theme();
        let style = |style| registry.insert(Capability::Style(style)).map(StyleEndpoint);
        let list = Rc::new(theme.select_list);
        let styles = crate::widget_lists::select_roles(&list)
            .into_iter()
            .map(style)
            .collect::<Result<_, _>>()?;
        Ok((style(theme.border_color)?, styles))
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
