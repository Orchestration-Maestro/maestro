//! Native download and preview/source controls in a real document.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{body, computed, driver, host, rgb, text, view};
use crate::session_download_browser::Fixture;
use js_sys::Uint8Array;
use maestro_theme::BrandMode;
use maestro_web::{
    components::{
        button::{ButtonSize, ButtonVariant},
        download_button::dom::{DownloadButtonProps, DownloadContent, download_button},
        preview_code_toggle::{PreviewCodeToggle, ViewMode},
    },
    dom::{DomContent, DomRoot, DomView},
    icons::IconSize,
};
use std::{
    array,
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, closure::Closure, prelude::wasm_bindgen};
use web_sys::{CustomEvent, Element, Event, HtmlButtonElement, HtmlElement, Node};

/// Class marking the selected preview/source button.
const SELECTED: &str = "maestro-toggle-selected";
/// Every admitted text payload with the exact bytes the reference saved.
const TEXT_PAYLOADS: &[(&str, &str, bool, &[u8])] = &[
    ("empty", "", false, &[]),
    ("plain", "Hello world", false, b"Hello world"),
    ("one", "Y", false, b"Y"),
    ("two", "YQ", false, b"YQ"),
    ("three", "YWI", false, b"YWI"),
    ("auto_unpadded", "YWJj", false, &[97, 98, 99]),
    ("auto_one_padding", "YWI=", false, &[97, 98]),
    ("auto_two_padding", "YQ==", false, &[97]),
    ("auto_word", "test", false, &[181, 235, 45]),
    ("trailing_bits", "YR==", false, &[97]),
    ("invalid_padding", "AA=A", false, b"AA=A"),
    ("invalid_length_four", "A===", false, b"A==="),
    ("over_padding", "====", false, b"===="),
    ("padded_nonmultiple", "YQ=", false, b"YQ="),
    ("nonalphabet", "a-b_", false, b"a-b_"),
    ("forced_unpadded_two", "YQ", true, &[97]),
    ("forced_unpadded_three", "YWI", true, &[97, 98]),
    ("forced_empty", "", true, &[]),
    ("forced_ascii_whitespace", " \tY\nQ\u{c}==\r ", true, &[97]),
    ("unforced_ascii_whitespace", " YQ== ", false, b" YQ== "),
    ("forced_invalid", "not base64!", true, b"not base64!"),
    (
        "forced_invalid_nel",
        "YQ==\u{85}",
        true,
        &[89, 81, 61, 61, 194, 133],
    ),
    (
        "forced_invalid_bom",
        "\u{feff}YQ==",
        true,
        &[239, 187, 191, 89, 81, 61, 61],
    ),
    (
        "data_url_literal",
        "data:text/plain;base64,YQ==",
        false,
        b"data:text/plain;base64,YQ==",
    ),
    (
        "forced_data_url_literal",
        "data:text/plain;base64,YQ==",
        true,
        b"data:text/plain;base64,YQ==",
    ),
    (
        "unicode_crlf",
        "雪😀\r\n<&>\u{0}",
        false,
        &[233, 155, 170, 240, 159, 152, 128, 13, 10, 60, 38, 62, 0],
    ),
];
/// Every admitted MIME spelling with the type the browser's Blob reports.
const MIME_TYPES: &[(&str, &str)] = &[
    ("text/plain", "text/plain"),
    ("", ""),
    ("TEXT/PLAIN;Charset=UTF-8", "text/plain;charset=utf-8"),
    ("text/雪", ""),
];

