//! Console output mounted as native page content and followed through its own scroller.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    browser_fixture_support::{Patch, get, global},
    contributions_support::{
        Clipboard, Recorded, computed, container, document, driver, frames, host, page, rgb,
        select, text, until, view,
    },
};
use maestro_web::{
    ConsoleBlock,
    components::console_block::ConsoleVariant,
    dom::{DomView, MountedContent},
};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement};

/// Console cases recorded from the reference component.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    /// Console operands and expected values.
    console: Console,
}
/// One console case set.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Console {
    /// Initial content and its header.
    mount: Mount,
    /// Replaced content with its semantic color.
    update: Update,
    /// Idle and success copy labels.
    copy: Copy,
    /// German labels with their own content.
    german: German,
}
/// The mounted console's operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Mount {
    /// Literal output.
    content: String,
    /// Localized header label.
    label: String,
    /// Whether the body starts scrolled to its end.
    scrolled_to_end: bool,
}
/// Replaced console operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    /// Literal output containing tag-like text.
    content: String,
    /// Selected semantic color, `default` or `error`.
    variant: String,
}
/// Copy labels.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Copy {
    /// Tooltip and accessible name.
    title: String,
    /// Success feedback text.
    feedback: String,
}
/// German operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct German {
    /// Localized header label.
    label: String,
    /// Tooltip and accessible name.
    title: String,
    /// Literal output.
    content: String,
    /// Success feedback text.
    feedback: String,
}

/// The console's success feedback duration, in milliseconds.
const FEEDBACK_MS: f64 = 1500.0;

