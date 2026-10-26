//! Host widgets, lent styles and lent highlighting, served on the local owner.
use super::{Capability, Registry, pump::State};
use crate::guest_calls::paints::{Failure, HighlightFn};
use crate::widget_reads::{
    HighlighterEndpoint, LentTheme, Paint, StyleEndpoint, Widget, WidgetEndpoint, WidgetMessage,
    highlighting, role,
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
        }
    }

    /// Applies a lent style or runs the lent highlighting.
    fn lent(&self, id: u64, lent: crate::widget_reads::Lent) {
        let capability = self.registry.capability(id);
        match (capability.as_deref(), lent) {
            (Some(Capability::Style(style)), crate::widget_reads::Lent::Style(text, reply)) => {
                let _ = reply.send(Ok(style(&text)));
            }
            (
                Some(Capability::Highlighter(highlight)),
                crate::widget_reads::Lent::Highlight(code, lang, reply),
            ) => {
                let _ = reply.send(Ok(highlight(&code, lang.as_deref())));
            }
            (_, lent) => lent.reject("extension style resource ended"),
        }
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