/// Mount one download control in the page and return it with the root that owns its handler.
fn mounted(view: &DomView, props: DownloadButtonProps) -> (DomRoot, HtmlElement) {
    let root = download_button(view, props).unwrap();
    let control = root.node.clone().unchecked_into::<HtmlElement>();
    body().append_child(&root.node).unwrap();
    (root, control)
}
/// The control's single glyph.
fn glyph(control: &Element) -> Element {
    control.query_selector("svg").unwrap().unwrap()
}
/// The rendered extent of `element`.
fn extent(element: &Element) -> (f64, f64) {
    let rect = element.get_bounding_client_rect();
    (rect.width(), rect.height())
}
/// A control with the supplied presentation operands.
fn shaped(
    size: ButtonSize,
    variant: ButtonVariant,
    icon: IconSize,
    title: Option<String>,
    show_text: bool,
) -> DownloadButtonProps {
    DownloadButtonProps {
        size,
        variant,
        icon_size: icon,
        title,
        show_text,
        ..DownloadButtonProps::new(DownloadContent::Text("payload".into()), "file.bin".into())
    }
}
/// What one control paints for its caller's variant.
fn paint(control: &Element) -> [String; 3] {
    ["background-color", "color", "border-top-color"].map(|property| computed(control, property))
}
/// The control's defaults reach a native button rather than a canvas surface.
fn assert_default_control(view: &DomView) {
    let (owner, control) = mounted(
        view,
        DownloadButtonProps::new(DownloadContent::Text("payload".into()), "file.bin".into()),
    );
    assert_eq!(control.tag_name(), "BUTTON");
    assert_eq!(control.get_attribute("type").as_deref(), Some("button"));
    assert!(control.query_selector("canvas").unwrap().is_none());
    assert_eq!(control.title(), "Download");
    assert_eq!(text(&control), "");
    assert_eq!(
        computed(&glyph(&control), "width"),
        "16px",
        "the default glyph keeps sixteen pixels"
    );
    drop(owner);
}
/// Each literal name reaches the temporary anchor unchanged.
async fn assert_filenames(view: &DomView, fixture: &Fixture) {
    let cases = [
        ("file.bin", "file.bin"),
        ("", ""),
        ("<b>&雪../name.txt", "<b>&雪../name.txt"),
    ];
    for (filename, expected) in cases {
        let (owner, control) = mounted(
            view,
            DownloadButtonProps::new(DownloadContent::Text("plain!".into()), filename.into()),
        );
        fixture
            .saved(&control, b"plain!", "text/plain", expected)
            .await;
        drop(owner);
    }
}
/// The tooltip and the visible label stay independent and localized.
fn assert_titles(view: &DomView) {
    let cases = [
        (None, false, "Download", ""),
        (None, true, "Download", "Download"),
        (Some(String::new()), true, "", "Download"),
        (Some("Exact title <&>".into()), false, "Exact title <&>", ""),
        (Some("Custom".into()), true, "Custom", "Download"),
    ];
    for (title, show_text, expected_title, expected_text) in cases {
        let props = shaped(
            ButtonSize::Sm,
            ButtonVariant::Ghost,
            IconSize::Sm,
            title,
            show_text,
        );
        let (owner, control) = mounted(view, props);
        assert_eq!(control.title(), expected_title);
        assert_eq!(text(&control), expected_text);
        drop(owner);
    }
    view.i18n.set_language("de").unwrap();
    let props = shaped(
        ButtonSize::Sm,
        ButtonVariant::Ghost,
        IconSize::Sm,
        None,
        true,
    );
    let (owner, control) = mounted(view, props);
    assert_eq!(control.title(), "Herunterladen");
    assert_eq!(text(&control), "Herunterladen");
    drop(owner);
    view.i18n.set_language("en").unwrap();
}
/// Every button size reaches the native box.
fn assert_sizes(view: &DomView) {
    let cases = [
        (ButtonSize::Sm, (36., 32.)),
        (ButtonSize::Md, (36., 36.)),
        (ButtonSize::Lg, (36., 40.)),
        (ButtonSize::Icon, (32., 32.)),
    ];
    for (size, expected) in cases {
        let props = shaped(size, ButtonVariant::Ghost, IconSize::Sm, None, false);
        let (owner, control) = mounted(view, props);
        let (width, height) = extent(&control);
        assert!(
            (width - expected.0).abs() < 0.5 && (height - expected.1).abs() < 0.5,
            "{size:?} measured {width}x{height}"
        );
        drop(owner);
    }
}
/// Every requested glyph extent reaches the SVG, which one fixed size would hide.
fn assert_glyph_extents(view: &DomView) {
    let cases = [
        (IconSize::Xs, "12px"),
        (IconSize::Sm, "16px"),
        (IconSize::Md, "20px"),
        (IconSize::Lg, "24px"),
        (IconSize::Xl, "32px"),
    ];
    for (icon, pixels) in cases {
        let props = shaped(ButtonSize::Sm, ButtonVariant::Ghost, icon, None, false);
        let (owner, control) = mounted(view, props);
        let glyph = glyph(&control);
        assert_eq!(computed(&glyph, "width"), pixels, "glyph extent {icon:?}");
        assert_eq!(computed(&glyph, "height"), pixels, "glyph extent {icon:?}");
        drop(owner);
    }
}
/// Each variant paints a background, text color and border no other variant shares.
fn assert_variants(view: &DomView) {
    let variants = [
        ButtonVariant::Default,
        ButtonVariant::Destructive,
        ButtonVariant::Outline,
        ButtonVariant::Secondary,
        ButtonVariant::Ghost,
        ButtonVariant::Link,
    ];
    let mut painted: Vec<[String; 3]> = Vec::new();
    for variant in variants {
        let props = shaped(ButtonSize::Sm, variant, IconSize::Sm, None, false);
        let (owner, control) = mounted(view, props);
        let paint = paint(&control);
        assert!(!painted.contains(&paint), "{variant:?} paints like another");
        painted.push(paint);
        drop(owner);
    }
}
/// Every admitted text payload keeps its exact bytes.
async fn assert_text_payloads(view: &DomView, fixture: &Fixture) {
    for (label, content, forced, expected) in TEXT_PAYLOADS {
        let content = DownloadContent::Text((*content).into());
        let mut props = DownloadButtonProps::new(content, "file.bin".into());
        props.is_base64 = *forced;
        let (owner, control) = mounted(view, props);
        fixture
            .saved(&control, expected, "text/plain", "file.bin")
            .await;
        drop(owner);
        assert!(!label.is_empty());
    }
}
/// Array operands save the caller's current view only.
async fn assert_byte_payloads(view: &DomView, fixture: &Fixture) {
    let array = Uint8Array::from(&[250u8, 1, 2, 249][..]);
    let windowed = array.subarray(1, 3);
    let (owner, control) = mounted(
        view,
        DownloadButtonProps::new(DownloadContent::Bytes(windowed), "file.bin".into()),
    );
    fixture
        .saved(&control, &[1, 2], "text/plain", "file.bin")
        .await;
    drop(owner);
    let empty = Uint8Array::new_with_length(0);
    let (owner, control) = mounted(
        view,
        DownloadButtonProps::new(DownloadContent::Bytes(empty), "file.bin".into()),
    );
    fixture.saved(&control, &[], "text/plain", "file.bin").await;
    drop(owner);
    let binary = Uint8Array::from(&[0u8, 128, 255, 13, 10][..]);
    let (owner, control) = mounted(
        view,
        DownloadButtonProps::new(DownloadContent::Bytes(binary), "file.bin".into()),
    );
    fixture
        .saved(&control, &[0, 128, 255, 13, 10], "text/plain", "file.bin")
        .await;
    drop(owner);
}
/// The browser's own Blob keeps its MIME semantics.
async fn assert_mime_types(view: &DomView, fixture: &Fixture) {
    for (supplied, expected) in MIME_TYPES {
        let mut props =
            DownloadButtonProps::new(DownloadContent::Text("plain!".into()), "file.bin".into());
        props.mime_type = (*supplied).into();
        let (owner, control) = mounted(view, props);
        fixture
            .saved(&control, b"plain!", expected, "file.bin")
            .await;
        drop(owner);
    }
}

