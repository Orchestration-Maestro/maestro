//! Widget requests through the generated host bindings.

use std::rc::Rc;

use super::{Generated, route};
use crate::bindings::maestro::extension::host;
use crate::component_adapter::{
    Builder, ChildKey, ChildRef, EditorAnswer, EditorRequest, EditorStyles, HandlerSlot,
    HighlightRef, HostMarkdownTheme, HostSettingsTheme, KeyHint, KeybindingsAnswer,
    KeybindingsRequest, LayoutRef, MarkdownStyles, RowStyleRef, SettingRef, SettingsStyles,
    StyleRef, TextStyle, WidgetImports, WriterAnswer, WriterRequest,
};
use crate::types::ExtensionResult;
use crate::{KeybindingDefinitions, KeybindingsConfig, SelectItem};

/// The host that builds this component's widgets.
pub(super) fn widget_host() -> Rc<dyn crate::WidgetHost> {
    Rc::new(Builder(Generated))
}

/// The wire form of a styling operand.
pub(super) fn style<'a>(style: &StyleRef<'a, Generated>) -> host::StyleSource<'a> {
    match *style {
        StyleRef::Host(style) => host::StyleSource::Host(style),
        StyleRef::Authored(callback) => host::StyleSource::Authored(callback),
    }
}

/// The wire form of Markdown styles in field order.
fn markdown_styles(
    theme: MarkdownStyles<'_, Generated>,
) -> ExtensionResult<host::MarkdownStyles<'_>> {
    let mut roles = theme.roles.iter().map(style);
    let mut next = || {
        roles
            .next()
            .ok_or_else(|| "a Markdown style is missing".to_owned())
    };
    Ok(host::MarkdownStyles {
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
        highlight_code: theme.highlight.map(|highlight| match highlight {
            HighlightRef::Host(host) => host::HighlightSource::Host(host),
            HighlightRef::Authored(callback) => host::HighlightSource::Authored(callback),
        }),
        code_block_indent: theme.indent.map(str::to_owned),
    })
}

impl WidgetImports for Generated {
    fn global_theme(&self) -> ExtensionResult<host::Theme> {
        host::global_theme()
    }
    type Widget = host::Widget;
    type Style = host::Style;
    type Highlighter = host::Highlighter;

