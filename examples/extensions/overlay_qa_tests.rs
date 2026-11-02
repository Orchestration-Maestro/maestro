//! Overlay demonstrations covering animation, placement, stacking, overflow, visibility,
//! passive overlays, focus and streaming input.
//!
//! Each `/overlay-*` command shows one demonstration; Escape closes it.
// maestro-component: MAESTRO_OVERLAY_QA_TESTS_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    CommandOptions, Component, ComponentDisposer, ComponentView, CustomDone, CustomFactory,
    CustomFactoryOutcome, CustomUIOptions, ExtensionAPI, ExtensionCommandContext,
    ExtensionComponent, ExtensionFuture, ExtensionResult, NotificationType, OverlayAnchor,
    OverlayHandleCallback, OverlayMargin, OverlayMarginValue, OverlayOptions, OverlayOptionsHandle,
    OverlayOptionsSource, SizeValue, TUI,
};
use std::{
    cell::RefCell,
    future::{Future, poll_fn},
    pin::pin,
    rc::Rc,
    task::Poll,
    time::Duration,
};
#[path = "overlay_qa_tests/dialogs.rs"]
mod dialogs;
#[path = "overlay_qa_tests/live.rs"]
mod live;
#[path = "overlay_qa_tests/paint.rs"]
mod paint;
#[path = "overlay_qa_tests/panels.rs"]
mod panels;
use dialogs::{Cycle, Step};

/// The nine anchors in the order `/overlay-anchors` cycles through them.
const ANCHORS: [(OverlayAnchor, &str); 9] = [
    (OverlayAnchor::TopLeft, "top-left"),
    (OverlayAnchor::TopCenter, "top-center"),
    (OverlayAnchor::TopRight, "top-right"),
    (OverlayAnchor::LeftCenter, "left-center"),
    (OverlayAnchor::Center, "center"),
    (OverlayAnchor::RightCenter, "right-center"),
    (OverlayAnchor::BottomLeft, "bottom-left"),
    (OverlayAnchor::BottomCenter, "bottom-center"),
    (OverlayAnchor::BottomRight, "bottom-right"),
];

/// The frame writer a view received, kept so periodic work can ask it for a frame.
#[derive(Default)]
struct Screen(RefCell<Option<TUI>>);
impl Screen {
    /// Keeps the frame writer the factory received.
    fn set(&self, tui: TUI) {
        self.0.replace(Some(tui));
    }
    /// Requests a frame once the view exists.
    fn request(&self) -> ExtensionResult<()> {
        self.0
            .borrow()
            .as_ref()
            .map_or(Ok(()), |tui| tui.request_render(false))
    }
}

/// Placement with only the fields a demonstration sets.
fn placement(anchor: Option<OverlayAnchor>, width: i64) -> OverlayOptions {
    OverlayOptions {
        anchor,
        width: Some(SizeValue::Cells(width)),
        ..OverlayOptions::default()
    }
}

/// Margins on the named sides only.
fn sides(
    top: Option<i64>,
    right: Option<i64>,
    bottom: Option<i64>,
    left: Option<i64>,
) -> OverlayMarginValue {
    OverlayMarginValue::Sides(OverlayMargin {
        top,
        right,
        bottom,
        left,
    })
}

/// Wraps a view, with the work the host runs when it removes the view.
fn built(
    view: impl ComponentView + 'static,
    dispose: Option<ComponentDisposer>,
) -> CustomFactoryOutcome {
    CustomFactoryOutcome::Ready(ExtensionComponent {
        component: Component::new(view),
        dispose,
        set_expanded: None,
        width: None,
    })
}

/// Shows a custom overlay with shared placement and returns its committed result.
async fn overlay<T: 'static>(
    ctx: &ExtensionCommandContext,
    options: OverlayOptionsHandle,
    on_handle: Option<OverlayHandleCallback>,
    factory: CustomFactory<T>,
) -> ExtensionResult<Option<T>> {
    let options = CustomUIOptions {
        overlay: Some(true),
        overlay_options: Some(OverlayOptionsSource::Static(options)),
        on_handle,
    };
    ctx.ui()?.custom(factory, Some(options)).await
}

