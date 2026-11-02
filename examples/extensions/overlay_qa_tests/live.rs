//! Views with periodic or external updates: the animation, streamed process output and the
//! visibility toggle.
use super::{
    Screen, alongside, built, join_all, overlay,
    paint::{Paint, Row, inner},
    placement,
};
use maestro_extensions_wasm::{
    CallbackEmitter, ComponentView, CustomDone, CustomFactory, ExtensionAPI,
    ExtensionCommandContext, ExtensionFuture, ExtensionResult, OverlayAnchor, OverlayHandle,
    OverlayHandleCallback, OverlayOptions, OverlayOptionsHandle, SizeValue,
    child_process::{ChildProcess, SpawnOptions},
    streams::Readable,
    truncate_to_width,
    utils::TruncateOptions,
    visible_width,
};
use std::{
    cell::{Cell, RefCell},
    future::poll_fn,
    rc::Rc,
    task::{Poll, Waker},
    time::{Duration, Instant},
};

/// Frame and rate counters advanced by the animation loop.
#[derive(Default)]
struct Animation {
    /// Frames drawn since the view opened.
    frame: Cell<u32>,
    /// Frames counted in the last whole second.
    fps: Cell<u32>,
    /// Redraws after each frame.
    screen: Screen,
}

/// The animated view.
struct AnimationView {
    /// Theme and host helpers.
    paint: Paint,
    /// Counters shared with the loop.
    state: Rc<Animation>,
    /// Closes the overlay.
    done: CustomDone<()>,
}

/// Rounds a non-negative value down to whole cells.
fn floor_cells(value: f64) -> usize {
    format!("{:.0}", value.max(0.0).floor())
        .parse()
        .unwrap_or(0)
}

/// The colour at `hue` degrees with 80% saturation and 50% lightness, as RGB channels.
fn hue_rgb(hue: f64) -> [usize; 3] {
    let (lightness, saturation) = (0.5, 0.8);
    let q = lightness + saturation - lightness * saturation;
    let p = 2.0 * lightness - q;
    let channel = |t: f64| {
        let t = if t < 0.0 {
            t + 1.0
        } else if t > 1.0 {
            t - 1.0
        } else {
            t
        };
        let value = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        floor_cells(value * 255.0 + 0.5)
    };
    let h = hue / 360.0;
    [channel(h + 1.0 / 3.0), channel(h), channel(h - 1.0 / 3.0)]
}