/// The recorded console cases.
fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/native_transcript/tools.json")).unwrap()
}
/// The selected semantic color named by `variant`.
fn variant(name: &str) -> ConsoleVariant {
    match name {
        "error" => ConsoleVariant::Error,
        "default" => ConsoleVariant::Default,
        other => panic!("unknown console variant {other}"),
    }
}
/// A console showing `content` in `variant`.
fn console(content: &str, variant: ConsoleVariant) -> ConsoleBlock {
    let mut block = ConsoleBlock::default();
    content.clone_into(&mut block.content);
    block.variant = variant;
    block
}
/// The mounted console's root and its scrolling body.
fn parts(root: &Element) -> (Element, HtmlElement) {
    let body = root.query_selector("pre").unwrap().unwrap();
    let scroll = root
        .query_selector(".console-scroll")
        .unwrap()
        .unwrap()
        .unchecked_into();
    (body, scroll)
}
/// The output height `scroll` hides, which must cover at least one full view, so only a body
/// that follows its end reads as scrolled to it.
fn hidden(scroll: &HtmlElement) -> i32 {
    let hidden = scroll.scroll_height() - scroll.client_height();
    assert!(
        hidden >= scroll.client_height(),
        "the body hides only {hidden} pixels"
    );
    hidden
}
/// The updated output: the mounted lines, then the update's literal text, so the replaced
/// body overflows too.
fn overflowing(cases: &Console) -> String {
    format!("{}\n{}", cases.mount.content, cases.update.content)
}
/// The console's header label.
fn header_label(root: &Element) -> Element {
    root.query_selector(".maestro-console-label")
        .unwrap()
        .unwrap()
}
/// The console's copy control.
fn copy_control(root: &Element) -> Element {
    root.query_selector("copy-button").unwrap().unwrap()
}
/// The copy control's button.
fn copy_button(root: &Element) -> HtmlElement {
    copy_control(root)
        .query_selector("button")
        .unwrap()
        .unwrap()
        .unchecked_into()
}
/// A view whose labels come from the German localization.
fn german(view: &DomView) -> Rc<DomView> {
    crate::contributions_support::localized_view(view, "de")
}
/// Page content placed in one container, removed when dropped.
struct Placed {
    /// Page operands.
    view: Rc<DomView>,
    /// Page operands of the mounted console, which place its messages.
    parent: Element,
    /// The placed root.
    shown: MountedContent,
}
impl Placed {
    /// Show `block` in a new container with `view`.
    fn show(view: Rc<DomView>, block: &ConsoleBlock) -> Self {
        let parent = container();
        let mut shown = MountedContent::default();
        shown.show(block, &parent, &view).unwrap();
        Self {
            view,
            parent,
            shown,
        }
    }
    /// The mounted root.
    fn root(&self) -> Element {
        self.shown.root().cloned().unwrap().unchecked_into()
    }
    /// Update the placed console with `block`.
    fn update(&mut self, block: &ConsoleBlock) {
        self.shown.show(block, &self.parent, &self.view).unwrap();
    }
    /// Update the placed console after replacing its page operands.
    fn replace_view(&mut self, block: &ConsoleBlock, view: Rc<DomView>) {
        self.view = view;
        self.update(block);
    }
    /// Detach the root from the page, keeping it alive for a later reattachment.
    fn detach(&self) -> Element {
        let root = self.root();
        let parent = root.parent_node().unwrap();
        parent.remove_child(&root).unwrap();
        root
    }
}
impl Drop for Placed {
    fn drop(&mut self) {
        if let Some(root) = self.shown.root() {
            root.parent_node().unwrap().remove_child(root).unwrap();
        }
    }
}
/// Timers the page registers while installed, held until the test runs them, so feedback
/// restoration is observed at the control's own timer rather than on a clock.
struct Held {
    /// Each registration's delay and callback, oldest first.
    timers: Rc<RefCell<Vec<(f64, js_sys::Function)>>>,
    /// The replaced page `setTimeout`.
    _patch: Patch,
}
impl Held {
    /// Hold every timer the page registers; each registration schedules a no-op instead, so
    /// clearing its handle cancels nothing else.
    fn install() -> Self {
        let timers: Rc<RefCell<Vec<(f64, js_sys::Function)>>> = Rc::default();
        let registered = Rc::clone(&timers);
        let inert = get(&global("Function"), "prototype");
        let window = web_sys::window().unwrap();
        let patch = Patch::method(&window, "setTimeout", move |original, this, args| {
            registered
                .borrow_mut()
                .push((args.get(1).as_f64().unwrap(), args.get(0).unchecked_into()));
            args.set(0, inert.clone());
            original.apply(&this, &args)
        });
        Self {
            timers,
            _patch: patch,
        }
    }
    /// The delays registered so far, oldest first.
    fn delays(&self) -> Vec<f64> {
        self.timers
            .borrow()
            .iter()
            .map(|(delay, _)| *delay)
            .collect()
    }
    /// Run the newest held timer, as the page would at its delay.
    fn fire(&self) {
        let (_, callback) = self.timers.borrow().last().cloned().unwrap();
        callback.call0(&JsValue::UNDEFINED).unwrap();
    }
}
/// Wait for `element` to render success feedback after this call, so feedback already
/// showing never satisfies the wait.
async fn copied(element: &Element) {
    page::changed(element).await;
    until(element, || !text(element).is_empty()).await;
}
/// Wait for `element` to return to its idle control.
async fn restored(element: &Element) {
    until(element, || text(element).is_empty()).await;
}
/// Collect page nodes no task boundary keeps alive.
async fn collect() {
    let window = web_sys::window().unwrap();
    let collect: js_sys::Function = js_sys::Reflect::get(&window, &"gc".into())
        .unwrap()
        .unchecked_into();
    for _ in 0..4 {
        frames(1).await;
        collect.call0(&window).unwrap();
    }
    frames(1).await;
}