/// Polls `helper` while `main` is pending and drops it once `main` completes.
///
/// A helper failure is returned after `main` completes, so the view always settles first.
async fn alongside<T>(
    main: impl Future<Output = ExtensionResult<T>>,
    helper: impl Future<Output = ExtensionResult<()>>,
) -> ExtensionResult<T> {
    let (mut main, mut helper) = (pin!(main), pin!(helper));
    let mut finished = None;
    let value = poll_fn(|cx| {
        let state = main.as_mut().poll(cx);
        if state.is_pending()
            && finished.is_none()
            && let Poll::Ready(result) = helper.as_mut().poll(cx)
        {
            finished = Some(result);
        }
        state
    })
    .await?;
    finished.unwrap_or(Ok(())).map(|()| value)
}

/// Runs every task to completion, or returns the first failure at once.
async fn join_all(mut tasks: Vec<ExtensionFuture<'_, ()>>) -> ExtensionResult<()> {
    poll_fn(|cx| {
        let mut index = 0;
        while index < tasks.len() {
            match tasks[index].as_mut().poll(cx) {
                Poll::Ready(Ok(())) => drop(tasks.swap_remove(index)),
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => index += 1,
            }
        }
        if tasks.is_empty() {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    })
    .await
}

/// Cycles the anchors until one is confirmed or the overlay closes.
async fn anchors(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let mut index = 0;
    loop {
        let (anchor, name) = ANCHORS[index];
        let options = OverlayOptionsHandle::new(placement(Some(anchor), 40))?;
        let view = Cycle::anchor(&api, name);
        match overlay(&ctx, options, None, view).await? {
            Some(Step::Next) => index = (index + 1) % ANCHORS.len(),
            Some(Step::Confirm) => {
                let message = format!("Anchor selected: {name}");
                return ctx.ui()?.notify(&message, Some(NotificationType::Info));
            }
            Some(Step::Close) | None => return Ok(()),
        }
    }
}

/// Margin and offset configurations shown by `/overlay-margins`.
fn margin_configs() -> [(&'static str, OverlayOptions); 5] {
    let mut offset = placement(Some(OverlayAnchor::Center), 35);
    (offset.offset_x, offset.offset_y) = (Some(10), Some(-3));
    [
        (
            "Top left, no margin",
            placement(Some(OverlayAnchor::TopLeft), 35),
        ),
        (
            "Top left, margin 3",
            OverlayOptions {
                margin: Some(OverlayMarginValue::Uniform(3)),
                ..placement(Some(OverlayAnchor::TopLeft), 35)
            },
        ),
        (
            "Top left, margin top 5, left 10",
            OverlayOptions {
                margin: Some(sides(Some(5), None, None, Some(10))),
                ..placement(Some(OverlayAnchor::TopLeft), 35)
            },
        ),
        ("Center, 10 right and 3 up", offset),
        (
            "Bottom right, margin 2",
            OverlayOptions {
                margin: Some(OverlayMarginValue::Uniform(2)),
                ..placement(Some(OverlayAnchor::BottomRight), 35)
            },
        ),
    ]
}

/// Percentage positions shown by `/overlay-percent`, as row and column percentages.
const PERCENTS: [(&str, u8, u8); 5] = [
    ("Row 0% (top)", 0, 50),
    ("Row 50% (middle)", 50, 50),
    ("Row 100% (bottom)", 100, 50),
    ("Column 0% (left)", 50, 0),
    ("Column 100% (right)", 50, 100),
];

/// Cycles titled configurations until the overlay closes.
async fn cycle(
    api: ExtensionAPI,
    ctx: ExtensionCommandContext,
    (title, next_hint): (&'static str, &'static str),
    configs: Vec<(String, OverlayOptions)>,
) -> ExtensionResult<()> {
    let mut index = 0;
    loop {
        let (name, options) = configs[index].clone();
        let view = Cycle::config(&api, title, name, next_hint);
        match overlay(&ctx, OverlayOptionsHandle::new(options)?, None, view).await? {
            Some(Step::Next) => index = (index + 1) % configs.len(),
            _ => return Ok(()),
        }
    }
}

/// Cycles the margin and offset configurations.
async fn margins(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let configs = margin_configs().map(|(name, options)| (name.to_owned(), options));
    let labels = ("Margins", "Space/→ next placement");
    cycle(api, ctx, labels, configs.into()).await
}

/// Cycles the percentage positions.
async fn percent(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let configs = PERCENTS.map(|(name, row, col)| {
        let options = OverlayOptions {
            width: Some(SizeValue::Cells(30)),
            row: Some(SizeValue::Percentage(format!("{row}%"))),
            col: Some(SizeValue::Percentage(format!("{col}%"))),
            ..OverlayOptions::default()
        };
        (name.to_owned(), options)
    });
    cycle(
        api,
        ctx,
        ("Percentages", "Space/→ next position"),
        configs.into(),
    )
    .await
}

/// One stacked layer: its number, its position and its horizontal and vertical offsets.
type Layer = (u64, &'static str, i64, i64);

/// The layers `/overlay-stack` shows, back to front.
const LAYERS: [Layer; 3] = [
    (1, "back, red border", -8, -4),
    (2, "middle, green border", 0, 0),
    (3, "front, blue border", 8, 4),
];

/// Shows three overlapping overlays, each 400 ms after the one below it, and reports the order
/// they closed in.
async fn stack(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let closed = Rc::new(RefCell::new(Vec::new()));
    let [back, above @ ..] = &LAYERS;
    stack_from(api, ctx.clone(), closed.clone(), (*back, above)).await?;
    let message = format!("Close order: {}", closed.borrow().join(", "));
    ctx.ui()?.notify(&message, Some(NotificationType::Info))
}

/// Shows `layer` and, 400 ms after asking for it, the layers `above` it, recording the order
/// they close in.
fn stack_from(
    api: ExtensionAPI,
    ctx: ExtensionCommandContext,
    closed: Rc<RefCell<Vec<String>>>,
    (layer, above): (Layer, &'static [Layer]),
) -> ExtensionFuture<'static, ()> {
    let mut tasks = vec![stack_layer(api.clone(), ctx.clone(), closed.clone(), layer)];
    if let Some((&next, above)) = above.split_first() {
        tasks.push(Box::pin(async move {
            api.native_io()
                .sleep(Duration::from_millis(400), None)
                .await?;
            stack_from(api, ctx, closed, (next, above)).await
        }));
    }
    Box::pin(join_all(tasks))
}

/// Shows one stacked layer and records its name as it closes.
///
/// The name is recorded when `done` closes the layer, not when the command next polls the view,
/// so layers that close between two polls keep the order they closed in.
fn stack_layer(
    api: ExtensionAPI,
    ctx: ExtensionCommandContext,
    closed: Rc<RefCell<Vec<String>>>,
    (number, position, x, y): Layer,
) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let order = ["back", "middle", "front"][usize::try_from(number - 1).unwrap_or(0)];
        let message = format!("Layer {number} of 3 ({order}) shown");
        ctx.ui()?.notify(&message, Some(NotificationType::Info))?;
        let options = OverlayOptions {
            offset_x: Some(x),
            offset_y: Some(y),
            max_height: Some(SizeValue::Cells(15)),
            ..placement(Some(OverlayAnchor::Center), 50)
        };
        let layer = dialogs::layer(&api, number, position);
        let view: CustomFactory<String> = Rc::new(move |tui, theme, keys, done, emitter| {
            let closed = closed.clone();
            let done: CustomDone<String> = Rc::new(move |name: String| {
                done(name.clone())?;
                closed.borrow_mut().push(name);
                Ok(())
            });
            layer(tui, theme, keys, done, emitter)
        });
        overlay(&ctx, OverlayOptionsHandle::new(options)?, None, view).await?;
        Ok(())
    })
}

