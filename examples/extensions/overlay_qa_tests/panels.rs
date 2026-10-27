//! Non-capturing overlays shown from a view: the passive timer, focus cycling and the
//! streaming input panels.
use super::{
    Screen, built,
    paint::{Paint, Row},
    placement, sides, with_editor_cleared,
};
use maestro_extensions_wasm::{
    CallbackEmitter, Component, ComponentView, CustomDone, CustomFactory, ExtensionAPI,
    ExtensionCommandContext, ExtensionResult, Focusable, OverlayAnchor, OverlayHandle,
    OverlayOptions, OverlayOptionsHandle, SizeValue, TUI, utils::local_date_time_string,
};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    rc::Rc,
    time::Duration,
};

/// Border colours of the three panels, in panel order.
const COLORS: [&str; 3] = ["error", "success", "accent"];

/// Typed text kept by a view that accepts input.
#[derive(Default)]
struct Typed(RefCell<String>);
impl Typed {
    /// Applies Backspace or one printable character; other input is ignored.
    fn apply(&self, paint: &Paint, data: &str) -> ExtensionResult<()> {
        let mut chars = data.chars();
        if paint.is(data, &["backspace"])? {
            self.0.borrow_mut().pop();
        } else if chars.next().is_some_and(|char| !char.is_control()) && chars.next().is_none() {
            self.0.borrow_mut().push_str(data);
        }
        Ok(())
    }
    /// The typed text, or a dim placeholder.
    fn display(&self, paint: &Paint) -> ExtensionResult<String> {
        let typed = self.0.borrow();
        if typed.is_empty() {
            paint.dim("(empty)")
        } else {
            Ok(typed.clone())
        }
    }
}

/// The overlay the passive timer shares with its view.
type Slot = Rc<RefCell<Option<OverlayHandle>>>;

/// The focused view that receives typing while the timer stays passive.
struct Passive {
    /// Theme and host helpers.
    paint: Paint,
    /// Focus as the host last set it.
    focused: Cell<bool>,
    /// Text typed so far.
    typed: Typed,
    /// Inputs received.
    inputs: Cell<u32>,
    /// Length and first code unit of the last input.
    last: RefCell<String>,
    /// The timer overlay, removed when the view closes.
    timer: Slot,
    /// Closes the overlay.
    done: CustomDone<()>,
}
impl ComponentView for Passive {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let paint = &self.paint;
        let last = self.last.borrow();
        let rows = vec![
            String::new(),
            paint.hint(&format!(
                "Focused: {}, inputs: {}",
                self.focused.get(),
                self.inputs.get()
            ))?,
            paint.hint(&format!(
                "Last input: {}",
                if last.is_empty() { "none" } else { &last }
            ))?,
            String::new(),
            format!(" > {}", self.typed.display(paint)?),
            String::new(),
            paint.dim(" Typing reaches this view.")?,
            paint.dim(" Esc closes both overlays.")?,
            String::new(),
        ];
        paint.boxed(rows, width, "Typing")
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        let first = data.encode_utf16().next().unwrap_or(0);
        self.inputs.set(self.inputs.get() + 1);
        self.last.replace(format!(
            "length {}, code {first}",
            data.encode_utf16().count()
        ));
        if self.paint.closes(data)? {
            hide_timer(&self.timer)?;
            return (self.done)(());
        }
        self.typed.apply(&self.paint, data)
    }
    fn focusable(&self) -> Option<&dyn Focusable> {
        Some(self)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}
impl Focusable for Passive {
    fn focused(&self, _: CallbackEmitter<'_>) -> ExtensionResult<bool> {
        Ok(self.focused.get())
    }
    fn set_focused(&self, focused: bool, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.focused.set(focused);
        Ok(())
    }
}

/// Removes the timer overlay once.
fn hide_timer(timer: &Slot) -> ExtensionResult<()> {
    let handle = timer.take();
    handle.map_or(Ok(()), |handle| handle.hide())
}