/// A custom tooltip needs no localization, so refused language reads still construct the control.
fn assert_tooltip_without_localization(view: &DomView) {
    let host = crate::support::Host::default();
    host.fail.replace("get".into());
    let refusing = DomView {
        i18n: Rc::new(maestro_web::I18n::new(Rc::new(host)).unwrap()),
        ..view.clone()
    };
    let props = shaped(
        ButtonSize::Sm,
        ButtonVariant::Ghost,
        IconSize::Sm,
        Some("Exact title".into()),
        false,
    );
    let (owner, control) = mounted(&refusing, props);
    assert_eq!(control.title(), "Exact title");
    drop(owner);
    let defaulted = download_button(
        &refusing,
        DownloadButtonProps::new(DownloadContent::Text("payload".into()), "file.bin".into()),
    );
    assert!(
        defaulted.is_err(),
        "a default tooltip is the only consumer of the refused localization"
    );
}
/// Defaults, tooltips, labels, sizes, variants and glyph extents reach their native consumers.
#[wasm_bindgen]
pub async fn download_dom_defaults_and_caller_options(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    let fixture = Fixture::import(import_module).await;
    assert_default_control(&view);
    assert_filenames(&view, &fixture).await;
    assert_titles(&view);
    assert_tooltip_without_localization(&view);
    assert_sizes(&view);
    assert_glyph_extents(&view);
    assert_variants(&view);
}

/// Every admitted payload keeps its exact bytes through the one native byte sink.
#[wasm_bindgen]
pub async fn download_dom_preserves_selected_payload_bytes(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    let fixture = Fixture::import(import_module).await;
    assert_text_payloads(&view, &fixture).await;
    assert_byte_payloads(&view, &fixture).await;
    assert_mime_types(&view, &fixture).await;
}