/// Places a notice against the right edge of the terminal.
async fn edge(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let options = OverlayOptions {
        margin: Some(sides(None, Some(0), None, None)),
        ..placement(Some(OverlayAnchor::RightCenter), 40)
    };
    overlay(
        &ctx,
        OverlayOptionsHandle::new(options)?,
        None,
        dialogs::edge(&api),
    )
    .await?;
    Ok(())
}

/// Shows twenty-one rows in an overlay that keeps ten.
async fn max_height(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let options = OverlayOptions {
        max_height: Some(SizeValue::Cells(10)),
        ..placement(Some(OverlayAnchor::Center), 50)
    };
    overlay(
        &ctx,
        OverlayOptionsHandle::new(options)?,
        None,
        dialogs::max_height(&api),
    )
    .await?;
    Ok(())
}

/// Shows the side panel only while the terminal has at least 100 columns.
async fn sidepanel(api: ExtensionAPI, ctx: ExtensionCommandContext) -> ExtensionResult<()> {
    let options = OverlayOptionsHandle::new(OverlayOptions {
        anchor: Some(OverlayAnchor::RightCenter),
        width: Some(SizeValue::Percentage("25%".into())),
        min_width: Some(30),
        margin: Some(sides(None, Some(1), None, None)),
        ..OverlayOptions::default()
    })?;
    options.set_visible(Some(Rc::new(|columns, _| Ok(columns >= 100))))?;
    overlay(&ctx, options, None, dialogs::sidepanel(&api)).await?;
    Ok(())
}

