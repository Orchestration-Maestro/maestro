//! The widget requests of the host at the wire level.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use super::Imports;
use crate::{
    ExtensionResult, KeybindingConflict, KeybindingDefinition, KeybindingDefinitions,
    KeybindingsConfig, SelectItem, SelectListTruncatePrimaryContext, TextDecoration,
};

/// A styling operand: a host style, or the identity of an authored one.
pub(crate) enum StyleRef<'a, I: WidgetImports> {
    /// A style the host supplied.
    Host(&'a I::Style),
    /// An authored style, staged under this identity.
    Authored(&'a I::Callback),
}

/// A highlighting operand: the host's highlighting, or the identity of an authored one.
pub(crate) enum HighlightRef<'a, I: WidgetImports> {
    /// Highlighting the host supplied.
    Host(&'a I::Highlighter),
    /// Authored highlighting, staged under this identity.
    Authored(&'a I::Callback),
}

/// A child to add: a host widget, or the identity of an authored component.
pub(crate) enum ChildRef<'a, I: WidgetImports> {
    /// A host widget.
    Widget(&'a I::Widget),
    /// An authored component, staged under this identity.
    Component(&'a I::Callback),
}

/// A child to remove: a host widget, or the identity an authored component was added under.
pub(crate) enum ChildKey<'a, I: WidgetImports> {
    /// A host widget.
    Widget(&'a I::Widget),
    /// The identity of an added authored component.
    Component(u32),
}

/// Markdown styles as operands, in the order of [`MarkdownTheme`]'s fields.
pub(crate) struct MarkdownStyles<'a, I: WidgetImports> {
    /// The fourteen element styles.
    pub(crate) roles: Vec<StyleRef<'a, I>>,
    /// Code highlighting.
    pub(crate) highlight: Option<HighlightRef<'a, I>>,
    /// Code row prefix.
    pub(crate) indent: Option<&'a str>,
}

/// A base text style as operands.
pub(crate) struct TextStyle<'a, I: WidgetImports> {
    /// Foreground.
    pub(crate) color: Option<StyleRef<'a, I>>,
    /// Row background.
    pub(crate) bg_color: Option<StyleRef<'a, I>>,
    /// Decorations to apply.
    pub(crate) decorations: &'a [TextDecoration],
}

/// The live theme's Markdown styles as the host lends them, in field order.
pub(crate) type HostMarkdownTheme<I> = (
    Vec<<I as WidgetImports>::Style>,
    <I as WidgetImports>::Highlighter,
);

/// The widget requests of the host at the wire level.
pub(crate) trait WidgetImports: Imports {
    /// Retains the host's live theme without acquiring a UI.
    fn global_theme(&self) -> ExtensionResult<Self::Theme>;
    /// A host widget.
    type Widget: 'static;
    /// A host styling function.
    type Style: 'static;
    /// The host's code highlighting.
    type Highlighter: 'static;
    /// Builds wrapped text.
    fn new_text(
        &self,
        text: &str,
        padding: (u32, u32),
        background: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<Self::Widget>;
    /// Builds a box.
    fn new_box(
        &self,
        padding: (u32, u32),
        background: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<Self::Widget>;
    /// Builds a container.
    fn new_container(&self) -> ExtensionResult<Self::Widget>;
    /// Builds a spacer.
    fn new_spacer(&self, lines: u32) -> ExtensionResult<Self::Widget>;
    /// Builds Markdown.
    fn new_markdown(
        &self,
        text: &str,
        padding: (u32, u32),
        theme: MarkdownStyles<'_, Self>,
        style: Option<TextStyle<'_, Self>>,
    ) -> ExtensionResult<Self::Widget>;
    /// Builds a rule.
    fn new_dynamic_border(
        &self,
        color: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<Self::Widget>;
    /// Lends the live theme's Markdown styles.
    fn get_markdown_theme(&self) -> ExtensionResult<HostMarkdownTheme<Self>>;
    /// Renders rows, running the nested calls the host makes first.
    fn widget_render(&self, widget: &Self::Widget, width: u32) -> ExtensionResult<Vec<String>>;
    /// Drops cached rows, running the nested calls the host makes first.
    fn widget_invalidate(&self, widget: &Self::Widget) -> ExtensionResult<()>;
    /// Replaces the text.
    fn widget_set_text(&self, widget: &Self::Widget, text: &str) -> ExtensionResult<()>;
    /// Replaces or removes the row background.
    fn widget_set_background(
        &self,
        widget: &Self::Widget,
        style: Option<StyleRef<'_, Self>>,
    ) -> ExtensionResult<()>;
    /// Replaces the row count.
    fn widget_set_lines(&self, widget: &Self::Widget, lines: u32) -> ExtensionResult<()>;
    /// Appends a child.
    fn widget_add_child(
        &self,
        widget: &Self::Widget,
        child: ChildRef<'_, Self>,
    ) -> ExtensionResult<()>;
    /// Removes the first occurrence of a child.
    fn widget_remove_child(
        &self,
        widget: &Self::Widget,
        child: ChildKey<'_, Self>,
    ) -> ExtensionResult<()>;
    /// Removes every child.
    fn widget_clear(&self, widget: &Self::Widget) -> ExtensionResult<()>;
    /// The frame writer behind `tui` as a container widget.
    fn tui_writer(&self, tui: &Self::Tui) -> ExtensionResult<Self::Widget>;
    /// One request on a frame writer widget, running the nested calls the host makes first.
    fn widget_writer(
        &self,
        widget: &Self::Widget,
        request: WriterRequest<'_, Self>,
    ) -> ExtensionResult<WriterAnswer<Self>>;
    /// Adds the frame writer's input listener staged under `listener`.
    fn widget_add_input_listener(
        &self,
        widget: &Self::Widget,
        listener: &Self::Callback,
    ) -> ExtensionResult<Self::Subscription>;
    /// Discards pending input of the frame writer's terminal.
    fn widget_drain_input<'a>(
        &'a self,
        widget: &'a Self::Widget,
        max_ms: Option<f64>,
        idle_ms: Option<f64>,
    ) -> crate::ExtensionFuture<'a, ()>;
    /// Applies a host style.
    fn style_apply(&self, style: &Self::Style, text: &str) -> ExtensionResult<String>;
    /// Runs the host's highlighting.
    fn highlight(
        &self,
        highlighter: &Self::Highlighter,
        code: &str,
        lang: Option<&str>,
    ) -> ExtensionResult<Vec<String>>;
    /// A host settings row style.
    type RowStyle: 'static;
    /// Closes a host settings submenu.
    type SubmenuDone: 'static;
    /// Builds a selection list.
    fn new_select_list(
        &self,
        items: &[SelectItem],
        max_visible: u32,
        theme: Vec<StyleRef<'_, Self>>,
        layout: LayoutRef<'_, Self>,
    ) -> ExtensionResult<Self::Widget>;
    /// Builds a settings list reporting to the two callbacks.
    fn new_settings_list(
        &self,
        items: (&[SettingRef<'_, Self>], u32),
        theme: SettingsStyles<'_, Self>,
        callbacks: (&Self::Callback, &Self::Callback),
        enable_search: bool,
    ) -> ExtensionResult<Self::Widget>;
    /// Builds a bordered loader on a lent terminal.
    fn new_bordered_loader(
        &self,
        tui: &Self::Tui,
        theme: &Self::Theme,
        message: &str,
        cancellable: Option<bool>,
    ) -> ExtensionResult<Self::Widget>;
    /// Lends the live theme's selection list styles in field order.
    fn get_select_list_theme(&self) -> ExtensionResult<Vec<Self::Style>>;
    /// Lends the live theme's settings list styles.
    fn get_settings_list_theme(&self) -> ExtensionResult<HostSettingsTheme<Self>>;
    /// Delivers input, running the nested calls the host makes first.
    fn widget_handle_input(&self, widget: &Self::Widget, data: &str) -> ExtensionResult<()>;
    /// Filters a selection list.
    fn widget_set_filter(&self, widget: &Self::Widget, filter: &str) -> ExtensionResult<()>;
    /// Selects a row of a selection list.
    fn widget_set_selected_index(&self, widget: &Self::Widget, index: i64) -> ExtensionResult<()>;
    /// The selected choice of a selection list.
    fn widget_get_selected_item(
        &self,
        widget: &Self::Widget,
    ) -> ExtensionResult<Option<SelectItem>>;
    /// Replaces or removes one handler.
    fn widget_set_handler(
        &self,
        widget: &Self::Widget,
        slot: HandlerSlot,
        handler: Option<&Self::Callback>,
    ) -> ExtensionResult<()>;
    /// Replaces a setting's current value.
    fn widget_update_value(
        &self,
        widget: &Self::Widget,
        id: &str,
        value: &str,
    ) -> ExtensionResult<()>;
    /// A loader's signal.
    fn widget_signal(&self, widget: &Self::Widget) -> ExtensionResult<Self::Signal>;
    /// Stops a loader's animation.
    fn widget_dispose(&self, widget: &Self::Widget) -> ExtensionResult<()>;
    /// Applies a host row style.
    fn row_style_apply(
        &self,
        style: &Self::RowStyle,
        text: &str,
        selected: bool,
    ) -> ExtensionResult<String>;
    /// Closes a submenu, running the nested calls the host makes first.
    fn submenu_done(&self, done: &Self::SubmenuDone, value: Option<&str>) -> ExtensionResult<()>;
    /// A host keybindings manager.
    type Keybindings: 'static;
    /// A manager of the application's keybindings.
    fn new_keybindings(
        &self,
        user_bindings: &KeybindingsConfig,
        config_path: Option<&str>,
    ) -> ExtensionResult<Self::Keybindings>;
    /// A manager of the application's keybindings loaded from an agent directory.
    fn create_keybindings(&self, agent_dir: Option<&str>) -> ExtensionResult<Self::Keybindings>;
    /// A manager of the supplied keybindings.
    fn keybindings_with_definitions(
        &self,
        definitions: &KeybindingDefinitions,
        user_bindings: &KeybindingsConfig,
    ) -> ExtensionResult<Self::Keybindings>;
    /// The keybindings the host's terminal uses.
    fn get_keybindings(&self) -> ExtensionResult<Self::Keybindings>;
    /// One request on a keybindings manager.
    fn keybindings_request(
        &self,
        manager: &Self::Keybindings,
        request: KeybindingsRequest<'_>,
    ) -> ExtensionResult<KeybindingsAnswer>;
    /// A keybinding's keys, or a hint of a keybinding or a literal key.
    fn key_hint(&self, hint: KeyHint<'_>) -> ExtensionResult<String>;
    /// Lends the live theme's editor border and selection list styles in field order.
    fn get_editor_theme(&self) -> ExtensionResult<(Self::Style, Vec<Self::Style>)>;
    /// Builds the toolkit editor on a lent terminal.
    fn new_editor(
        &self,
        tui: &Self::Tui,
        theme: EditorStyles<'_, Self>,
        options: crate::EditorOptions,
    ) -> ExtensionResult<Self::Widget>;
    /// Builds the application's custom editor on a lent terminal.
    fn new_custom_editor(
        &self,
        tui: &Self::Tui,
        theme: EditorStyles<'_, Self>,
        keybindings: &Self::Keybindings,
        options: crate::EditorOptions,
    ) -> ExtensionResult<Self::Widget>;
    /// One request on an editor, running the nested calls the host makes first.
    fn widget_editor(
        &self,
        widget: &Self::Widget,
        request: EditorRequest<'_, Self>,
    ) -> ExtensionResult<EditorAnswer>;
    /// Replaces the handler of a custom editor's application action.
    fn widget_on_action(
        &self,
        widget: &Self::Widget,
        action: &str,
        handler: &Self::Callback,
    ) -> ExtensionResult<()>;
}

/// A settings row styling operand: a host row style, or the identity of an authored one.
pub(crate) enum RowStyleRef<'a, I: WidgetImports> {
    /// A row style the host supplied.
    Host(&'a I::RowStyle),
    /// An authored row style, staged under this identity.
    Authored(&'a I::Callback),
}

/// Settings list styles as operands.
pub(crate) struct SettingsStyles<'a, I: WidgetImports> {
    /// Labels.
    pub(crate) label: RowStyleRef<'a, I>,
    /// Values.
    pub(crate) value: RowStyleRef<'a, I>,
    /// Descriptions.
    pub(crate) description: StyleRef<'a, I>,
    /// The selected row's prefix.
    pub(crate) cursor: &'a str,
    /// The hint row.
    pub(crate) hint: StyleRef<'a, I>,
}

/// One setting as an operand, with its staged submenu.
pub(crate) struct SettingRef<'a, I: WidgetImports> {
    /// The setting.
    pub(crate) item: &'a crate::SettingItem,
    /// The submenu's identity, when the setting has one.
    pub(crate) submenu: Option<&'a I::Callback>,
}

/// Primary column layout as operands.
pub(crate) type LayoutRef<'a, I> = (
    Option<i64>,
    Option<i64>,
    Option<&'a <I as Imports>::Callback>,
);

/// What focus or an overlay goes to.
pub(crate) enum WriterTarget<'a, I: WidgetImports> {
    /// A host widget.
    Widget(&'a I::Widget),
    /// An authored component, lent now under this identity.
    Component(&'a I::Callback),
    /// The identity an authored component was lent under before.
    Lent(u32),
}

/// One request on the terminal a frame writer draws on.
#[derive(Clone, Copy)]
pub(crate) enum TerminalRequest<'a> {
    /// Stops the terminal.
    Stop,
    /// Writes output.
    Write(&'a str),
    /// Reads the width.
    Columns,
    /// Reads the height.
    Rows,
    /// Reads whether the Kitty keyboard protocol is active.
    KittyProtocolActive,
    /// Moves the cursor by rows.
    MoveBy(i64),
    /// Hides the cursor.
    HideCursor,
    /// Shows the cursor.
    ShowCursor,
    /// Clears the current line.
    ClearLine,
    /// Clears from the cursor to the end of the screen.
    ClearFromCursor,
    /// Clears the screen.
    ClearScreen,
    /// Sets the window title.
    SetTitle(&'a str),
    /// Turns the progress indicator on or off.
    SetProgress(bool),
}

/// One request on a frame writer.
pub(crate) enum WriterRequest<'a, I: WidgetImports> {
    /// Reads whether the hardware cursor is shown.
    GetShowHardwareCursor,
    /// Shows or hides the hardware cursor.
    SetShowHardwareCursor(bool),
    /// Reads whether shrinking content clears the screen.
    GetClearOnShrink,
    /// Changes whether shrinking content clears the screen.
    SetClearOnShrink(bool),
    /// Moves focus.
    SetFocus(Option<WriterTarget<'a, I>>),
    /// Shows an overlay with optional shared placement.
    ShowOverlay(WriterTarget<'a, I>, Option<&'a I::OverlayOptions>),
    /// Removes the most recently shown overlay.
    HideOverlay,
    /// Reads whether any overlay is visible.
    HasOverlay,
    /// Starts the terminal.
    Start,
    /// Stops the terminal.
    Stop,
    /// Requests a frame, a full one when forced.
    RequestRender(bool),
    /// Replaces or removes the debug key's callback.
    SetOnDebug(Option<&'a I::Callback>),
    /// A request on the terminal.
    Terminal(TerminalRequest<'a>),
    /// Starts the terminal with input and resize callbacks.
    TerminalStart(&'a I::Callback, &'a I::Callback),
    /// Reads whether the host still keeps the component lent under an identity.
    LentLive(u32),
}

/// What a frame writer request answers.
pub(crate) enum WriterAnswer<I: WidgetImports> {
    /// Nothing.
    Done,
    /// A flag.
    Flag(bool),
    /// A terminal dimension.
    Count(u32),
    /// A shown overlay's handle.
    Overlay(I::OverlayHandle),
}

/// What a widget tells an authored callback.
pub(crate) enum WidgetEvent<I: WidgetImports> {
    /// A choice was confirmed.
    Select(SelectItem),
    /// The widget was cancelled.
    Cancel,
    /// The selection moved.
    SelectionChange(SelectItem),
    /// A setting changed: its id and new value.
    Change(String, String),
    /// A loader was cancelled.
    Abort,
    /// A row's primary text needs truncating.
    Truncate(SelectListTruncatePrimaryContext),
    /// A settings row's text needs styling.
    Row(String, bool),
    /// A submenu opens with the current value and its closer.
    Submenu(String, I::SubmenuDone),
    /// An editor's submitted or changed text.
    Text(String),
    /// An editor key or action.
    Notice,
    /// Input offered to an editor's interceptor.
    Shortcut(String),
    /// An overlay's visibility check at these terminal columns and rows.
    Visible(u32, u32),
    /// A custom overlay's handle after it is shown.
    OverlayHandle(I::OverlayHandle),
    /// A custom overlay asks for its placement.
    OverlayOptions,
}

/// What an authored widget callback answers.
pub(crate) enum WidgetAnswer {
    /// Nothing.
    Done,
    /// Text.
    Text(String),
    /// A built component.
    Component(crate::Component),
    /// Whether an interceptor consumed its input.
    Consumed(bool),
    /// Whether an overlay is visible.
    Visible(bool),
    /// An overlay's placement.
    OverlayOptions(crate::OverlayOptionsHandle),
}

/// The widget handler an authored callback replaces.
#[derive(Clone, Copy)]
pub(crate) enum HandlerSlot {
    /// Confirmation.
    Select,
    /// Cancellation.
    Cancel,
    /// Selection movement.
    SelectionChange,
    /// A loader's abort.
    Abort,
    /// An editor's submission.
    Submit,
    /// An editor's change.
    Change,
    /// A custom editor's interrupt.
    Escape,
    /// A custom editor's empty-editor exit.
    CtrlD,
    /// A custom editor's image paste.
    PasteImage,
    /// A custom editor's input interceptor.
    ExtensionShortcut,
}

/// Editor styles as operands: the border, and the completion list in field order.
pub(crate) struct EditorStyles<'a, I: WidgetImports> {
    /// The border.
    pub(crate) border: StyleRef<'a, I>,
    /// The completion list.
    pub(crate) select_list: Vec<StyleRef<'a, I>>,
}

/// One request on an editor.
pub(crate) enum EditorRequest<'a, I: WidgetImports> {
    /// The text.
    GetText,
    /// Replaces the text.
    SetText(&'a str),
    /// The expanded text.
    GetExpandedText,
    /// The lines.
    GetLines,
    /// The cursor.
    GetCursor,
    /// Inserts text at the cursor.
    InsertTextAtCursor(&'a str),
    /// Adds text to the history.
    AddToHistory(&'a str),
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
    SetBorderColor(StyleRef<'a, I>),
}

/// What an editor request answers.
pub(crate) enum EditorAnswer {
    /// Nothing.
    Done,
    /// Text.
    Text(String),
    /// Lines.
    Lines(Vec<String>),
    /// A line and column.
    Cursor(u32, u32),
    /// A number.
    Number(f64),
    /// A flag.
    Flag(bool),
}

/// The live theme's settings list styles as the host lends them.
pub(crate) type HostSettingsTheme<I> = (
    <I as WidgetImports>::RowStyle,
    <I as WidgetImports>::RowStyle,
    <I as WidgetImports>::Style,
    String,
    <I as WidgetImports>::Style,
);

/// One request on a keybindings manager.
#[derive(Clone, Copy)]
pub(crate) enum KeybindingsRequest<'a> {
    /// Whether input matches a keybinding.
    Matches(&'a str, &'a str),
    /// A keybinding's keys.
    Keys(&'a str),
    /// A keybinding's definition.
    Definition(&'a str),
    /// The user binding conflicts.
    Conflicts,
    /// Replaces the user bindings.
    SetUserBindings(&'a KeybindingsConfig),
    /// The user bindings.
    UserBindings,
    /// The resolved bindings.
    ResolvedBindings,
    /// Reloads the user bindings.
    Reload,
    /// The effective configuration.
    EffectiveConfig,
}

/// What a keybindings request answers.
pub(crate) enum KeybindingsAnswer {
    /// Nothing.
    Done,
    /// Whether input matched.
    Matched(bool),
    /// Keys.
    Keys(Vec<String>),
    /// A definition, absent for an unknown keybinding.
    Definition(Option<KeybindingDefinition>),
    /// Conflicts.
    Conflicts(Vec<KeybindingConflict>),
    /// Bindings.
    Config(KeybindingsConfig),
}

/// A keybinding hint request.
pub(crate) enum KeyHint<'a> {
    /// The keys of a keybinding.
    Text(&'a str),
    /// A keybinding and a description.
    Keybinding(&'a str, &'a str),
    /// A literal key and a description.
    Raw(&'a str, &'a str),
}