impl ComponentView for AnimationView {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let paint = &self.paint;
        let frame = self.state.frame.get();
        let bar_width = inner(width).saturating_sub(4).max(12);
        let travel = f64::from(u32::try_from(bar_width - 10).unwrap_or(u32::MAX));
        let pos = floor_cells((f64::from(frame) / 10.0).sin().mul_add(travel, travel) / 2.0);
        let bar = format!(
            "{}{}{}",
            " ".repeat(pos),
            paint.fg("accent", "██████████")?,
            " ".repeat((bar_width - 10).saturating_sub(pos))
        );
        let spin = ["◐", "◓", "◑", "◒"][usize::try_from(frame % 4).unwrap_or(0)];
        let [r, g, b] = hue_rgb(f64::from(frame * 3 % 360));
        let block = format!("\x1b[48;2;{r};{g};{b}m{}\x1b[0m", "  ".repeat(10));
        let mut rows = vec![
            Row::Text(paint.fg("accent", " Overlay animation (~30 FPS)")?),
            Row::Text(String::new()),
            Row::Text(format!(
                " Frame: {}",
                paint.fg("accent", &frame.to_string())?
            )),
            Row::Text(format!(
                " FPS: {}",
                paint.fg("success", &self.state.fps.get().to_string())?
            )),
            Row::Text(String::new()),
            Row::Text(format!(" {bar}")),
            Row::Text(format!(" Spinner: {}", paint.fg("warning", spin)?)),
            Row::Text(format!(" Colour: {block}")),
            Row::Text(String::new()),
        ];
        for text in [
            " Each frame comes from one native wait.",
            " The view redraws whole, so a game",
            " view can draw the same way.",
        ] {
            rows.push(Row::Text(paint.dim(text)?));
        }
        rows.push(Row::Text(String::new()));
        rows.push(Row::Text(paint.dim(" Esc closes")?));
        paint.framed("border", rows, width)
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

/// Advances one frame about every 33 ms and recounts the rate each wall-clock second.
async fn animate(api: ExtensionAPI, state: Rc<Animation>) -> ExtensionResult<()> {
    let io = api.native_io();
    let (mut counted, mut frames) = (Instant::now(), 0);
    loop {
        io.sleep(Duration::from_secs(1) / 30, None).await?;
        state.frame.set(state.frame.get() + 1);
        frames += 1;
        let now = Instant::now();
        if now.duration_since(counted) >= Duration::from_secs(1) {
            state.fps.set(frames);
            (counted, frames) = (now, 0);
        }
        state.screen.request()?;
    }
}

/// Shows the animation until Escape closes it; closing stops the loop.
pub async fn animation(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let state = Rc::new(Animation::default());
    let (owner, shared) = (api.clone(), state.clone());
    let view: CustomFactory<()> = Rc::new(move |tui, theme, _, done, _| {
        shared.screen.set(tui);
        let paint = Paint {
            api: owner.clone(),
            theme,
        };
        Ok(built(
            AnimationView {
                paint,
                state: shared.clone(),
                done,
            },
            None,
        ))
    });
    let options = OverlayOptions {
        max_height: Some(SizeValue::Cells(20)),
        ..placement(Some(OverlayAnchor::Center), 50)
    };
    let shown = overlay(&ctx, OverlayOptionsHandle::new(options)?, None, view);
    alongside(shown, animate(api, state)).await?;
    Ok(())
}

/// One hundred long, coloured, hyperlinked lines over thirty seconds.
const SCRIPT: &str = r#"
echo "Streaming 100 long lines over 30 seconds."
echo "Each line carries colour, a hyperlink or a long path."
echo ""
for i in $(seq 1 100); do
	DIR="/Users/example/Documents/development/maestro/crates/maestro-chat/src/interactive_mode"
	FILE="${DIR}/components/very-long-component-name-that-exceeds-width-${i}.rs"
	echo -e "\033]8;;file://${FILE}\007▶ read: ${FILE}\033]8;;\007"
	if [ $((i % 5)) -eq 0 ]; then
		echo -e "  \033[32m✓ Read ${i} files under /Users/example/Documents/development/maestro\033[0m"
	fi
	if [ $((i % 7)) -eq 0 ]; then
		echo -e "  \033[33m⚠ Warning: line ${i} of very-long-component-name-that-exceeds-width.rs needs a look\033[0m"
	fi
	if [ $((i % 11)) -eq 0 ]; then
		echo -e "  \033[31m✗ Error: missing /some/really/long/path/that/definitely/exceeds/the/overlay/width/limit/file-${i}.rs\033[0m"
	fi
	sleep 0.3
done
echo ""
echo -e "\033[32m✓ Done: 100 files in 30 seconds.\033[0m"
echo "Esc closes this view."
"#;

/// Rows shown at once by the output view.
const VISIBLE: usize = 15;

/// One received line, kept as the process wrote it.
enum Line {
    /// Standard output, one line.
    Out(String),
    /// A trimmed chunk of standard error, drawn in the error colour.
    Err(String),
}

/// Output received so far, the scroll position and the running process.
#[derive(Default)]
struct Output {
    /// Lines in arrival order.
    lines: RefCell<Vec<Line>>,
    /// First visible line.
    scroll: Cell<usize>,
    /// Whether the process exited and closed its output.
    finished: Cell<bool>,
    /// The process, once started, so closing can stop it.
    child: RefCell<Option<Rc<dyn ChildProcess>>>,
    /// Redraws after new output.
    screen: Screen,
}
impl Output {
    /// Stops the process, if it is running.
    fn kill(&self) -> ExtensionResult<()> {
        let child = self.child.borrow().clone();
        child.map_or(Ok(()), |child| child.kill(None).map(drop))
    }
    /// The largest scroll position that still fills the view.
    fn bottom(&self) -> usize {
        self.lines.borrow().len().saturating_sub(VISIBLE)
    }
}

/// The scrolling output view.
struct OutputView {
    /// Theme and host helpers.
    paint: Paint,
    /// Output shared with the reader.
    state: Rc<Output>,
    /// Closes the overlay.
    done: CustomDone<()>,
}
impl ComponentView for OutputView {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let (paint, state) = (&self.paint, &self.state);
        let lines = state.lines.borrow();
        let scroll = state.scroll.get();
        let below = lines.len().saturating_sub(VISIBLE + scroll);
        let info = if scroll > 0 || scroll + VISIBLE < lines.len() {
            paint.dim(&format!(" ↑{scroll} | ↓{below}"))?
        } else {
            String::new()
        };
        let mut rows = vec![Row::Text(info)];
        for line in lines.iter().skip(scroll).take(VISIBLE) {
            rows.push(Row::Text(match line {
                Line::Out(text) => format!(" {text}"),
                Line::Err(text) => format!(" {}", paint.fg("error", text)?),
            }));
        }
        rows.resize_with(VISIBLE + 1, || Row::Text(String::new()));
        let status = if state.finished.get() {
            paint.fg("success", "✓ Exited")?
        } else {
            paint.fg("warning", "● Running")?
        };
        rows.push(Row::Text(format!(
            " {status} {}",
            paint.dim("| ↑↓ scroll | Esc close")?
        )));
        let mut drawn = paint.framed("border", rows, width)?;
        let inner = inner(width);
        let title = format!(" Process output ({} lines) ", lines.len());
        let title = truncate_to_width(&paint.api, &title, inner, TruncateOptions::default())?;
        let rest = inner.saturating_sub(visible_width(&paint.api, &title)?);
        drawn[0] = format!(
            "{}{}{}",
            paint.fg("border", "╭")?,
            paint.fg("accent", &title)?,
            paint.fg("border", &format!("{}╮", "─".repeat(rest)))?
        );
        Ok(drawn)
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        let (paint, state) = (&self.paint, &self.state);
        let scroll = state.scroll.get();
        if paint.closes(data)? {
            state.kill()?;
            return (self.done)(());
        } else if paint.is(data, &["up"])? {
            state.scroll.set(scroll.saturating_sub(1));
        } else if paint.is(data, &["down"])? {
            state.scroll.set(state.bottom().min(scroll + 1));
        } else {
            return Ok(());
        }
        state.screen.request()
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Appends each complete nonempty line a pipe delivers, and the unterminated rest at EOF;
/// standard output also scrolls to the newest line.
async fn pump(source: Rc<dyn Readable>, state: &Output, errors: bool) -> ExtensionResult<()> {
    let mut pending = Vec::new();
    loop {
        let read = source.read().await?;
        let eof = read.is_none();
        if eof && pending.is_empty() {
            return Ok(());
        }
        pending.extend(read.unwrap_or_default());
        let end = if eof {
            pending.len()
        } else {
            pending
                .iter()
                .rposition(|&byte| byte == b'\n')
                .map_or(0, |at| at + 1)
        };
        let text = String::from_utf8_lossy(&pending[..end]).into_owned();
        pending.drain(..end);
        let lines = text
            .split('\n')
            .map(|line| if errors { line.trim() } else { line })
            .filter(|line| !line.is_empty());
        state.lines.borrow_mut().extend(lines.map(|line| {
            if errors {
                Line::Err(line.to_owned())
            } else {
                Line::Out(line.to_owned())
            }
        }));
        if !errors {
            state.scroll.set(state.bottom());
        }
        state.screen.request()?;
        if eof {
            return Ok(());
        }
    }
}

/// Runs the script, reading both pipes until the process exits.
async fn stream(api: ExtensionAPI, state: Rc<Output>) -> ExtensionResult<()> {
    let args = vec!["-c".to_owned(), SCRIPT.to_owned()];
    let io = api.native_io();
    let child = io
        .spawn("bash".into(), args, SpawnOptions::default())
        .await?;
    state.child.replace(Some(child.clone()));
    let pipes = [(child.stdout(), false), (child.stderr(), true)];
    let pumps = pipes.into_iter().filter_map(|(pipe, errors)| {
        let pumped: ExtensionFuture<'_, ()> = Box::pin(pump(pipe?, &state, errors));
        Some(pumped)
    });
    join_all(pumps.collect()).await?;
    child.wait().await?;
    state.finished.set(true);
    state.screen.request()
}

/// Shows streamed process output that overflows the overlay's width.
pub async fn overflow(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let state = Rc::new(Output::default());
    let (owner, shared) = (api.clone(), state.clone());
    let view: CustomFactory<()> = Rc::new(move |tui, theme, _, done, _| {
        shared.screen.set(tui);
        let paint = Paint {
            api: owner.clone(),
            theme,
        };
        let disposing = shared.clone();
        let view = OutputView {
            paint,
            state: shared.clone(),
            done,
        };
        Ok(built(view, Some(Rc::new(move |_| disposing.kill()))))
    });
    let options = OverlayOptions {
        max_height: Some(SizeValue::Cells(20)),
        ..placement(Some(OverlayAnchor::Center), 90)
    };
    let shown = overlay(&ctx, OverlayOptionsHandle::new(options)?, None, view);
    alongside(shown, stream(api, state)).await?;
    Ok(())
}

/// The handle `on_handle` received and the pending restore.
#[derive(Default)]
struct Toggle {
    /// The shown overlay's handle.
    handle: RefCell<Option<OverlayHandle>>,
    /// Times the overlay was hidden.
    count: Cell<u32>,
    /// Whether a restore is pending.
    toggling: Cell<bool>,
    /// Wakes the restore loop when a toggle starts.
    waker: RefCell<Option<Waker>>,
    /// Redraws after restoring.
    screen: Screen,
}

/// The toggle view.
struct ToggleView {
    /// Theme and host helpers.
    paint: Paint,
    /// State shared with the restore loop.
    state: Rc<Toggle>,
    /// Closes the overlay.
    done: CustomDone<()>,
}
impl ComponentView for ToggleView {
    fn render(&self, width: u32, _: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        let paint = &self.paint;
        let count = paint.fg("accent", &self.state.count.get().to_string())?;
        let mut rows = vec![
            String::new(),
            paint.fg("accent", " Visibility toggle")?,
            String::new(),
            " t hides this overlay through".into(),
            " the handle on_handle received.".into(),
            String::new(),
            format!(" Times hidden: {count}"),
            String::new(),
        ];
        for text in [
            " t hides it for one second",
            " (set_hidden on the handle)",
            "",
            " A global key binding could",
            " drive the same handle.",
            "",
            " Esc closes",
        ] {
            rows.push(if text.is_empty() {
                String::new()
            } else {
                paint.dim(text)?
            });
        }
        rows.push(String::new());
        paint.boxed(rows, width, "Toggle")
    }
    fn handle_input(&self, data: &str, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        let state = &self.state;
        if self.paint.closes(data)? {
            return (self.done)(());
        }
        let handle = state.handle.borrow().clone();
        if let Some(handle) = handle.filter(|_| !state.toggling.get())
            && self.paint.is(data, &["t"])?
        {
            state.toggling.set(true);
            state.count.set(state.count.get() + 1);
            handle.set_hidden(true)?;
            if let Some(waker) = state.waker.take() {
                waker.wake();
            }
        }
        Ok(())
    }
    fn invalidate(&self, _: CallbackEmitter<'_>) -> ExtensionResult<()> {
        Ok(())
    }
}

/// Shows the overlay again one second after each toggle hides it.
async fn restore(api: ExtensionAPI, state: Rc<Toggle>) -> ExtensionResult<()> {
    let io = api.native_io();
    loop {
        poll_fn(|cx| {
            if state.toggling.get() {
                return Poll::Ready(());
            }
            state.waker.replace(Some(cx.waker().clone()));
            Poll::Pending
        })
        .await;
        io.sleep(Duration::from_secs(1), None).await?;
        let Some(handle) = state.handle.borrow().clone() else {
            return Ok(());
        };
        handle.set_hidden(false)?;
        state.toggling.set(false);
        state.screen.request()?;
    }
}

/// Shows an overlay that `t` hides for one second through the handle `on_handle` received.
pub async fn toggle(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let state = Rc::new(Toggle::default());
    let (owner, shared, received) = (api.clone(), state.clone(), state.clone());
    let view: CustomFactory<()> = Rc::new(move |tui, theme, _, done, _| {
        shared.screen.set(tui);
        let paint = Paint {
            api: owner.clone(),
            theme,
        };
        Ok(built(
            ToggleView {
                paint,
                state: shared.clone(),
                done,
            },
            None,
        ))
    });
    let on_handle: OverlayHandleCallback = Rc::new(move |handle, _| {
        received.handle.replace(Some(handle));
        Ok(())
    });
    let options = OverlayOptionsHandle::new(placement(Some(OverlayAnchor::Center), 50))?;
    let shown = overlay(&ctx, options, Some(on_handle), view);
    let result = alongside(shown, restore(api, state.clone())).await;
    state.handle.take();
    result.map(drop)
}