/// The mounted console shows its header, copy title and literal output, scrolled to the end.
fn assert_mounted(root: &Element, mount: &Mount, copy: &Copy) {
    assert_eq!(root.class_name(), "maestro-console");
    assert!(document().query_selector("canvas").unwrap().is_none());
    assert_eq!(text(&header_label(root)), mount.label);
    assert_eq!(
        copy_control(root).get_attribute("title").as_deref(),
        Some(copy.title.as_str())
    );
    let (body, scroll) = parts(root);
    assert_eq!(text(&body), mount.content);
    let hidden_height = hidden(&scroll);
    if mount.scrolled_to_end {
        assert_eq!(scroll.scroll_top(), hidden_height);
    }
}
/// The console shows the literal `content`, keeps its root and scrolls its body to the end.
fn assert_updated(mounted: &mut Placed, root: &Element, content: &str) {
    assert_eq!(
        mounted.root(),
        *root,
        "the updated console replaced its root"
    );
    let (body, scroll) = parts(root);
    assert_eq!(text(&body), content);
    assert!(body.query_selector("b").unwrap().is_none());
    assert_eq!(body.class_name(), "maestro-console-error");
    assert_eq!(
        scroll.scroll_top(),
        hidden(&scroll),
        "the update left the body above its end"
    );
}
/// An activation copies the current output once and follows it to the body's end.
async fn assert_copy_follows(root: &Element, clipboard: &Recorded, content: &str) {
    let control = copy_control(root);
    let (_, scroll) = parts(root);
    scroll.set_scroll_top(0);
    copy_button(root).click();
    copied(&control).await;
    assert_eq!(clipboard.texts(), [content]);
    assert_eq!(
        scroll.scroll_top(),
        hidden(&scroll),
        "the copy feedback left the body above its end"
    );
}
/// An update carrying the same operands keeps the scrolled place, the root and the body.
fn assert_unchanged_update(mounted: &mut Placed, root: &Element, block: &ConsoleBlock) {
    let before = parts(root);
    let (_, scroll) = parts(root);
    scroll.set_scroll_top(0);
    mounted.update(block);
    assert_eq!(parts(root).0, before.0);
    assert_eq!(scroll.scroll_top(), 0);
}
/// A presentation-only update keeps the selection and reaches the replacement localization.
fn assert_localized_update(
    mounted: &mut Placed,
    view: &Rc<DomView>,
    (root, block): (&Element, &ConsoleBlock),
    german_cases: &German,
) {
    page::select_children(&parts(root).0);
    mounted.replace_view(block, german(view));
    assert_eq!(text(&header_label(root)), german_cases.label);
    assert_eq!(
        copy_control(root).get_attribute("title").as_deref(),
        Some(german_cases.title.as_str())
    );
    let (selected, inside) = page::selection(root);
    assert!(!selected.is_empty() && inside, "selection lost on update");
}

/// Literal output stays uninterpreted, changed operands restart scrolling, and a
/// presentation-only update keeps the root, the body and the selection.
#[wasm_bindgen]
pub async fn console_dom_literal_scroll_and_variant(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let cases = fixture().console;
    let clipboard = Recorded::install(Clipboard::Accept);
    let initial = console(&cases.mount.content, ConsoleVariant::Default);
    let mut mounted = Placed::show(Rc::clone(&view), &initial);
    let root = mounted.root();
    frames(1).await;
    assert_mounted(&root, &cases.mount, &cases.copy);
    assert_unchanged_update(&mut mounted, &root, &initial);
    let content = overflowing(&cases);
    let changed = console(&content, variant(&cases.update.variant));
    mounted.update(&changed);
    assert_updated(&mut mounted, &root, &content);
    assert_copy_follows(&root, &clipboard, &content).await;
    let (_, scroll) = parts(&root);
    scroll.set_scroll_top(0);
    mounted.update(&changed);
    assert_eq!(
        scroll.scroll_top(),
        0,
        "an unchanged update reset the scroll"
    );
    assert_localized_update(&mut mounted, &view, (&root, &changed), &cases.german);
    mounted.replace_view(&changed, Rc::clone(&view));
    assert_eq!(text(&header_label(&root)), cases.mount.label);
}

/// Success feedback reads the current text, holds until its restoring timer at the
/// console's own duration and then returns to the idle control.
async fn assert_success_feedback(root: &Element, clipboard: &Recorded, cases: &Fixture) {
    let control = copy_control(root);
    let idle = control.inner_html();
    let timers = Held::install();
    copy_button(root).click();
    copied(&control).await;
    assert_eq!(clipboard.texts(), [cases.console.update.content.as_str()]);
    assert_eq!(text(&control), cases.console.copy.feedback);
    assert_ne!(control.inner_html(), idle);
    assert!(control.query_selector("svg").unwrap().is_some());
    assert_eq!(timers.delays(), [FEEDBACK_MS], "restoring interval");
    timers.fire();
    assert_eq!(
        control.inner_html(),
        idle,
        "the restoring timer left the feedback showing"
    );
}

/// A presented localization reaches both the recorded text and the feedback label.
async fn assert_german_feedback(
    mounted: &mut Placed,
    german_view: Rc<DomView>,
    block: &ConsoleBlock,
    cases: &Console,
) {
    mounted.replace_view(block, german_view);
    let control = copy_control(&mounted.root());
    copy_button(&mounted.root()).click();
    copied(&control).await;
    assert_eq!(text(&control), cases.german.feedback);
}

/// A refused write, once settled, leaves the control in its idle state.
async fn assert_refused_copy(root: &Element, refused: &Recorded, content: &str) {
    let control = copy_control(root);
    copy_button(root).click();
    assert_eq!(refused.texts(), [content]);
    // The refusal settles in microtasks, which all run before the next frame.
    frames(1).await;
    assert!(
        text(&control).is_empty(),
        "a refused write showed success feedback"
    );
    assert!(control.query_selector("span").unwrap().is_none());
}