    fn new_text(
        &self,
        text: &str,
        (padding_x, padding_y): (u32, u32),
        background: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<host::Widget> {
        host::new_text(
            text,
            padding_x,
            padding_y,
            background.as_ref().map(style).as_ref(),
        )
    }
    fn new_box(
        &self,
        (padding_x, padding_y): (u32, u32),
        background: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<host::Widget> {
        host::new_box(
            padding_x,
            padding_y,
            background.as_ref().map(style).as_ref(),
        )
    }
    fn new_container(&self) -> ExtensionResult<host::Widget> {
        host::new_container()
    }
    fn new_spacer(&self, lines: u32) -> ExtensionResult<host::Widget> {
        host::new_spacer(lines)
    }
    fn new_markdown(
        &self,
        text: &str,
        (padding_x, padding_y): (u32, u32),
        theme: MarkdownStyles<'_, Self>,
        text_style: Option<TextStyle<'_, Self>>,
    ) -> ExtensionResult<host::Widget> {
        let text_style = text_style.map(|text_style| host::DefaultTextStyle {
            color: text_style.color.as_ref().map(style),
            bg_color: text_style.bg_color.as_ref().map(style),
            decorations: text_style
                .decorations
                .iter()
                .map(|decoration| match decoration {
                    crate::TextDecoration::Bold => host::TextDecoration::Bold,
                    crate::TextDecoration::Italic => host::TextDecoration::Italic,
                    crate::TextDecoration::Strikethrough => host::TextDecoration::Strikethrough,
                    crate::TextDecoration::Underline => host::TextDecoration::Underline,
                })
                .collect(),
        });
        let theme = markdown_styles(theme)?;
        host::new_markdown(text, padding_x, padding_y, &theme, text_style.as_ref())
    }
    fn new_dynamic_border(
        &self,
        color: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<host::Widget> {
        host::new_dynamic_border(color.as_ref().map(style).as_ref())
    }
    fn get_markdown_theme(&self) -> ExtensionResult<HostMarkdownTheme<Self>> {
        let theme = host::get_markdown_theme()?;
        let styles = vec![
            theme.heading,
            theme.link,
            theme.link_url,
            theme.code,
            theme.code_block,
            theme.code_block_border,
            theme.quote,
            theme.quote_border,
            theme.hr,
            theme.list_bullet,
            theme.bold,
            theme.italic,
            theme.strikethrough,
            theme.underline,
        ];
        Ok((styles, theme.highlight_code))
    }
    fn widget_render(&self, widget: &host::Widget, width: u32) -> ExtensionResult<Vec<String>> {
        route::lines(&widget.render(width))
    }
    fn widget_invalidate(&self, widget: &host::Widget) -> ExtensionResult<()> {
        route::drive(&widget.invalidate())
    }
    fn widget_set_text(&self, widget: &host::Widget, text: &str) -> ExtensionResult<()> {
        widget.set_text(text)
    }
    fn widget_set_background(
        &self,
        widget: &host::Widget,
        background: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<()> {
        widget.set_background(background.as_ref().map(style).as_ref())
    }
    fn widget_set_lines(&self, widget: &host::Widget, lines: u32) -> ExtensionResult<()> {
        widget.set_lines(lines)
    }
    fn widget_add_child(
        &self,
        widget: &host::Widget,
        child: ChildRef<'_, Self>,
    ) -> ExtensionResult<()> {
        widget.add_child(&match child {
            ChildRef::Widget(child) => host::WidgetChild::Widget(child),
            ChildRef::Component(callback) => host::WidgetChild::Component(callback),
        })
    }
    fn widget_remove_child(
        &self,
        widget: &host::Widget,
        child: ChildKey<'_, Self>,
    ) -> ExtensionResult<()> {
        widget.remove_child(&match child {
            ChildKey::Widget(child) => host::ChildKey::Widget(child),
            ChildKey::Component(id) => host::ChildKey::Component(id),
        })
    }
    fn widget_clear(&self, widget: &host::Widget) -> ExtensionResult<()> {
        widget.clear()
    }
    fn tui_writer(&self, tui: &host::Tui) -> ExtensionResult<host::Widget> {
        tui.writer()
    }
    fn widget_writer(
        &self,
        widget: &host::Widget,
        request: WriterRequest<'_, Self>,
    ) -> ExtensionResult<WriterAnswer<Self>> {
        super::writer::request(widget, &request)
    }
    fn widget_add_input_listener(
        &self,
        widget: &host::Widget,
        listener: &host::Callback,
    ) -> ExtensionResult<host::Subscription> {
        widget.add_input_listener(listener)
    }
    fn widget_drain_input<'a>(
        &'a self,
        widget: &'a host::Widget,
        max_ms: Option<f64>,
        idle_ms: Option<f64>,
    ) -> crate::ExtensionFuture<'a, ()> {
        Box::pin(widget.drain_input(max_ms, idle_ms))
    }
    fn style_apply(&self, style: &host::Style, text: &str) -> ExtensionResult<String> {
        style.apply(text)
    }
    fn highlight(
        &self,
        highlighter: &host::Highlighter,
        code: &str,
        lang: Option<&str>,
    ) -> ExtensionResult<Vec<String>> {
        highlighter.highlight(code, lang)
    }
    type RowStyle = host::RowStyle;
    type SubmenuDone = host::SubmenuDone;

    fn new_select_list(
        &self,
        items: &[SelectItem],
        max_visible: u32,
        theme: Vec<StyleRef<'_, Self>>,
        (min, max, truncate): LayoutRef<'_, Self>,
    ) -> ExtensionResult<host::Widget> {
        let items: Vec<_> = items.iter().map(select_item).collect();
        let theme = super::editors::select_styles(&theme)?;
        let layout = host::SelectLayout {
            min_primary_column_width: min,
            max_primary_column_width: max,
            truncate_primary: truncate,
        };
        host::new_select_list(&items, max_visible, &theme, &layout)
    }
    fn new_settings_list(
        &self,
        (items, max_visible): (&[SettingRef<'_, Self>], u32),
        theme: SettingsStyles<'_, Self>,
        (on_change, on_cancel): (&host::Callback, &host::Callback),
        enable_search: bool,
    ) -> ExtensionResult<host::Widget> {
        let items: Vec<_> = items
            .iter()
            .map(|setting| host::SettingItem {
                id: setting.item.id.clone(),
                label: setting.item.label.clone(),
                description: setting.item.description.clone(),
                current_value: setting.item.current_value.clone(),
                values: setting.item.values.clone(),
                submenu: setting.submenu,
            })
            .collect();
        let theme = host::SettingsListStyles {
            label: row(&theme.label),
            value: row(&theme.value),
            description: style(&theme.description),
            cursor: theme.cursor.to_owned(),
            hint: style(&theme.hint),
        };
        host::new_settings_list(
            &items,
            max_visible,
            &theme,
            on_change,
            on_cancel,
            enable_search,
        )
    }
    fn new_bordered_loader(
        &self,
        tui: &host::Tui,
        theme: &host::Theme,
        message: &str,
        cancellable: Option<bool>,
    ) -> ExtensionResult<host::Widget> {
        host::new_bordered_loader(tui, theme, message, cancellable)
    }
    fn get_select_list_theme(&self) -> ExtensionResult<Vec<host::Style>> {
        let theme = host::get_select_list_theme()?;
        Ok(vec![
            theme.selected_prefix,
            theme.selected_text,
            theme.description,
            theme.scroll_info,
            theme.no_match,
        ])
    }
    fn get_settings_list_theme(&self) -> ExtensionResult<HostSettingsTheme<Self>> {
        let theme = host::get_settings_list_theme()?;
        Ok((
            theme.label,
            theme.value,
            theme.description,
            theme.cursor,
            theme.hint,
        ))
    }
    fn widget_handle_input(&self, widget: &host::Widget, data: &str) -> ExtensionResult<()> {
        route::drive(&widget.handle_input(data))
    }
    fn widget_set_filter(&self, widget: &host::Widget, filter: &str) -> ExtensionResult<()> {
        widget.set_filter(filter)
    }
    fn widget_set_selected_index(&self, widget: &host::Widget, index: i64) -> ExtensionResult<()> {
        widget.set_selected_index(index)
    }
    fn widget_get_selected_item(
        &self,
        widget: &host::Widget,
    ) -> ExtensionResult<Option<SelectItem>> {
        Ok(widget.get_selected_item()?.map(super::widget_events::item))
    }
    fn widget_set_handler(
        &self,
        widget: &host::Widget,
        slot: HandlerSlot,
        handler: Option<&host::Callback>,
    ) -> ExtensionResult<()> {
        let slot = match slot {
            HandlerSlot::Select => host::HandlerSlot::Select,
            HandlerSlot::Cancel => host::HandlerSlot::Cancel,
            HandlerSlot::SelectionChange => host::HandlerSlot::SelectionChange,
            HandlerSlot::Abort => host::HandlerSlot::Abort,
            HandlerSlot::Submit => host::HandlerSlot::Submit,
            HandlerSlot::Change => host::HandlerSlot::Change,
            HandlerSlot::Escape => host::HandlerSlot::Escape,
            HandlerSlot::CtrlD => host::HandlerSlot::CtrlD,
            HandlerSlot::PasteImage => host::HandlerSlot::PasteImage,
            HandlerSlot::ExtensionShortcut => host::HandlerSlot::ExtensionShortcut,
        };
        widget.set_handler(slot, handler)
    }
    fn new_editor(
        &self,
        tui: &host::Tui,
        theme: EditorStyles<'_, Self>,
        options: crate::EditorOptions,
    ) -> ExtensionResult<host::Widget> {
        let (theme, options) = super::editors::wire(&theme, options)?;
        host::new_editor(tui, &theme, options)
    }
    fn new_custom_editor(
        &self,
        tui: &host::Tui,
        theme: EditorStyles<'_, Self>,
        keybindings: &host::KeybindingsManager,
        options: crate::EditorOptions,
    ) -> ExtensionResult<host::Widget> {
        let (theme, options) = super::editors::wire(&theme, options)?;
        host::new_custom_editor(tui, &theme, keybindings, options)
    }
    fn widget_editor(
        &self,
        widget: &host::Widget,
        request: EditorRequest<'_, Self>,
    ) -> ExtensionResult<EditorAnswer> {
        super::editors::request(widget, request)
    }
    fn widget_on_action(
        &self,
        widget: &host::Widget,
        action: &str,
        handler: &host::Callback,
    ) -> ExtensionResult<()> {
        widget.on_action(action, handler)
    }
    fn widget_update_value(
        &self,
        widget: &host::Widget,
        id: &str,
        value: &str,
    ) -> ExtensionResult<()> {
        widget.update_value(id, value)
    }
    fn widget_signal(&self, widget: &host::Widget) -> ExtensionResult<host::AbortSignal> {
        widget.signal()
    }
    fn widget_dispose(&self, widget: &host::Widget) -> ExtensionResult<()> {
        widget.dispose()
    }
    fn row_style_apply(
        &self,
        style: &host::RowStyle,
        text: &str,
        selected: bool,
    ) -> ExtensionResult<String> {
        style.apply(text, selected)
    }
    fn submenu_done(&self, done: &host::SubmenuDone, value: Option<&str>) -> ExtensionResult<()> {
        route::drive(&done.done(value))
    }
    type Keybindings = host::KeybindingsManager;
    fn new_keybindings(
        &self,
        user_bindings: &KeybindingsConfig,
        config_path: Option<&str>,
    ) -> ExtensionResult<host::KeybindingsManager> {
        host::new_keybindings_manager(&super::keys::wire_config(user_bindings), config_path)
    }
    fn create_keybindings(
        &self,
        agent_dir: Option<&str>,
    ) -> ExtensionResult<host::KeybindingsManager> {
        host::create_keybindings_manager(agent_dir)
    }
    fn keybindings_with_definitions(
        &self,
        definitions: &KeybindingDefinitions,
        user_bindings: &KeybindingsConfig,
    ) -> ExtensionResult<host::KeybindingsManager> {
        let definitions: Vec<_> = definitions
            .iter()
            .map(|(id, definition)| {
                let wire = host::KeybindingDefinition {
                    default_keys: super::keys::wire_keys(&definition.default_keys),
                    description: definition.description.clone(),
                };
                (id.clone(), wire)
            })
            .collect();
        host::new_keybindings_with_definitions(
            &definitions,
            &super::keys::wire_config(user_bindings),
        )
    }
    fn get_keybindings(&self) -> ExtensionResult<host::KeybindingsManager> {
        host::get_keybindings()
    }
    fn keybindings_request(
        &self,
        manager: &host::KeybindingsManager,
        request: KeybindingsRequest<'_>,
    ) -> ExtensionResult<KeybindingsAnswer> {
        super::keys::request(manager, request)
    }
    fn key_hint(&self, hint: KeyHint<'_>) -> ExtensionResult<String> {
        match hint {
            KeyHint::Text(id) => host::key_text(id),
            KeyHint::Keybinding(id, description) => host::key_hint(id, description),
            KeyHint::Raw(key, description) => host::raw_key_hint(key, description),
        }
    }
    fn get_editor_theme(&self) -> ExtensionResult<(host::Style, Vec<host::Style>)> {
        let theme = host::get_editor_theme()?;
        let list = theme.select_list;
        let styles = vec![
            list.selected_prefix,
            list.selected_text,
            list.description,
            list.scroll_info,
            list.no_match,
        ];
        Ok((theme.border_color, styles))
    }
}

/// The wire form of a row styling operand.
fn row<'a>(style: &RowStyleRef<'a, Generated>) -> host::RowStyleSource<'a> {
    match *style {
        RowStyleRef::Host(style) => host::RowStyleSource::Host(style),
        RowStyleRef::Authored(callback) => host::RowStyleSource::Authored(callback),
    }
}

/// The wire form of a choice.
fn select_item(item: &SelectItem) -> host::SelectItem {
    host::SelectItem {
        value: item.value.clone(),
        label: item.label.clone(),
        description: item.description.clone(),
    }
}