/// Elapsed seconds, shown without taking focus.
struct Timer {
    /// Theme and host helpers.
    paint: Paint,
    /// Seconds counted by the tick loop.
    seconds: Rc<Cell<u64>>,
}
impl ComponentView for Timer {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let seconds = self.seconds.get();
        let time = format!("{:02}:{:02}", seconds / 60, seconds % 60);
        let rows = vec![
            format!(" {}", self.paint.fg("accent", &time)?),
            self.paint.dim(" non_capturing: true")?,
        ];
        self.paint.boxed(rows, width, "Timer")
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Counts one second at a time and redraws.
async fn tick(
    api: ExtensionAPI,
    seconds: Rc<Cell<u64>>,
    screen: Rc<Screen>,
) -> ExtensionResult<()> {
    let io = api.native_io();
    loop {
        io.sleep(Duration::from_secs(1), None).await?;
        seconds.set(seconds.get() + 1);
        screen.request()?;
    }
}

/// Shows a typing view with a passive timer in the top-right corner.
pub async fn passive(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let (seconds, screen) = (Rc::new(Cell::new(0)), Rc::new(Screen::default()));
    let (owner, counted, shared) = (api.clone(), seconds.clone(), screen.clone());
    let view: CustomFactory<()> = Rc::new(move |tui, theme, _, done, _| {
        shared.set(tui.clone());
        let paint = Paint {
            api: owner.clone(),
            theme,
        };
        let timer = Component::new(Timer {
            paint: paint.clone(),
            seconds: counted.clone(),
        });
        let options = OverlayOptionsHandle::new(OverlayOptions {
            non_capturing: Some(true),
            margin: Some(sides(Some(1), Some(2), None, None)),
            ..placement(Some(OverlayAnchor::TopRight), 22)
        })?;
        let slot = Rc::new(RefCell::new(Some(
            tui.show_overlay(&timer, Some(&options))?,
        )));
        let disposing = slot.clone();
        let view = Passive {
            paint,
            focused: Cell::new(false),
            typed: Typed::default(),
            inputs: Cell::new(0),
            last: RefCell::default(),
            timer: slot,
            done,
        };
        Ok(built(view, Some(Rc::new(move |_| hide_timer(&disposing)))))
    });
    let options = placement(Some(OverlayAnchor::Center), 48);
    with_editor_cleared(&ctx, options, view, tick(api, seconds, screen)).await
}

/// Three non-capturing panels and the one holding focus, if any.
struct Ring {
    /// Panel names in panel order.
    labels: [&'static str; 3],
    /// The panels' handles, removed when the demonstration closes.
    handles: RefCell<Vec<OverlayHandle>>,
    /// The focused panel; `None` leaves focus with the controller.
    focus: Cell<Option<usize>>,
    /// Redraws after focus moves.
    tui: TUI,
    /// Closes the controller.
    done: CustomDone<()>,
}
impl Ring {
    /// Moves focus to the next panel, then back to the controller after the last.
    fn cycle(&self) -> ExtensionResult<()> {
        let handles = self.handles.borrow().clone();
        if let Some(handle) = self.focus.get().and_then(|index| handles.get(index)) {
            handle.unfocus()?;
        }
        let next = self.focus.get().map_or(0, |index| index + 1);
        self.focus.set((next < handles.len()).then_some(next));
        if let Some(handle) = self.focus.get().and_then(|index| handles.get(index)) {
            handle.focus()?;
        }
        self.tui.request_render(false)
    }
    /// Removes every panel.
    fn hide(&self) -> ExtensionResult<()> {
        for handle in self.handles.take() {
            handle.hide()?;
        }
        Ok(())
    }
    /// Removes every panel and closes the controller.
    fn close(&self) -> ExtensionResult<()> {
        self.hide()?;
        (self.done)(())
    }
    /// The focused panel's name.
    fn focused(&self) -> Option<&'static str> {
        self.focus.get().map(|index| self.labels[index])
    }
    /// Handles the controller's keys: Tab cycles, Escape closes.
    fn input(&self, paint: &Paint, data: &str) -> ExtensionResult<()> {
        if paint.closes(data)? {
            self.close()
        } else if paint.is(data, &["tab"])? {
            self.cycle()
        } else {
            Ok(())
        }
    }
}

/// One non-capturing panel; with typing, it also edits its own text.
struct Panel {
    /// Theme and host helpers.
    paint: Paint,
    /// Position in the ring.
    index: usize,
    /// The panels and their focus.
    ring: Rc<Ring>,
    /// This panel's handle, set once it is shown.
    handle: Slot,
    /// Typed text, for panels that accept input.
    typed: Option<Typed>,
}
impl ComponentView for Panel {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let paint = &self.paint;
        let handle = self.handle.borrow().clone();
        let focused = handle.map_or(Ok(false), |handle| handle.is_focused())?;
        let label = paint.fg("accent", self.ring.labels[self.index])?;
        let mut rows = vec![Row::Text(format!(" {label}")), Row::Text(String::new())];
        if focused {
            rows.push(Row::Text(paint.fg("success", " ● Focused")?));
            rows.push(Row::Text(paint.dim(" (gets input)")?));
        } else {
            rows.push(Row::Text(paint.dim(" ○ Not focused")?));
            rows.push(Row::Text(match self.typed {
                Some(_) => String::new(),
                None => paint.dim(" (passive)")?,
            }));
        }
        if let Some(typed) = &self.typed {
            rows.push(Row::Text(format!(" > {}", typed.display(paint)?)));
        }
        rows.push(Row::Text(String::new()));
        if self.typed.is_some() {
            rows.push(Row::Text(paint.dim(" Tab | Esc")?));
        }
        paint.framed(COLORS[self.index], rows, width)
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        if self.paint.is(data, &["tab"])? || self.paint.closes(data)? {
            self.ring.input(&self.paint, data)
        } else {
            self.typed
                .as_ref()
                .map_or(Ok(()), |typed| typed.apply(&self.paint, data))
        }
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Shows the three panels at `position(index)` and returns the ring that controls them.
fn ring(
    (tui, paint, done): (TUI, &Paint, CustomDone<()>),
    labels: [&'static str; 3],
    typing: bool,
    position: fn(i64) -> (i64, i64, i64),
) -> ExtensionResult<Rc<Ring>> {
    let ring = Rc::new(Ring {
        labels,
        handles: RefCell::default(),
        focus: Cell::new(None),
        tui: tui.clone(),
        done,
    });
    for (index, place) in [0, 1, 2].into_iter().enumerate() {
        let handle = Slot::default();
        let panel = Component::new(Panel {
            paint: paint.clone(),
            index,
            ring: ring.clone(),
            handle: handle.clone(),
            typed: typing.then(Typed::default),
        });
        let (row, col, width) = position(place);
        let options = OverlayOptionsHandle::new(OverlayOptions {
            non_capturing: Some(true),
            row: Some(SizeValue::Cells(row)),
            col: Some(SizeValue::Cells(col)),
            width: Some(SizeValue::Cells(width)),
            ..OverlayOptions::default()
        })?;
        let shown = tui.show_overlay(&panel, Some(&options))?;
        handle.replace(Some(shown.clone()));
        ring.handles.borrow_mut().push(shown);
    }
    Ok(ring)
}

/// The focus demonstration's controller.
struct FocusDemo {
    /// Theme and host helpers.
    paint: Paint,
    /// The overlapping panels.
    ring: Rc<Ring>,
}
impl ComponentView for FocusDemo {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let paint = &self.paint;
        let focused = paint.fg("accent", self.ring.focused().unwrap_or("Controller"))?;
        let rows = vec![
            String::new(),
            format!(" Focus: {focused}"),
            String::new(),
            " The three panels above are".into(),
            format!(" all {}. Tab moves", paint.fg("accent", "non_capturing")?),
            " focus() from one to the next.".into(),
            String::new(),
            " The focused panel draws on top,".into(),
            " above the others.".into(),
            String::new(),
            paint.dim(" Tab next | Esc close")?,
            String::new(),
        ];
        paint.boxed(rows, width, "Focus")
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.ring.input(&self.paint, data)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Shows three overlapping panels whose focus Tab cycles.
pub async fn focus(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let view: CustomFactory<()> = Rc::new(move |tui, theme, _, done, _| {
        let paint = Paint {
            api: api.clone(),
            theme,
        };
        let labels = ["First", "Second", "Third"];
        let ring = ring((tui, &paint, done), labels, false, |i| (2, 5 + i * 6, 28))?;
        let disposing = ring.clone();
        Ok(built(
            FocusDemo { paint, ring },
            Some(Rc::new(move |_| disposing.hide())),
        ))
    });
    let options = OverlayOptions {
        margin: Some(sides(None, None, Some(1), None)),
        ..placement(Some(OverlayAnchor::BottomCenter), 55)
    };
    with_editor_cleared(&ctx, options, view, std::future::pending()).await
}

/// Lines kept by the simulated stream.
const KEPT: usize = 8;

/// The newest simulated lines and how many were produced.
#[derive(Default)]
struct Feed {
    /// At most eight lines, oldest first.
    lines: RefCell<VecDeque<String>>,
    /// Lines produced so far.
    count: Cell<u64>,
    /// Redraws after each line.
    screen: Screen,
}

/// The streaming demonstration's controller.
struct StreamingDemo {
    /// Theme and host helpers.
    paint: Paint,
    /// The input panels.
    ring: Rc<Ring>,
    /// Lines shared with the stream.
    feed: Rc<Feed>,
}
impl ComponentView for StreamingDemo {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let paint = &self.paint;
        let focused = match self.ring.focused() {
            Some(label) => label.to_owned(),
            None => paint.fg("success", "Controller (this view)")?,
        };
        let rule =
            paint.dim(&" ─".repeat(usize::try_from(width.saturating_sub(2) / 2).unwrap_or(0)))?;
        let mut rows = vec![
            String::new(),
            format!(" Focus: {}", paint.fg("accent", &focused)?),
            String::new(),
            " Simulated stream:".into(),
            rule.clone(),
        ];
        for line in self.feed.lines.borrow().iter() {
            rows.push(paint.hint(line)?);
        }
        rows.resize(rows.len().max(12), String::new());
        rows.extend([
            rule,
            String::new(),
            format!(
                " Three {} input panels on the left.",
                paint.fg("accent", "non_capturing")?
            ),
            " Tab: controller → A → B → C → controller".into(),
            " Each panel keeps its own typing.".into(),
            String::new(),
            paint.dim(" Tab next | Esc close all")?,
            String::new(),
        ]);
        paint.boxed(rows, width, "Stream and input")
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.ring.input(&self.paint, data)
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Adds one timestamped line every 500 ms, keeping the newest eight.
async fn feed(api: ExtensionAPI, feed: Rc<Feed>) -> ExtensionResult<()> {
    let io = api.native_io();
    loop {
        io.sleep(Duration::from_millis(500), None).await?;
        feed.count.set(feed.count.get() + 1);
        let now = local_date_time_string(&api)?;
        let time = now.split_once(", ").map_or(now.as_str(), |(_, time)| time);
        let mut lines = feed.lines.borrow_mut();
        lines.push_back(format!("[{time}] Line {}", feed.count.get()));
        if lines.len() > KEPT {
            lines.pop_front();
        }
        drop(lines);
        feed.screen.request()?;
    }
}

/// Shows three input panels beside a simulated stream; Tab moves input between them.
pub async fn streaming(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let lines = Rc::new(Feed::default());
    let (owner, shared) = (api.clone(), lines.clone());
    let view: CustomFactory<()> = Rc::new(move |tui, theme, _, done, _| {
        shared.screen.set(tui.clone());
        let paint = Paint {
            api: owner.clone(),
            theme,
        };
        let labels = ["Input A", "Input B", "Input C"];
        let ring = ring((tui, &paint, done), labels, true, |i| (1 + i * 9, 2, 35))?;
        let disposing = ring.clone();
        let view = StreamingDemo {
            paint,
            ring,
            feed: shared.clone(),
        };
        Ok(built(view, Some(Rc::new(move |_| disposing.hide()))))
    });
    let options = OverlayOptions {
        margin: Some(sides(None, None, Some(1), None)),
        ..placement(Some(OverlayAnchor::BottomCenter), 60)
    };
    with_editor_cleared(&ctx, options, view, feed(api, lines)).await
}
