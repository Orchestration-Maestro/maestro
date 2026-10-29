//! Views that only react to keys: placement cycles, stacked layers, the edge and maximum-height
//! notices and the side panel.
use super::{
    built,
    paint::{Paint, Row},
};
use maestro_extensions_wasm::{
    CallbackEmitter, ComponentView, CustomDone, CustomFactory, ExtensionAPI, ExtensionResult, TUI,
};
use std::{cell::Cell, rc::Rc};

/// What a placement cycle asks its command to do next.
pub enum Step {
    /// Show the next placement.
    Next,
    /// Report the current anchor and stop.
    Confirm,
    /// Stop without reporting.
    Close,
}

/// A placement view that names the current placement and moves to the next one.
pub struct Cycle {
    /// Theme and host helpers.
    paint: Paint,
    /// Title in the top border.
    title: &'static str,
    /// Text before the accented placement name.
    label: &'static str,
    /// The current placement's name.
    name: String,
    /// Dim key hints below the name.
    hints: Vec<&'static str>,
    /// Whether Enter confirms the placement.
    confirms: bool,
    /// Completes the overlay with the next step.
    done: CustomDone<Step>,
}
impl Cycle {
    /// The anchor view: Enter confirms, Escape cancels.
    pub fn anchor(api: &ExtensionAPI, name: &'static str) -> CustomFactory<Step> {
        let api = api.clone();
        Rc::new(move |_, theme, _, done, _| {
            Ok(built(
                Self {
                    paint: Paint {
                        api: api.clone(),
                        theme,
                    },
                    title: "Anchors",
                    label: "Anchor: ",
                    name: name.into(),
                    hints: vec!["Space/→ next anchor", "Enter select", "Esc cancel"],
                    confirms: true,
                    done,
                },
                None,
            ))
        })
    }
    /// A configuration view: the next key cycles, Escape closes.
    pub fn config(
        api: &ExtensionAPI,
        title: &'static str,
        name: String,
        next: &'static str,
    ) -> CustomFactory<Step> {
        let api = api.clone();
        Rc::new(move |_, theme, _, done, _| {
            Ok(built(
                Self {
                    paint: Paint {
                        api: api.clone(),
                        theme,
                    },
                    title,
                    label: "",
                    name: name.clone(),
                    hints: vec![next, "Esc close"],
                    confirms: false,
                    done,
                },
                None,
            ))
        })
    }
}
impl ComponentView for Cycle {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let paint = &self.paint;
        let name = paint.fg("accent", &self.name)?;
        let mut rows = vec![
            String::new(),
            format!(" {}{name}", self.label),
            String::new(),
        ];
        for hint in &self.hints {
            rows.push(paint.hint(hint)?);
        }
        rows.push(String::new());
        paint.boxed(rows, width, self.title)
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if self.paint.closes(data)? {
            (self.done)(Step::Close)
        } else if self.confirms && self.paint.is(data, &["return"])? {
            (self.done)(Step::Confirm)
        } else if self.paint.is(data, &["space", "right"])? {
            (self.done)(Step::Next)
        } else {
            Ok(())
        }
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// One of the three stacked overlays, each with its own border colour.
struct Layer {
    /// Theme and host helpers.
    paint: Paint,
    /// Position in the stack, from 1 at the back.
    number: u64,
    /// Describes the layer and its colour.
    position: &'static str,
    /// Completes the overlay with its name.
    done: CustomDone<String>,
}
/// Builds a stacked layer that closes with Enter or Escape.
pub fn layer(api: &ExtensionAPI, number: u64, position: &'static str) -> CustomFactory<String> {
    let api = api.clone();
    Rc::new(move |_, theme, _, done, _| {
        Ok(built(
            Layer {
                paint: Paint {
                    api: api.clone(),
                    theme,
                },
                number,
                position,
                done,
            },
            None,
        ))
    })
}
impl ComponentView for Layer {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let paint = &self.paint;
        let color =
            ["error", "success", "accent"][usize::try_from((self.number - 1) % 3).unwrap_or(0)];
        let shade = "░".repeat(super::paint::inner(width).saturating_sub(2));
        let mut rows = vec![
            Row::Text(format!(
                " Layer {}",
                paint.fg("accent", &format!("#{}", self.number))?
            )),
            Row::Text(format!(" Depth: {}", paint.fg(color, self.position)?)),
            Row::Text(String::new()),
        ];
        rows.extend((0..5).map(|_| Row::Text(format!(" {shade} "))));
        rows.push(Row::Text(String::new()));
        rows.push(Row::Text(paint.dim(" Enter or Esc closes this layer")?));
        paint.framed(color, rows, width)
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if self.paint.closes(data)? || self.paint.is(data, &["return"])? {
            (self.done)(format!("layer {}", self.number))
        } else {
            Ok(())
        }
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// A titled notice whose rows are fixed at construction, closed by Escape.
struct Notice {
    /// Theme and host helpers.
    paint: Paint,
    /// Title in the top border.
    title: &'static str,
    /// Rows inside the border.
    rows: Vec<String>,
    /// Closes the overlay.
    done: CustomDone<()>,
}
impl ComponentView for Notice {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        self.paint.boxed(self.rows.clone(), width, self.title)
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if self.paint.closes(data)? {
            (self.done)(())
        } else {
            Ok(())
        }
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Builds a notice from rows drawn with the view's theme.
fn notice(
    api: &ExtensionAPI,
    title: &'static str,
    rows: fn(&Paint) -> ExtensionResult<Vec<String>>,
) -> CustomFactory<()> {
    let api = api.clone();
    Rc::new(move |_, theme, _, done, _| {
        let paint = Paint {
            api: api.clone(),
            theme,
        };
        Ok(built(
            Notice {
                rows: rows(&paint)?,
                paint,
                title,
                done,
            },
            None,
        ))
    })
}

/// The notice placed against the right edge of the terminal.
pub fn edge(api: &ExtensionAPI) -> CustomFactory<()> {
    notice(api, "Right edge", |paint| {
        Ok(vec![
            String::new(),
            " This overlay touches the".into(),
            " terminal's right edge.".into(),
            String::new(),
            paint.hint("Its right border sits")?,
            paint.hint("in the last column.")?,
            String::new(),
            paint.hint("Esc closes")?,
            String::new(),
        ])
    })
}

/// Twenty-one rendered rows, of which a maximum height of ten keeps the first ten.
pub fn max_height(api: &ExtensionAPI) -> CustomFactory<()> {
    notice(api, "Maximum height", |paint| {
        let mut rows = vec![
            paint.fg("warning", " ⚠ 21 rows drawn, max_height 10")?,
            paint.dim(" Rows 11-21 and the bottom border are cut")?,
            String::new(),
        ];
        rows.extend((1..=14).map(|line| format!(" Row {line} of 14")));
        rows.push(String::new());
        rows.push(paint.dim(" Esc closes")?);
        Ok(rows)
    })
}

/// Menu entries of the side panel.
const ITEMS: [&str; 5] = ["Sessions", "Models", "Tools", "Settings", "Help"];

/// A menu whose overlay hides itself in narrow terminals.
struct Sidepanel {
    /// Theme and host helpers.
    paint: Paint,
    /// Redraws after a selection change.
    tui: TUI,
    /// The highlighted entry.
    selected: Cell<usize>,
    /// Closes the overlay.
    done: CustomDone<()>,
}
/// Builds the side panel menu.
pub fn sidepanel(api: &ExtensionAPI) -> CustomFactory<()> {
    let api = api.clone();
    Rc::new(move |tui, theme, _, done, _| {
        Ok(built(
            Sidepanel {
                paint: Paint {
                    api: api.clone(),
                    theme,
                },
                tui,
                selected: Cell::new(0),
                done,
            },
            None,
        ))
    })
}
impl ComponentView for Sidepanel {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let paint = &self.paint;
        let mut rows = vec![Row::Text(paint.fg("accent", " Side panel")?), Row::Divider];
        for (index, item) in ITEMS.iter().enumerate() {
            rows.push(Row::Text(if index == self.selected.get() {
                format!("{}{}", paint.fg("accent", "→ ")?, paint.fg("accent", item)?)
            } else {
                format!("  {item}")
            }));
        }
        rows.push(Row::Divider);
        for (color, text) in [
            ("warning", " ⚠ Below 100 columns"),
            ("warning", "   this panel hides"),
            ("dim", " set_visible: width >= 100"),
            ("dim", " ↑↓ select | Esc close"),
        ] {
            rows.push(Row::Text(paint.fg(color, text)?));
        }
        paint.framed("border", rows, width)
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        let selected = self.selected.get();
        if self.paint.closes(data)? {
            return (self.done)(());
        } else if self.paint.is(data, &["up"])? {
            self.selected.set(selected.saturating_sub(1));
        } else if self.paint.is(data, &["down"])? {
            self.selected.set((selected + 1).min(ITEMS.len() - 1));
        } else if !self.paint.is(data, &["return"])? {
            return Ok(());
        }
        self.tui.request_render(false)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}
