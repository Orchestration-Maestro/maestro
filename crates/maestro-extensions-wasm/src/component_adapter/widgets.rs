//! Host widgets, styles and highlighting behind the author's widget facades.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::cell::RefCell;
use std::rc::Rc;

use super::callbacks::{self, Identity};
use super::lists::{WidgetHandler, set_handler};
use super::widget_imports::{
    ChildKey, ChildRef, HandlerSlot, HighlightRef, MarkdownStyles, StyleRef, TextStyle,
    WidgetImports,
};
use crate::{
    Component, DefaultTextStyle, ExtensionResult, Highlight, HostWidgetHandle as Widget,
    MarkdownTheme, Style, WidgetHost, WidgetPort,
};

/// A host style kept by its facade.
pub(super) struct HostStyle<I: WidgetImports> {
    /// The host resource.
    style: I::Style,
}

/// The host's highlighting kept by its facade.
struct HostHighlighter<I: WidgetImports> {
    /// The host resource.
    highlighter: I::Highlighter,
}

/// An authored function staged for one request, or the host resource a facade names.
pub(super) enum Staged<I: WidgetImports, H> {
    /// A host resource.
    Host(Rc<H>),
    /// An authored function under a fresh identity.
    Authored(Identity<I>),
}

impl<I: WidgetImports, H> Staged<I, H> {
    /// Keeps a staged function after an accepted request, or withdraws it.
    pub(super) fn settle(self, accepted: bool) {
        if let Self::Authored(identity) = self {
            identity.settle(accepted);
        }
    }
}

/// The host style behind `style`, or `style` staged under a fresh identity.
pub(super) fn stage_style<I: WidgetImports>(imports: &I, style: &Style) -> Staged<I, HostStyle<I>> {
    if let Some(host) = style.0.host::<HostStyle<I>>() {
        return Staged::Host(host);
    }
    let identity = Identity::new(imports);
    identity.stage(style.clone());
    Staged::Authored(identity)
}

/// The operand naming a staged style.
pub(super) fn style_ref<I: WidgetImports>(staged: &Staged<I, HostStyle<I>>) -> StyleRef<'_, I> {
    match staged {
        Staged::Host(host) => StyleRef::Host(&host.style),
        Staged::Authored(identity) => StyleRef::Authored(&identity.handle),
    }
}

