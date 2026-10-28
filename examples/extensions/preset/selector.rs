//! Themed preset selection through the host's retained list and container widgets.
use super::config::{Preset, Presets};
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, Container, CustomDone, CustomFactoryOutcome,
    DynamicBorder, ExtensionComponent, ExtensionContext, ExtensionResult, SelectItem, SelectList,
    SelectListLayoutOptions, SelectListTheme, Style, TUI, Text, Theme,
};
use std::rc::Rc;

/// Describes the authored settings without resolving a model or opening a file.
fn description(preset: &Preset) -> String {
    let mut parts = Vec::new();
    if let (Some(provider), Some(model)) = (&preset.provider, &preset.model)
        && !provider.is_empty()
        && !model.is_empty()
    {
        parts.push(format!("{provider}/{model}"));
    }
    if let Some(level) = &preset.thinking_level {
        parts.push(format!("thinking:{level}"));
    }
    if let Some(tools) = &preset.tools {
        parts.push(format!("tools:{}", tools.join(",")));
    }
    if let Some(instructions) = &preset.instructions
        && !instructions.is_empty()
    {
        let truncated = if instructions.encode_utf16().count() > 30 {
            format!(
                "{}...",
                String::from_utf16_lossy(&instructions.encode_utf16().take(27).collect::<Vec<_>>())
            )
        } else {
            instructions.clone()
        };
        parts.push(format!("\"{truncated}\""));
    }
    parts.join(" | ")
}

/// Styles one widget fragment with the supplied live theme.
fn color(theme: &Theme, name: &'static str) -> Style {
    let theme = theme.clone();
    Style::new(move |text| theme.fg(name, text))
}

/// The container renders the frame while the host's list handles input.
struct Selector {
    /// Heading, borders, choices and footer in render order.
    container: Container,
    /// Host-owned selection and cancellation handlers.
    list: SelectList,
    /// Frame writer used after each input.
    tui: TUI,
}
impl ComponentView for Selector {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.container.render(width)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.container.invalidate()
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.list.handle_input(data)?;
        self.tui.request_render(false)
    }
}

/// Opens the authored selector; dismissal leaves the current preset unchanged.
pub(super) async fn select(
    ctx: &ExtensionContext,
    presets: &Presets,
    active: Option<&str>,
) -> ExtensionResult<Option<String>> {
    let mut items: Vec<_> = presets
        .iter()
        .map(|(name, preset)| SelectItem {
            value: name.clone(),
            label: if Some(name.as_str()) == active {
                format!("{name} (active)")
            } else {
                name.clone()
            },
            description: Some(description(preset)),
        })
        .collect();
    items.push(SelectItem {
        value: "(none)".into(),
        label: "(none)".into(),
        description: Some("Clear active preset, restore defaults".into()),
    });
    ctx.ui()?
        .custom(
            Rc::new(move |tui, theme, _, done, _| build(tui, &theme, done, &items)),
            None,
        )
        .await
        .map(Option::flatten)
}

/// Builds the retained frame using host widgets and their existing event handlers.
fn build(
    tui: TUI,
    theme: &Theme,
    done: CustomDone<Option<String>>,
    items: &[SelectItem],
) -> ExtensionResult<CustomFactoryOutcome> {
    let container = Container::new()?;
    let accent = color(theme, "accent");
    container.add_child(&(&DynamicBorder::new(Some(&accent))?).into())?;
    container.add_child(
        &(&Text::new(
            &theme.fg("accent", &theme.bold("Select Preset")?)?,
            0,
            0,
            None,
        )?)
            .into(),
    )?;
    let list = SelectList::new(
        items,
        u32::try_from(items.len().min(10)).map_err(|error| error.to_string())?,
        &SelectListTheme {
            selected_prefix: accent.clone(),
            selected_text: accent,
            description: color(theme, "muted"),
            scroll_info: color(theme, "dim"),
            no_match: color(theme, "warning"),
        },
        &SelectListLayoutOptions::default(),
    )?;
    let selected = done.clone();
    list.set_on_select(Some(Rc::new(move |item, _| {
        selected(Some(item.value.clone()))
    })))?;
    list.set_on_cancel(Some(Rc::new(move |_| done(None))))?;
    container.add_child(&(&list).into())?;
    container.add_child(
        &(&Text::new(
            &theme.fg("dim", "↑↓ navigate • enter select • esc cancel")?,
            0,
            0,
            None,
        )?)
            .into(),
    )?;
    container.add_child(&(&DynamicBorder::new(Some(&color(theme, "accent")))?).into())?;

    Ok(CustomFactoryOutcome::Ready(ExtensionComponent {
        component: Component::new(Selector {
            container,
            list,
            tui,
        }),
        dispose: None,
        set_expanded: None,
        width: None,
    }))
}
