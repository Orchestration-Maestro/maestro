//! Native download and preview/source controls in a real document.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::contributions_support::{body, computed, driver, host, text, view};
use crate::session_download_browser::Fixture;
use js_sys::Uint8Array;
use maestro_web::{
    components::{
        button::{ButtonSize, ButtonVariant},
        download_button::dom::{DownloadButtonProps, DownloadContent, download_button},
    },
    dom::{DomRoot, DomView},
    icons::IconSize,
};
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::{JsCast as _, closure::Closure, prelude::wasm_bindgen};
use web_sys::{Element, Event, HtmlElement};

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
/// Activate `control` and observe one complete native download of `bytes`.
async fn saved(fixture: &Fixture, control: &HtmlElement, bytes: &[u8], mime: &str, name: &str) {
    fixture.begin("success");
    control.click();
    wasm_bindgen_futures::JsFuture::from(fixture.finish(bytes, mime, name, true))
        .await
        .unwrap();
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
        saved(fixture, &control, b"plain!", "text/plain", expected).await;
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
        saved(fixture, &control, expected, "text/plain", "file.bin").await;
        drop(owner);
        assert!(expected.len() < 64, "{label} decoded its own payload");
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
    saved(fixture, &control, &[1, 2], "text/plain", "file.bin").await;
    drop(owner);
    let empty = Uint8Array::new_with_length(0);
    let (owner, control) = mounted(
        view,
        DownloadButtonProps::new(DownloadContent::Bytes(empty), "file.bin".into()),
    );
    saved(fixture, &control, &[], "text/plain", "file.bin").await;
    drop(owner);
    let binary = Uint8Array::from(&[0u8, 128, 255, 13, 10][..]);
    let (owner, control) = mounted(
        view,
        DownloadButtonProps::new(DownloadContent::Bytes(binary), "file.bin".into()),
    );
    saved(
        fixture,
        &control,
        &[0, 128, 255, 13, 10],
        "text/plain",
        "file.bin",
    )
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
        saved(fixture, &control, b"plain!", expected, "file.bin").await;
        drop(owner);
    }
}

/// Defaults, tooltips, labels, sizes, variants and glyph extents reach their native consumers.
#[wasm_bindgen]
pub async fn download_dom_defaults_and_caller_options(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    let fixture = Fixture::load().await;
    assert_default_control(&view);
    assert_filenames(&view, &fixture).await;
    assert_titles(&view);
    assert_sizes(&view);
    assert_glyph_extents(&view);
    assert_variants(&view);
}

/// Every admitted payload keeps its exact bytes through the one native byte sink.
#[wasm_bindgen]
pub async fn download_dom_preserves_selected_payload_bytes(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    let fixture = Fixture::load().await;
    assert_text_payloads(&view, &fixture).await;
    assert_byte_payloads(&view, &fixture).await;
    assert_mime_types(&view, &fixture).await;
}

/// Real pointer and keyboard activation each save once and never submit a form.
#[wasm_bindgen]
pub async fn download_dom_activation_uses_one_scoped_download(import_module: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import_module);
    let fixture = Fixture::load().await;
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
    let (keys_owner, keys) = mounted(
        &view,
        DownloadButtonProps::new(DownloadContent::Text("keys".into()), "keys.bin".into()),
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
    let observed = fixture.finish(b"keys", "text/plain", "keys.bin", true);
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
    let fixture = Fixture::load().await;
    let buffer = Uint8Array::from(&[250u8, 1, 2, 249][..]);
    let windowed = buffer.subarray(1, 3);
    let (window_owner, window_control) = mounted(
        &view,
        DownloadButtonProps::new(DownloadContent::Bytes(windowed), "window.bin".into()),
    );
    buffer.set_index(1, 42);
    buffer.set_index(2, 43);
    saved(
        &fixture,
        &window_control,
        &[42, 43],
        "text/plain",
        "window.bin",
    )
    .await;
    drop(window_owner);
    // Text is owned on construction, so no later caller change can reach the control.
    let captured = String::from("first!");
    let (text_owner, text_control) = mounted(
        &view,
        DownloadButtonProps::new(DownloadContent::Text(captured.clone()), "text.bin".into()),
    );
    drop(captured);
    saved(&fixture, &text_control, b"first!", "text/plain", "text.bin").await;
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
        DownloadButtonProps::new(DownloadContent::Text("kept".into()), "kept.bin".into()),
    );
    drop(released);
    fixture.begin("success");
    released_control.click();
    kept_control.click();
    let observed = fixture.finish(b"kept", "text/plain", "kept.bin", true);
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
/// Form submissions counted while the recorder lives.
struct Submitting {
    /// Submissions observed so far.
    count: Rc<Cell<u32>>,
    /// Registration removed with the recorder.
    _listening: Listening<dyn Fn(Event)>,
}
impl Submitting {
    /// Count submissions of `form` without navigating.
    fn new(form: &Element) -> Self {
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
    fn count(&self) -> u32 {
        self.count.get()
    }
}