/// Runs `request` with an optional staged style, keeping it only when the host accepts.
fn with_style<I: WidgetImports, T>(
    imports: &I,
    style: Option<&Style>,
    request: impl FnOnce(Option<StyleRef<'_, I>>) -> ExtensionResult<T>,
) -> ExtensionResult<T> {
    let staged = style.map(|style| stage_style(imports, style));
    let result = request(staged.as_ref().map(style_ref));
    if let Some(staged) = staged {
        staged.settle(result.is_ok());
    }
    result
}

/// One host widget with the authored components added to it.
pub(super) struct HostWidget<I: WidgetImports> {
    /// The host the widget belongs to.
    pub(super) imports: I,
    /// The host resource.
    pub(super) widget: I::Widget,
    /// Authored children with the identity each was added under, in insertion order.
    guests: RefCell<Vec<(Component, u32)>>,
    /// An editor's change handler last set through its facade.
    pub(super) change: RefCell<Option<crate::TextHandler>>,
}

impl<I: WidgetImports> HostWidget<I> {
    /// Keeps a built widget.
    pub(super) fn shared(imports: &I, widget: I::Widget) -> Rc<Self> {
        Rc::new(Self {
            imports: imports.clone(),
            widget,
            guests: RefCell::default(),
            change: RefCell::default(),
        })
    }
    /// The identity `component` was added under, when it is an authored child.
    pub(super) fn guest(&self, component: &Component) -> Option<u32> {
        let guests = self.guests.borrow();
        guests
            .iter()
            .find_map(|(added, id)| added.same(component).then_some(*id))
    }
    /// Retains a built widget.
    fn retain(imports: &I, widget: I::Widget) -> Widget {
        Widget::new(Self::shared(imports, widget))
    }
}

impl<I: WidgetImports> WidgetPort for HostWidget<I> {
    fn render(&self, width: u32) -> ExtensionResult<Vec<String>> {
        self.imports.widget_render(&self.widget, width)
    }
    fn invalidate(&self) -> ExtensionResult<()> {
        self.imports.widget_invalidate(&self.widget)
    }
    fn set_text(&self, text: &str) -> ExtensionResult<()> {
        self.imports.widget_set_text(&self.widget, text)
    }
    fn set_background(&self, style: Option<&Style>) -> ExtensionResult<()> {
        with_style(&self.imports, style, |style| {
            self.imports.widget_set_background(&self.widget, style)
        })
    }
    fn set_lines(&self, lines: u32) -> ExtensionResult<()> {
        self.imports.widget_set_lines(&self.widget, lines)
    }
    fn add_child(&self, child: &Component) -> ExtensionResult<()> {
        if let Some(host) = child.widget().and_then(Widget::owner::<Self>) {
            return self
                .imports
                .widget_add_child(&self.widget, ChildRef::Widget(&host.widget));
        }
        let identity = Identity::new(&self.imports);
        identity.stage(child.clone());
        let result = self
            .imports
            .widget_add_child(&self.widget, ChildRef::Component(&identity.handle));
        if result.is_ok() {
            self.guests
                .borrow_mut()
                .push((child.clone(), identity.id()));
        }
        identity.settle(result.is_ok());
        result
    }
    fn remove_child(&self, child: &Component) -> ExtensionResult<()> {
        if let Some(host) = child.widget().and_then(Widget::owner::<Self>) {
            return self
                .imports
                .widget_remove_child(&self.widget, ChildKey::Widget(&host.widget));
        }
        let index = self
            .guests
            .borrow()
            .iter()
            .position(|(added, _)| added.same(child));
        let Some(index) = index else {
            return Ok(());
        };
        let id = self.guests.borrow()[index].1;
        self.imports
            .widget_remove_child(&self.widget, ChildKey::Component(id))?;
        let removed = self.guests.borrow_mut().remove(index);
        drop(removed);
        Ok(())
    }
    fn clear(&self) -> ExtensionResult<()> {
        self.imports.widget_clear(&self.widget)?;
        let removed = std::mem::take(&mut *self.guests.borrow_mut());
        drop(removed);
        Ok(())
    }
    fn handle_input(&self, data: &str) -> ExtensionResult<()> {
        self.imports.widget_handle_input(&self.widget, data)
    }
    fn set_filter(&self, filter: &str) -> ExtensionResult<()> {
        self.imports.widget_set_filter(&self.widget, filter)
    }
    fn set_selected_index(&self, index: i64) -> ExtensionResult<()> {
        self.imports.widget_set_selected_index(&self.widget, index)
    }
    fn get_selected_item(&self) -> ExtensionResult<Option<crate::SelectItem>> {
        self.imports.widget_get_selected_item(&self.widget)
    }
    fn set_on_select(&self, handler: Option<crate::SelectHandler>) -> ExtensionResult<()> {
        let handler = handler.map(WidgetHandler::Select);
        set_handler(&self.imports, &self.widget, HandlerSlot::Select, handler)
    }
    fn set_on_cancel(&self, handler: Option<crate::WidgetNotice>) -> ExtensionResult<()> {
        let handler = handler.map(WidgetHandler::Notice);
        set_handler(&self.imports, &self.widget, HandlerSlot::Cancel, handler)
    }
    fn set_on_selection_change(
        &self,
        handler: Option<crate::SelectHandler>,
    ) -> ExtensionResult<()> {
        let handler = handler.map(WidgetHandler::Select);
        set_handler(
            &self.imports,
            &self.widget,
            HandlerSlot::SelectionChange,
            handler,
        )
    }
    fn set_on_abort(&self, handler: Option<crate::WidgetNotice>) -> ExtensionResult<()> {
        let handler = handler.map(WidgetHandler::Notice);
        set_handler(&self.imports, &self.widget, HandlerSlot::Abort, handler)
    }
    fn update_value(&self, id: &str, value: &str) -> ExtensionResult<()> {
        self.imports.widget_update_value(&self.widget, id, value)
    }
    fn signal(&self) -> ExtensionResult<crate::AbortSignal> {
        let signal = self.imports.widget_signal(&self.widget)?;
        Ok(super::signals::signal(&self.imports, signal))
    }
    fn dispose(&self) -> ExtensionResult<()> {
        self.imports.widget_dispose(&self.widget)
    }
}

/// Builds host widgets over `I`.
pub(crate) struct Builder<I: WidgetImports>(pub(crate) I);

impl<I: WidgetImports> Builder<I> {
    /// Stages every authored function of `theme` and `style` for one Markdown request.
    fn build_markdown(
        &self,
        text: &str,
        padding: (u32, u32),
        theme: &MarkdownTheme,
        style: Option<&DefaultTextStyle>,
    ) -> ExtensionResult<I::Widget> {
        let imports = &self.0;
        let roles: Vec<_> = roles(theme)
            .into_iter()
            .map(|style| stage_style(imports, style))
            .collect();
        let highlight = theme
            .highlight_code
            .as_ref()
            .map(|highlight| stage_highlight(imports, highlight));
        let color = style.and_then(|style| style.color.as_ref());
        let color = color.map(|color| stage_style(imports, color));
        let bg = style.and_then(|style| style.bg_color.as_ref());
        let bg = bg.map(|bg| stage_style(imports, bg));
        let styles = MarkdownStyles {
            roles: roles.iter().map(style_ref).collect(),
            highlight: highlight.as_ref().map(|staged| match staged {
                Staged::Host(host) => HighlightRef::Host(&host.highlighter),
                Staged::Authored(identity) => HighlightRef::Authored(&identity.handle),
            }),
            indent: theme.code_block_indent.as_deref(),
        };
        let text_style = style.map(|style| TextStyle {
            color: color.as_ref().map(style_ref),
            bg_color: bg.as_ref().map(style_ref),
            decorations: &style.decorations,
        });
        let result = imports.new_markdown(text, padding, styles, text_style);
        let accepted = result.is_ok();
        for staged in roles.into_iter().chain(color).chain(bg) {
            staged.settle(accepted);
        }
        if let Some(staged) = highlight {
            staged.settle(accepted);
        }
        result
    }
}

/// The element styles of `theme`, in field order.
fn roles(theme: &MarkdownTheme) -> [&Style; 14] {
    [
        &theme.heading,
        &theme.link,
        &theme.link_url,
        &theme.code,
        &theme.code_block,
        &theme.code_block_border,
        &theme.quote,
        &theme.quote_border,
        &theme.hr,
        &theme.list_bullet,
        &theme.bold,
        &theme.italic,
        &theme.strikethrough,
        &theme.underline,
    ]
}

/// The host highlighting behind `highlight`, or `highlight` staged under a fresh identity.
fn stage_highlight<I: WidgetImports>(
    imports: &I,
    highlight: &Highlight,
) -> Staged<I, HostHighlighter<I>> {
    if let Some(host) = highlight.0.host::<HostHighlighter<I>>() {
        return Staged::Host(host);
    }
    let identity = Identity::new(imports);
    identity.stage(highlight.clone());
    Staged::Authored(identity)
}

/// A style that applies the lent host style.
pub(super) fn host_style<I: WidgetImports>(imports: &I, style: I::Style) -> Style {
    let host = Rc::new(HostStyle::<I> { style });
    let (imports, owner) = (imports.clone(), host.clone());
    Style(crate::Supplied::lent(
        Rc::new(move |text: &str| imports.style_apply(&owner.style, text)),
        host,
    ))
}

impl<I: super::providers::EditorImports> WidgetHost for Builder<I> {
    fn overlay_options(
        &self,
        options: crate::OverlayOptions,
    ) -> ExtensionResult<crate::OverlayOptionsHandle> {
        super::overlays::options(&self.0, &options)
    }
    fn theme(&self) -> ExtensionResult<crate::Theme> {
        Ok(super::theme::theme(self.0.clone(), self.0.global_theme()?))
    }
    fn text(
        &self,
        text: &str,
        padding: (u32, u32),
        background: Option<&Style>,
    ) -> ExtensionResult<Widget> {
        let widget = with_style(&self.0, background, |background| {
            self.0.new_text(text, padding, background)
        })?;
        Ok(HostWidget::retain(&self.0, widget))
    }
    fn r#box(&self, padding: (u32, u32), background: Option<&Style>) -> ExtensionResult<Widget> {
        let widget = with_style(&self.0, background, |background| {
            self.0.new_box(padding, background)
        })?;
        Ok(HostWidget::retain(&self.0, widget))
    }
    fn container(&self) -> ExtensionResult<Widget> {
        let widget = self.0.new_container()?;
        Ok(HostWidget::retain(&self.0, widget))
    }
    fn spacer(&self, lines: u32) -> ExtensionResult<Widget> {
        let widget = self.0.new_spacer(lines)?;
        Ok(HostWidget::retain(&self.0, widget))
    }
    fn markdown(
        &self,
        text: &str,
        padding: (u32, u32),
        theme: &MarkdownTheme,
        style: Option<&DefaultTextStyle>,
    ) -> ExtensionResult<Widget> {
        let widget = self.build_markdown(text, padding, theme, style)?;
        Ok(HostWidget::retain(&self.0, widget))
    }
    fn dynamic_border(&self, color: Option<&Style>) -> ExtensionResult<Widget> {
        let widget = with_style(&self.0, color, |color| self.0.new_dynamic_border(color))?;
        Ok(HostWidget::retain(&self.0, widget))
    }
    fn markdown_theme(&self) -> ExtensionResult<MarkdownTheme> {
        let (styles, highlighter) = self.0.get_markdown_theme()?;
        let mut styles = styles.into_iter().map(|style| host_style(&self.0, style));
        let mut next = || {
            styles
                .next()
                .ok_or_else(|| "the host lent too few Markdown styles".to_owned())
        };
        let host = Rc::new(HostHighlighter::<I> { highlighter });
        let (imports, owner) = (self.0.clone(), host.clone());
        let highlight = Highlight(crate::Supplied::lent(
            Rc::new(move |code: &str, lang: Option<&str>| {
                imports.highlight(&owner.highlighter, code, lang)
            }),
            host,
        ));
        Ok(MarkdownTheme {
            heading: next()?,
            link: next()?,
            link_url: next()?,
            code: next()?,
            code_block: next()?,
            code_block_border: next()?,
            quote: next()?,
            quote_border: next()?,
            hr: next()?,
            list_bullet: next()?,
            bold: next()?,
            italic: next()?,
            strikethrough: next()?,
            underline: next()?,
            highlight_code: Some(highlight),
            code_block_indent: None,
        })
    }
    fn select_list(
        &self,
        items: &[crate::SelectItem],
        max_visible: u32,
        theme: &crate::SelectListTheme,
        layout: &crate::SelectListLayoutOptions,
    ) -> ExtensionResult<Widget> {
        let widget = self.build_select_list(items, max_visible, theme, layout)?;
        Ok(HostWidget::retain(&self.0, widget))
    }
    fn settings_list(
        &self,
        (items, max_visible): (Vec<crate::SettingItem>, u32),
        theme: &crate::SettingsListTheme,
        callbacks: (crate::SettingChange, crate::WidgetNotice),
        options: crate::SettingsListOptions,
    ) -> ExtensionResult<Widget> {
        let widget = self.build_settings_list((&items, max_visible), theme, callbacks, options)?;
        Ok(HostWidget::retain(&self.0, widget))
    }
    fn bordered_loader(
        &self,
        tui: &crate::TUI,
        theme: &crate::Theme,
        message: &str,
        cancellable: Option<bool>,
    ) -> ExtensionResult<Widget> {
        let widget = self.build_bordered_loader(tui, theme, message, cancellable)?;
        Ok(HostWidget::retain(&self.0, widget))
    }
    fn select_list_theme(&self) -> ExtensionResult<crate::SelectListTheme> {
        self.lent_select_list_theme()
    }
    fn settings_list_theme(&self) -> ExtensionResult<crate::SettingsListTheme> {
        self.lent_settings_list_theme()
    }
    fn keybindings(
        &self,
        user_bindings: crate::KeybindingsConfig,
        config_path: Option<&str>,
    ) -> ExtensionResult<crate::KeybindingsManager> {
        self.application_keybindings(&user_bindings, config_path)
    }
    fn create_keybindings(
        &self,
        agent_dir: Option<&str>,
    ) -> ExtensionResult<crate::KeybindingsManager> {
        let manager = self.0.create_keybindings(agent_dir)?;
        Ok(super::keys::lend(&self.0, manager))
    }
    fn keybindings_with_definitions(
        &self,
        definitions: crate::KeybindingDefinitions,
        user_bindings: crate::KeybindingsConfig,
    ) -> ExtensionResult<crate::KeybindingsManager> {
        let manager = self
            .0
            .keybindings_with_definitions(&definitions, &user_bindings)?;
        Ok(super::keys::lend(&self.0, manager))
    }
    fn get_keybindings(&self) -> ExtensionResult<crate::KeybindingsManager> {
        let manager = self.0.get_keybindings()?;
        Ok(super::keys::lend(&self.0, manager))
    }
    fn key_text(&self, keybinding: &str) -> ExtensionResult<String> {
        self.hint(super::widget_imports::KeyHint::Text(keybinding))
    }
    fn key_hint(&self, keybinding: &str, description: &str) -> ExtensionResult<String> {
        self.hint(super::widget_imports::KeyHint::Keybinding(
            keybinding,
            description,
        ))
    }
    fn raw_key_hint(&self, key: &str, description: &str) -> ExtensionResult<String> {
        self.hint(super::widget_imports::KeyHint::Raw(key, description))
    }
    fn editor_theme(&self) -> ExtensionResult<crate::EditorTheme> {
        self.lent_editor_theme()
    }
    fn editor(
        &self,
        tui: &crate::TUI,
        theme: &crate::EditorTheme,
        options: crate::EditorOptions,
    ) -> ExtensionResult<(Widget, Rc<dyn crate::EditorPort>)> {
        let widget = HostWidget::shared(&self.0, self.build_editor(tui, theme, None, options)?);
        Ok((Widget::new(widget.clone()), widget))
    }
    fn custom_editor(
        &self,
        tui: &crate::TUI,
        theme: &crate::EditorTheme,
        keybindings: &crate::KeybindingsManager,
        options: crate::EditorOptions,
    ) -> ExtensionResult<crate::tui::CustomEditorParts> {
        let built = self.build_editor(tui, theme, Some(keybindings), options)?;
        let widget = HostWidget::shared(&self.0, built);
        Ok((Widget::new(widget.clone()), widget.clone(), widget))
    }
    fn foreign_editor(
        &self,
        factory: &dyn std::any::Any,
        tui: &crate::TUI,
        theme: &crate::EditorTheme,
        keybindings: &crate::KeybindingsManager,
    ) -> ExtensionResult<crate::EditorFactoryOutcome> {
        self.build_foreign(factory, tui, theme, keybindings)
    }
}