/// Real pointer and keyboard activation each save once and never submit a form.
#[wasm_bindgen]
pub async fn download_dom_activation_uses_one_scoped_download(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    let fixture = Fixture::import(import_module).await;
    let form = view.document.create_element("form").unwrap();
    body().append_child(&form).unwrap();
    let submits = Submitting::new(&form);
    let (pointer_owner, pointer) = mounted(
        &view,
        DownloadButtonProps::new(
            DownloadContent::Text("pointer".into()),
            "pointer.bin".into(),
        ),
    );
    // A Base64-shaped payload would be decoded; this control saves a literal payload.
    let (keys_owner, keys) = mounted(
        &view,
        DownloadButtonProps::new(DownloadContent::Text("keys!".into()), "keys.bin".into()),
    );
    form.append_child(&pointer).unwrap();
    form.append_child(&keys).unwrap();
    let rect = pointer.get_bounding_client_rect();
    let point = format!(
        "[{},{},true]",
        rect.x() + rect.width() / 2.,
        rect.y() + rect.height() / 2.
    );
    fixture.begin("success");
    driver("driverPointer", &point).await;
    let observed = fixture.finish(b"pointer", "text/plain", "pointer.bin", true);
    wasm_bindgen_futures::JsFuture::from(observed)
        .await
        .unwrap();
    keys.focus().unwrap();
    assert!(
        view.document
            .active_element()
            .is_some_and(|active| active.is_same_node(Some(keys.unchecked_ref())))
    );
    fixture.begin("success");
    driver("driverPress", "Enter").await;
    let observed = fixture.finish(b"keys!", "text/plain", "keys.bin", true);
    wasm_bindgen_futures::JsFuture::from(observed)
        .await
        .unwrap();
    assert_eq!(submits.count(), 0, "a download button is no submit control");
    drop((pointer_owner, keys_owner, submits));
}

/// A control reads live byte views at activation and releases its route with its root.
#[wasm_bindgen]
pub async fn download_dom_retains_activation_inputs_and_releases_on_drop(
    import_module: &js_sys::Function,
) {
    let view = view();
    let _host = host(&view, import_module);
    let fixture = Fixture::import(import_module).await;
    let buffer = Uint8Array::from(&[250u8, 1, 2, 249][..]);
    let windowed = buffer.subarray(1, 3);
    let (window_owner, window_control) = mounted(
        &view,
        DownloadButtonProps::new(DownloadContent::Bytes(windowed), "window.bin".into()),
    );
    buffer.set_index(1, 42);
    buffer.set_index(2, 43);
    fixture
        .saved(&window_control, &[42, 43], "text/plain", "window.bin")
        .await;
    drop(window_owner);
    // Text is owned on construction, so no later caller change can reach the control.
    let captured = String::from("first!");
    let (text_owner, text_control) = mounted(
        &view,
        DownloadButtonProps::new(DownloadContent::Text(captured.clone()), "text.bin".into()),
    );
    drop(captured);
    fixture
        .saved(&text_control, b"first!", "text/plain", "text.bin")
        .await;
    drop(text_owner);
    let (released, released_control) = mounted(
        &view,
        DownloadButtonProps::new(
            DownloadContent::Text("released".into()),
            "released.bin".into(),
        ),
    );
    let (kept, kept_control) = mounted(
        &view,
        DownloadButtonProps::new(DownloadContent::Text("kept!".into()), "kept.bin".into()),
    );
    drop(released);
    fixture.begin("success");
    released_control.click();
    kept_control.click();
    let observed = fixture.finish(b"kept!", "text/plain", "kept.bin", true);
    wasm_bindgen_futures::JsFuture::from(observed)
        .await
        .unwrap();
    drop(kept);
}