/// Success feedback reads the current text and locale, restores at its own duration,
/// and a refused copy adds no rendered state.
#[wasm_bindgen]
pub async fn console_dom_copy_feedback_uses_current_text_and_locale(
    import_module: js_sys::Function,
) {
    let view = view();
    let _host = host(&view, &import_module);
    let cases = fixture();
    let clipboard = Recorded::install(Clipboard::Accept);
    let block = console(&cases.console.update.content, ConsoleVariant::Error);
    let mut mounted = Placed::show(Rc::clone(&view), &block);
    let root = mounted.root();
    assert_success_feedback(&root, &clipboard, &cases).await;
    copy_button(&root).click();
    copied(&copy_control(&root)).await;
    assert_eq!(clipboard.texts().len(), 2);
    assert_german_feedback(&mut mounted, german(&view), &block, &cases.console).await;
    assert_eq!(clipboard.texts().len(), 3);
    let refusing = Recorded::install(Clipboard::Refuse);
    let refused = Placed::show(
        german(&view),
        &console(&cases.console.german.content, ConsoleVariant::Default),
    );
    assert_refused_copy(&refused.root(), &refusing, &cases.console.german.content).await;
}

/// An activation copies once, and a reattached instance keeps one subscription while
/// another instance reports its own feedback.
#[wasm_bindgen]
pub async fn console_dom_copy_disconnect_reconnect_releases_work(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let cases = fixture().console;
    let clipboard = Recorded::install(Clipboard::Defer);
    let (left, right) = (
        Placed::show(
            Rc::clone(&view),
            &console(&cases.update.content, ConsoleVariant::Error),
        ),
        Placed::show(
            Rc::clone(&view),
            &console(&cases.german.content, ConsoleVariant::Default),
        ),
    );
    let (left_root, right_root) = (left.root(), right.root());
    let (left_copy, right_copy) = (copy_control(&left_root), copy_control(&right_root));
    copy_button(&left_root).click();
    assert_eq!(clipboard.texts(), [cases.update.content.as_str()]);
    clipboard.settle();
    copied(&left_copy).await;
    assert!(text(&right_copy).is_empty(), "instances shared feedback");
    copy_button(&right_root).click();
    clipboard.settle();
    copied(&right_copy).await;
    assert_eq!(
        clipboard.texts(),
        [cases.update.content.as_str(), cases.german.content.as_str()]
    );
    let detached = left.detach();
    container().append_child(&detached).unwrap();
    copy_button(&left_root).click();
    assert_eq!(
        clipboard.texts().len(),
        3,
        "reattachment installed a second subscription"
    );
    clipboard.settle();
    copied(&left_copy).await;
    drop(right);
    copy_button(&left_root).click();
    assert_eq!(
        clipboard.texts().len(),
        4,
        "a released instance held the survivor"
    );
    clipboard.settle();
    copied(&left_copy).await;
    assert_pending_across_disconnect(&clipboard, &cases).await;
    assert_retired_completions(&clipboard, &cases).await;
}
/// A write still pending across a disconnect and reconnection settles after the reconnected
/// control's own copy showed feedback, and it restarts no restoring interval.
async fn assert_pending_across_disconnect(clipboard: &Recorded, cases: &Console) {
    let timers = Held::install();
    let written = clipboard.texts().len();
    let placed = Placed::show(
        view(),
        &console(&cases.mount.content, ConsoleVariant::Default),
    );
    let root = placed.root();
    let control = copy_control(&root);
    copy_button(&root).click();
    let detached = placed.detach();
    container().append_child(&detached).unwrap();
    copy_button(&root).click();
    assert_eq!(
        clipboard.texts().len(),
        written + 2,
        "the reconnected control did not copy again"
    );
    clipboard.settle();
    copied(&control).await;
    assert_eq!(timers.delays(), [FEEDBACK_MS]);
    // Only the write from before the disconnect is still pending.
    clipboard.settle();
    frames(1).await;
    assert_eq!(
        timers.delays(),
        [FEEDBACK_MS],
        "a write pending across the disconnect restarted the feedback"
    );
    timers.fire();
    assert!(text(&control).is_empty(), "the feedback outlived its timer");
}
/// A completion that settles after its control disconnected, or after it reconnected,
/// is retired with the view that asked for it.
async fn assert_retired_completions(clipboard: &Recorded, cases: &Console) {
    let retiring = Placed::show(
        Rc::clone(&view()),
        &console(&cases.mount.content, ConsoleVariant::Default),
    );
    let root = retiring.root();
    let control = copy_control(&root);
    copy_button(&root).click();
    let detached = retiring.detach();
    clipboard.settle();
    frames(1).await;
    assert!(
        text(&control).is_empty(),
        "a completion settled after disconnect painted feedback"
    );
    container().append_child(&detached).unwrap();
    copy_button(&root).click();
    let again = retiring.detach();
    container().append_child(&again).unwrap();
    clipboard.settle();
    frames(1).await;
    assert!(
        text(&control).is_empty(),
        "a completion settled after reconnection painted feedback"
    );
}