/// Styles text with the authored style under `handler`.
///
/// # Errors
/// Returns a message when no style is registered under `handler`, or the style's failure.
pub(crate) fn invoke_style(handler: u32, text: &str) -> ExtensionResult<String> {
    callbacks::find::<Style>(handler)?.apply(text)
}

/// Highlights code with the authored highlighting under `handler`.
///
/// # Errors
/// Returns a message when no highlighting is registered under `handler`, or its failure.
pub(crate) fn invoke_highlight(
    handler: u32,
    code: &str,
    lang: Option<&str>,
) -> ExtensionResult<Vec<String>> {
    callbacks::find::<Highlight>(handler)?.highlight(code, lang)
}

/// The authored component staged under `id`.
///
/// # Errors
/// Returns a message when no component is registered under `id`.
#[cfg(test)]
pub(crate) fn authored_component(id: u32) -> ExtensionResult<Component> {
    callbacks::find::<Component>(id)
}

thread_local! {
    /// The widget host of this instance: the component's own, or the one a test installed.
    static INSTALLED: RefCell<Option<Rc<dyn WidgetHost>>> = const { RefCell::new(None) };
}

/// Makes `host` build this instance's widgets.
pub(crate) fn install(host: Rc<dyn WidgetHost>) {
    INSTALLED.set(Some(host));
}

/// The host that builds widgets.
///
/// # Errors
/// Returns a message before the extension starts.
pub(crate) fn widget_host() -> ExtensionResult<Rc<dyn WidgetHost>> {
    INSTALLED
        .with_borrow(Clone::clone)
        .ok_or_else(|| "no widget host is installed".to_owned())
}