/// A listener registered on an element and removed with the recording that owns it.
struct Listening<A: ?Sized> {
    /// Registered native target.
    target: Element,
    /// Registered event name.
    name: &'static str,
    /// Callable kept alive while registered.
    listener: Closure<A>,
}
impl<A: ?Sized> Listening<A> {
    /// Register `listener` for `name` on `target`.
    fn new(target: Element, name: &'static str, listener: Closure<A>) -> Self {
        target
            .add_event_listener_with_callback(name, listener.as_ref().unchecked_ref())
            .unwrap();
        Self {
            target,
            name,
            listener,
        }
    }
}
impl<A: ?Sized> Drop for Listening<A> {
    fn drop(&mut self) {
        let _ = self
            .target
            .remove_event_listener_with_callback(self.name, self.listener.as_ref().unchecked_ref());
    }
}
/// One observed changed selection.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Changed {
    /// Exact event detail.
    detail: String,
    /// Presentation already selected when the listener ran.
    selected: String,
}
/// Changed selections observed below a listening element.
struct Recorder {
    /// Observed changes so far.
    seen: Rc<RefCell<Vec<Changed>>>,
    /// Registration removed with the recorder.
    _listening: Listening<dyn Fn(CustomEvent)>,
}
impl Recorder {
    /// Record every change of one standalone group, which reports and presents itself.
    fn group(group: &Element) -> Self {
        let shown = group.clone();
        Self::new(group, group, move || Some(shown.clone()))
    }
    /// Record the changes an element dispatches, reading the group it presents at that moment.
    fn element(element: &Element) -> Self {
        let host = element.clone();
        Self::new(element, element, move || host.first_element_child())
    }
    /// Record every change `origin` dispatches below `listener`, presenting what `current` yields.
    fn new(
        listener: &Element,
        origin: &Element,
        current: impl Fn() -> Option<Element> + 'static,
    ) -> Self {
        let seen: Rc<RefCell<Vec<Changed>>> = Rc::default();
        let (log, watched) = (Rc::clone(&seen), origin.clone());
        let callback = Closure::<dyn Fn(CustomEvent)>::new(move |event: CustomEvent| {
            assert!(
                event.bubbles() && !event.composed() && !event.cancelable(),
                "a changed selection bubbles and may be neither composed nor canceled"
            );
            let target = event
                .target()
                .and_then(|target| target.dyn_into::<Node>().ok())
                .is_some_and(|target| target.is_same_node(Some(watched.unchecked_ref())));
            assert!(
                target,
                "the event originates at the element the page listens on"
            );
            log.borrow_mut().push(Changed {
                detail: event.detail().as_string().unwrap_or_default(),
                selected: current()
                    .map(|group| mode_of(&group).to_owned())
                    .unwrap_or_default(),
            });
        });
        Self {
            seen,
            _listening: Listening::new(listener.clone(), "mode-change", callback),
        }
    }
    /// Every change observed so far.
    fn observed(&self) -> Vec<Changed> {
        self.seen.borrow().clone()
    }
    /// How many changes were observed.
    fn len(&self) -> usize {
        self.seen.borrow().len()
    }
}
/// Form submissions counted while the recorder lives.
pub(crate) struct Submitting {
    /// Submissions observed so far.
    count: Rc<Cell<u32>>,
    /// Registration removed with the recorder.
    _listening: Listening<dyn Fn(Event)>,
}
impl Submitting {
    /// Count submissions of `form` without navigating.
    pub(crate) fn new(form: &Element) -> Self {
        let count = Rc::new(Cell::new(0u32));
        let counted = Rc::clone(&count);
        let callback = Closure::<dyn Fn(Event)>::new(move |event: Event| {
            event.prevent_default();
            counted.set(counted.get() + 1);
        });
        Self {
            count,
            _listening: Listening::new(form.clone(), "submit", callback),
        }
    }
    /// How many submissions were observed.
    pub(crate) fn count(&self) -> u32 {
        self.count.get()
    }
}
/// Mount one typed toggle and return the root that owns it with its group.
fn mounted_toggle(view: &DomView, toggle: &PreviewCodeToggle) -> (DomRoot, HtmlElement) {
    let root = toggle.mount(view).unwrap();
    let group = root.node.clone().unchecked_into::<HtmlElement>();
    body().append_child(&root.node).unwrap();
    (root, group)
}
/// The group's two buttons in native order.
fn buttons(group: &Element) -> [HtmlElement; 2] {
    let mut next = group.first_element_child();
    array::from_fn(|_| {
        let button = next.clone().unwrap().unchecked_into::<HtmlElement>();
        next = button.next_element_sibling();
        button
    })
}
/// The presentation `group` currently shows.
fn mode_of(group: &Element) -> &'static str {
    let selected = buttons(group)[1].class_name().contains(SELECTED);
    if selected { "code" } else { "preview" }
}
/// Whether `group` shows `mode` selected.
fn shows(group: &Element, mode: ViewMode) -> bool {
    let index = usize::from(mode == ViewMode::Code);
    buttons(group)[index].class_name().contains(SELECTED)
}
/// The element's `mode` property, as a page author reads it.
fn property_mode(element: &Element) -> Option<String> {
    js_sys::Reflect::get(element, &"mode".into())
        .unwrap()
        .as_string()
}
/// Assign the element's `mode` property, as a page author does.
fn set_property_mode(element: &Element, mode: &str) {
    js_sys::Reflect::set(element, &"mode".into(), &mode.into()).unwrap();
}
/// Whether the document's active element is `expected`.
fn focused(view: &DomView, expected: &HtmlElement) -> bool {
    view.document
        .active_element()
        .is_some_and(|active| active.is_same_node(Some(expected.unchecked_ref())))
}
/// One observed change with the presentation its listener saw.
fn changed(detail: &str, selected: &str) -> Changed {
    Changed {
        detail: detail.into(),
        selected: selected.into(),
    }
}
/// A connected, defined preview/source element with its group.
fn element_group(view: &DomView) -> (Element, Element) {
    let element = view.document.create_element("preview-code-toggle").unwrap();
    body().append_child(&element).unwrap();
    let group = element.first_element_child().unwrap();
    (element, group)
}