/// A removed console releases its scroller rather than retaining it through the copy callback.
#[wasm_bindgen]
pub async fn console_dom_removal_releases_its_subtree(import_module: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import_module);
    let cases = fixture().console;
    let placed = Placed::show(
        Rc::clone(&view),
        &console(&cases.mount.content, ConsoleVariant::Default),
    );
    let root = placed.root();
    let scroller = root.query_selector(".console-scroll").unwrap().unwrap();
    let copy = copy_control(&root);
    let probes = [
        js_sys::WeakRef::new(&root),
        js_sys::WeakRef::new(&copy),
        js_sys::WeakRef::new(&scroller),
    ];
    drop(root);
    drop(copy);
    drop(scroller);
    drop(placed);
    collect().await;
    let alive = probes.each_ref().map(|probe| probe.deref().is_some());
    assert!(
        !alive.contains(&true),
        "the removed console retained root/copy/scroller: {alive:?}"
    );
}

/// The console's copy control keeps compact geometry, a muted idle role and an accent
/// highlight, measured on the painted control.
async fn assert_compact_control(view: &maestro_web::dom::DomView, root: &Element, cases: &Console) {
    let presentation = view.pack.presentation(view.mode).unwrap();
    let role = |name: &str| rgb(presentation.colors[name]);
    let control = copy_button(root);
    driver("driverPointer", "[0,0,false]").await;
    settle_transitions(&control);
    assert_eq!(computed(&control, "height"), "20px", "compact height");
    assert_eq!(computed(&control, "column-gap"), "4px", "compact gap");
    assert_eq!(computed(&control, "font-size"), "12px", "compact text");
    assert_eq!(computed(&control, "color"), role("muted"), "idle role");
    let rect = control.get_bounding_client_rect();
    let point = format!(
        "[{},{},false]",
        rect.x() + rect.width() / 2.,
        rect.y() + rect.height() / 2.
    );
    driver("driverPointer", &point).await;
    settle_transitions(&control);
    assert_eq!(
        computed(&control, "background-color"),
        role("accent"),
        "hover surface"
    );
    assert_eq!(computed(&control, "color"), role("on-accent"), "hover role");
    driver("driverPointer", "[0,0,false]").await;
    assert_success_gap((&control, cases)).await;
}
/// Complete a copy and measure the success label's rendered separation from its glyph.
async fn assert_success_gap((control, cases): (&HtmlElement, &Console)) {
    let clipboard = Recorded::install(Clipboard::Accept);
    control.click();
    copied(control).await;
    assert_eq!(clipboard.texts(), [cases.mount.content.as_str()]);
    assert_eq!(text(control), cases.copy.feedback, "success label");
    let glyph = select(control.as_ref(), "svg").get_bounding_client_rect();
    let label = select(control.as_ref(), "span").get_bounding_client_rect();
    let gap = label.left() - glyph.right();
    assert!(
        (gap - 4.0).abs() < 0.5,
        "success label separated by {gap} pixels"
    );
    restored(control).await;
}
/// Finish the control's running transitions, so the next reading is its painted result.
fn settle_transitions(element: &Element) {
    page::finish(element);
}

/// The console's copy control keeps the reference's compact, muted and accent treatment.
#[wasm_bindgen]
pub async fn console_dom_control_keeps_compact_muted_and_accent_treatment(
    import_module: js_sys::Function,
) {
    let view = view();
    let _host = host(&view, &import_module);
    let cases = fixture().console;
    let placed = Placed::show(
        Rc::clone(&view),
        &console(&cases.mount.content, ConsoleVariant::Default),
    );
    frames(1).await;
    let root = placed.root();
    assert_compact_control(&view, &root, &cases).await;
}