/// Clears the editor, then shows a demonstration that owns periodic work.
async fn with_editor_cleared(
    ctx: &ExtensionCommandContext,
    options: OverlayOptions,
    view: CustomFactory<()>,
    work: impl Future<Output = ExtensionResult<()>>,
) -> ExtensionResult<()> {
    ctx.ui()?.set_editor_text("")?;
    let shown = overlay(ctx, OverlayOptionsHandle::new(options)?, None, view);
    alongside(shown, work).await?;
    Ok(())
}

/// Runs one registered demonstration.
type Demo = fn(ExtensionAPI, ExtensionCommandContext) -> ExtensionFuture<'static, ()>;

/// The command callback that runs a demonstration function.
macro_rules! demo {
    ($run:path) => {
        |api, ctx| Box::pin($run(api, ctx))
    };
}

/// Every command with its description and demonstration.
const DEMOS: [(&str, &str, Demo); 13] = [
    (
        "overlay-animation",
        "Animate an overlay at about 30 frames a second",
        demo!(live::animation),
    ),
    (
        "overlay-anchors",
        "Show an overlay at each of the nine anchors",
        demo!(anchors),
    ),
    (
        "overlay-margins",
        "Place an overlay with margins and offsets",
        demo!(margins),
    ),
    (
        "overlay-stack",
        "Stack three overlapping overlays",
        demo!(stack),
    ),
    (
        "overlay-overflow",
        "Stream long process output into a narrower overlay",
        demo!(live::overflow),
    ),
    (
        "overlay-edge",
        "Place an overlay against the right edge",
        demo!(edge),
    ),
    (
        "overlay-percent",
        "Place an overlay by row and column percentages",
        demo!(percent),
    ),
    (
        "overlay-maxheight",
        "Cut an overlay at its maximum height",
        demo!(max_height),
    ),
    (
        "overlay-sidepanel",
        "Show a side panel that hides below 100 columns",
        demo!(sidepanel),
    ),
    (
        "overlay-toggle",
        "Hide an overlay for one second with t",
        demo!(live::toggle),
    ),
    (
        "overlay-passive",
        "Type beside a timer that never takes focus",
        demo!(panels::passive),
    ),
    (
        "overlay-focus",
        "Move focus between overlapping non-capturing panels",
        demo!(panels::focus),
    ),
    (
        "overlay-streaming",
        "Type into panels beside a simulated stream",
        demo!(panels::streaming),
    ),
];

/// Registers every overlay demonstration command.
///
/// # Errors
/// Returns the host's registration rejection.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        for (name, description, demo) in DEMOS {
            let owner = api.clone();
            api.register_command(
                name,
                CommandOptions {
                    description: Some(description.into()),
                    get_argument_completions: None,
                    handler: Rc::new(move |_, ctx| demo(owner.clone(), ctx)),
                },
            )?;
        }
        Ok(())
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);