/// Defaults, silent publication and the unreflected element property agree.
#[wasm_bindgen]
pub fn preview_toggle_dom_defaults_and_programmatic_selection(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    assert_typed_defaults(&view);
    assert_element_property(&view);
    assert_controller_replacement(&view);
    assert_updated_titles(&view);
    view.i18n.set_language("de").unwrap();
    let (_, group) = element_group(&view);
    let [preview, code] = buttons(&group);
    assert_eq!(
        (preview.title(), code.title()),
        ("Vorschau".into(), "Code".into())
    );
    view.i18n.set_language("en").unwrap();
}
/// The typed controller starts in preview and publishes silently.
fn assert_typed_defaults(view: &DomView) {
    let toggle = PreviewCodeToggle::new();
    assert_eq!(toggle.mode(), ViewMode::Preview);
    let (owner, group) = mounted_toggle(view, &toggle);
    assert_eq!(group.class_name(), "maestro-toggle");
    let [preview, code] = buttons(&group);
    assert_eq!(
        (preview.title(), code.title()),
        ("Preview".into(), "Code".into())
    );
    assert_eq!(preview.get_attribute("type"), None);
    assert_eq!(
        preview.unchecked_ref::<HtmlButtonElement>().type_(),
        "submit"
    );
    assert!(shows(&group, ViewMode::Preview) && !shows(&group, ViewMode::Code));
    assert!(
        (extent(&group).1 - 28.).abs() < 0.5,
        "the group measures the reference's 28-pixel control line"
    );
    assert_eq!(computed(&group, "overflow"), "hidden");
    assert_eq!(computed(&glyph(&preview), "width"), "16px");
    let recorder = Recorder::group(&group);
    toggle.set_mode(ViewMode::Code);
    toggle.update(&owner, view).unwrap();
    assert!(
        recorder.observed().is_empty(),
        "a typed assignment is silent"
    );
    assert!(shows(&group, ViewMode::Code));
    let [preview_now, code_now] = buttons(&group);
    assert!(
        preview_now.is_same_node(Some(preview.unchecked_ref()))
            && code_now.is_same_node(Some(code.unchecked_ref())),
        "publication keeps both native nodes"
    );
    code.focus().unwrap();
    toggle.update(&owner, view).unwrap();
    assert!(focused(view, &code), "publication keeps focus");
    assert_eq!(toggle.mode(), ViewMode::Code);
    drop(owner);
}
/// The element route reads and writes one retained enum without reflecting it.
fn assert_element_property(view: &DomView) {
    let (element, group) = element_group(view);
    assert_eq!(property_mode(&element).as_deref(), Some("preview"));
    assert_eq!(element.get_attribute("mode"), None);
    let recorder = Recorder::element(&element);
    set_property_mode(&element, "code");
    assert_eq!(property_mode(&element).as_deref(), Some("code"));
    assert_eq!(
        group.class_name(),
        "maestro-toggle",
        "a property write keeps the group's own classes"
    );
    assert!(
        shows(&group, ViewMode::Code),
        "a property write repaints the group's buttons"
    );
    assert!(
        recorder.observed().is_empty(),
        "a property assignment is silent"
    );
    assert_eq!(element.get_attribute("mode"), None);
}
/// A later controller takes over the mounted group and its clicks.
fn assert_controller_replacement(view: &DomView) {
    let first = PreviewCodeToggle::new();
    let (owner, group) = mounted_toggle(view, &first);
    let recorder = Recorder::group(&group);
    first.set_mode(ViewMode::Code);
    first.update(&owner, view).unwrap();
    assert!(shows(&group, ViewMode::Code));
    let second = PreviewCodeToggle::new();
    second.update(&owner, view).unwrap();
    assert!(
        shows(&group, ViewMode::Preview),
        "the later controller shows its own selection"
    );
    let [preview, code] = buttons(&group);
    code.click();
    assert_eq!(
        second.mode(),
        ViewMode::Code,
        "a click reaches the controller shown last"
    );
    assert_eq!(recorder.observed(), [changed("code", "code")]);
    assert!(shows(&group, ViewMode::Code));
    let [preview_now, code_now] = buttons(&group);
    assert!(
        preview_now.is_same_node(Some(preview.unchecked_ref()))
            && code_now.is_same_node(Some(code.unchecked_ref())),
        "a taken-over group keeps its native buttons"
    );
    drop(owner);
}
/// An update refreshes the mounted titles from the current localization.
fn assert_updated_titles(view: &DomView) {
    let toggle = PreviewCodeToggle::new();
    let (owner, group) = mounted_toggle(view, &toggle);
    let [preview, code] = buttons(&group);
    view.i18n.set_language("de").unwrap();
    toggle.update(&owner, view).unwrap();
    assert_eq!(
        (preview.title(), code.title()),
        ("Vorschau".into(), "Code".into()),
        "an update localizes the mounted titles"
    );
    view.i18n.set_language("en").unwrap();
    toggle.update(&owner, view).unwrap();
    assert_eq!(
        (preview.title(), code.title()),
        ("Preview".into(), "Code".into())
    );
    drop(owner);
}

/// User transitions report one changed mode with its exact event shape.
#[wasm_bindgen]
pub fn preview_toggle_dom_emits_only_changed_bubbling_modes(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    assert_reported_transitions(&view);
    assert_retargeting(&view);
    assert_independent_instances(&view);
    assert_form_semantics(&view);
}
/// Selecting a new mode reports it once; the current mode reports nothing.
fn assert_reported_transitions(view: &DomView) {
    let toggle = PreviewCodeToggle::new();
    let (owner, group) = mounted_toggle(view, &toggle);
    let recorder = Recorder::group(&group);
    let [preview, code] = buttons(&group);
    preview.click();
    assert!(
        recorder.observed().is_empty(),
        "the current mode reports nothing"
    );
    code.click();
    assert_eq!(recorder.observed(), [changed("code", "code")]);
    code.click();
    assert_eq!(recorder.len(), 1, "a repeated selection reports nothing");
    preview.click();
    assert_eq!(
        recorder.observed(),
        [changed("code", "code"), changed("preview", "preview")]
    );
    assert_eq!(toggle.mode(), ViewMode::Preview);
    drop(owner);
}
/// A listener that selects again keeps its own choice after the dispatch.
fn assert_retargeting(view: &DomView) {
    let toggle = PreviewCodeToggle::new();
    let (owner, group) = mounted_toggle(view, &toggle);
    let recorder = Recorder::group(&group);
    let watched = group.clone();
    let retarget = Closure::<dyn Fn(CustomEvent)>::new(move |event: CustomEvent| {
        if event.detail().as_string().as_deref() == Some("code") {
            buttons(&watched)[0].click();
        }
    });
    let _listening = Listening::new(group.clone().into(), "mode-change", retarget);
    buttons(&group)[1].click();
    let details: Vec<_> = recorder
        .observed()
        .into_iter()
        .map(|change| change.detail)
        .collect();
    assert_eq!(details, ["code", "preview"]);
    assert_eq!(toggle.mode(), ViewMode::Preview);
    assert!(shows(&group, ViewMode::Preview));
    drop(owner);
}
/// Two independent controls never share a selection.
fn assert_independent_instances(view: &DomView) {
    let (first_owner, first) = mounted_toggle(view, &PreviewCodeToggle::new());
    let (second_owner, second) = mounted_toggle(view, &PreviewCodeToggle::new());
    buttons(&first)[1].click();
    assert!(shows(&first, ViewMode::Code) && shows(&second, ViewMode::Preview));
    drop((first_owner, second_owner));
}
/// The toggle button keeps its native form behavior; a download button does not.
fn assert_form_semantics(view: &DomView) {
    let form = view.document.create_element("form").unwrap();
    body().append_child(&form).unwrap();
    let toggle = PreviewCodeToggle::new();
    let (toggle_owner, group) = mounted_toggle(view, &toggle);
    form.append_child(&group).unwrap();
    let submits = Submitting::new(&form);
    buttons(&group)[1].click();
    assert_eq!((submits.count(), toggle.mode()), (1, ViewMode::Code));
    assert_eq!(buttons(&group)[1].get_attribute("type"), None);
    let (download_owner, download) = mounted(
        view,
        DownloadButtonProps::new(DownloadContent::Text("payload".into()), "file.bin".into()),
    );
    form.append_child(&download).unwrap();
    assert_eq!(
        download.unchecked_ref::<HtmlButtonElement>().type_(),
        "button"
    );
    assert_eq!(
        submits.count(),
        1,
        "a download button is the negative neighbor"
    );
    drop((toggle_owner, download_owner, submits));
}

/// Disconnection removes the handling, reconnection restores it once with the selection.
#[wasm_bindgen]
pub fn preview_toggle_dom_reconnects_without_duplicate_handlers(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    assert_reconnection(&view);
    assert_released_root(&view);
}
/// The element reports once per action across a disconnection.
fn assert_reconnection(view: &DomView) {
    let (element, group) = element_group(view);
    let recorder = Recorder::element(&element);
    buttons(&group)[1].click();
    assert_eq!(recorder.len(), 1);
    let detached = element.first_element_child().unwrap();
    element.remove();
    assert!(
        element.first_element_child().is_none(),
        "the detached group is released"
    );
    buttons(&detached)[0].click();
    assert_eq!(recorder.len(), 1, "a detached control reports nothing");
    set_property_mode(&element, "code");
    assert_eq!(property_mode(&element).as_deref(), Some("code"));
    body().append_child(&element).unwrap();
    let group = element.first_element_child().unwrap();
    assert!(
        shows(&group, ViewMode::Code),
        "the selection survives reconnection"
    );
    buttons(&group)[0].click();
    assert_eq!(recorder.len(), 2, "one handler per reconnection");
    buttons(&group)[0].click();
    assert_eq!(recorder.len(), 2);
    element.remove();
    let (fresh, group) = element_group(view);
    assert!(
        shows(&group, ViewMode::Preview),
        "a fresh element starts in preview"
    );
    assert_eq!(property_mode(&fresh).as_deref(), Some("preview"));
}
/// A released typed root keeps its node but reports nothing.
fn assert_released_root(view: &DomView) {
    let toggle = PreviewCodeToggle::new();
    let (owner, group) = mounted_toggle(view, &toggle);
    let recorder = Recorder::group(&group);
    drop(owner);
    buttons(&group)[1].click();
    assert!(
        recorder.observed().is_empty(),
        "a released root reports nothing"
    );
}

/// Both controls paint the selected pack's roles and stay keyboard usable.
#[wasm_bindgen]
pub async fn artifact_controls_dom_paint_tokens_and_keyboard_actions(
    import_module: &js_sys::Function,
) {
    let view = view();
    let _host = host(&view, import_module);
    assert_selected_paint(&view).await;
    assert_keyboard(&view).await;
    assert_light_tokens(&view);
}
/// Selected, resting and hovered controls paint the selected pack's roles.
async fn assert_selected_paint(view: &DomView) {
    let presentation = view.pack.presentation(view.mode).unwrap();
    let role = |name: &str| rgb(presentation.colors[name]);
    let toggle = PreviewCodeToggle::new();
    let (owner, group) = mounted_toggle(view, &toggle);
    let [preview, code] = buttons(&group);
    assert_eq!(computed(&group, "border-top-color"), role("border"));
    assert_eq!(computed(&preview, "background-color"), role("background"));
    assert_eq!(computed(&preview, "color"), role("text"));
    assert_eq!(computed(&code, "color"), role("muted"));
    driver("driverPointer", "[0,0,false]").await;
    assert_eq!(
        computed(&code, "color"),
        role("muted"),
        "resting stays muted"
    );
    let rect = code.get_bounding_client_rect();
    let point = format!(
        "[{},{},false]",
        rect.x() + rect.width() / 2.,
        rect.y() + rect.height() / 2.
    );
    driver("driverPointer", &point).await;
    assert_eq!(
        computed(&code, "color"),
        role("text"),
        "hovering brightens it"
    );
    driver("driverPointer", "[0,0,false]").await;
    drop(owner);
    group.remove();
}
/// Tab traversal stays in native order, repeatable, and Enter activates.
async fn assert_keyboard(view: &DomView) {
    let toggle = PreviewCodeToggle::new();
    let (owner, group) = mounted_toggle(view, &toggle);
    let [preview, code] = buttons(&group);
    let recorder = Recorder::group(&group);
    driver("driverPress", "Tab").await;
    assert!(focused(view, &preview), "the first control takes focus");
    let accent = rgb(view.pack.presentation(view.mode).unwrap().colors["accent"]);
    assert!(
        computed(&preview, "box-shadow").contains(&accent),
        "a focused control paints the pack's accent ring"
    );
    driver("driverPress", "Tab").await;
    assert!(focused(view, &code));
    driver("driverPress", "Shift+Tab").await;
    assert!(focused(view, &preview));
    toggle.update(&owner, view).unwrap();
    toggle.update(&owner, view).unwrap();
    driver("driverPress", "Tab").await;
    assert!(
        focused(view, &code),
        "repeated publication adds no focus stop"
    );
    driver("driverPress", "Enter").await;
    assert_eq!(toggle.mode(), ViewMode::Code);
    assert_eq!(recorder.len(), 1);
    drop(owner);
    group.remove();
}
/// Both brand modes reach the controls through the one token projection.
fn assert_light_tokens(view: &DomView) {
    let light = view.pack.presentation(BrandMode::Light).unwrap();
    let wrapper = view.document.create_element("div").unwrap();
    body().append_child(&wrapper).unwrap();
    let style = wrapper.unchecked_ref::<HtmlElement>().style();
    for name in ["background", "text", "muted", "border"] {
        style
            .set_property(&format!("--maestro-color-{name}"), light.colors[name])
            .unwrap();
    }
    let (owner, group) = mounted_toggle(view, &PreviewCodeToggle::new());
    wrapper.append_child(&group).unwrap();
    let [preview, code] = buttons(&group);
    let role = |name: &str| rgb(light.colors[name]);
    assert_eq!(computed(&preview, "background-color"), role("background"));
    assert_eq!(computed(&preview, "color"), role("text"));
    assert_eq!(computed(&code, "color"), role("muted"));
    assert_eq!(computed(&group, "border-top-color"), role("border"));
    drop(owner);
    wrapper.remove();
}
