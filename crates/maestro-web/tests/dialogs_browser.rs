//! Native dialogs, the shared field and the masked prompt in a real page.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    contributions_support::{
        body, computed, container, document, host, localized_view, rgb, text, view,
    },
    dialog_support::{panel, panels, press, settled, type_text},
};
use maestro_theme::BrandMode;
use maestro_web::{
    clear_auth_token,
    components::input::{Input, InputProps, InputSize, InputType},
    dialogs::{
        dialog::{
            DialogHeaderProps, DialogProps, dialog, dialog_content, dialog_footer, dialog_header,
        },
        dialog_base::{DialogBase, MODAL_HEIGHT, MODAL_WIDTH},
        prompt_dialog::PromptDialog,
    },
    dom::DomView,
    get_auth_token,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, prelude::wasm_bindgen};
use web_sys::{Element, HtmlElement};

/// A close counter and the callback that advances it.
fn counter() -> (Rc<Cell<u32>>, Rc<dyn Fn()>) {
    let count = Rc::new(Cell::new(0));
    let seen = Rc::clone(&count);
    (count, Rc::new(move || seen.set(seen.get() + 1)))
}
/// A paragraph child with literal text.
fn paragraph(text: &str) -> web_sys::Node {
    let node = document().create_element("p").unwrap();
    node.set_text_content(Some(text));
    node.into()
}
/// One required descendant.
fn find(root: &Element, selector: &str) -> Element {
    root.query_selector(selector).unwrap().unwrap()
}
/// Click `element` as the page would.
fn click(element: &Element) {
    element.unchecked_ref::<HtmlElement>().click();
}

/// A closed dialog mounts nothing and registers nothing.
#[wasm_bindgen]
pub async fn dom_dialog_closed_content_has_no_live_handling() {
    let view = view();
    let (closed, on_close) = counter();
    let props = DialogProps {
        is_open: false,
        on_close: Some(on_close),
        ..DialogProps::new(vec![paragraph("hidden")])
    };
    assert!(dialog(&view, &props).unwrap().is_none());
    press("Escape").await;
    assert_eq!(closed.get(), 0);
    assert!(panels().is_empty());
}

/// Backdrop, Close and Escape each notify the one callback once; panel clicks do not.
#[wasm_bindgen]
pub async fn dom_dialog_close_routes_share_one_owned_notification() {
    let view = view();
    let (closed, on_close) = counter();
    let props = DialogProps {
        on_close: Some(on_close),
        ..DialogProps::new(vec![paragraph("body")])
    };
    let root = dialog(&view, &props).unwrap().unwrap();
    body().append_child(&root.node).unwrap();
    let backdrop: Element = root.node.clone().unchecked_into();
    let outer = Rc::new(Cell::new(0));
    let seen = Rc::clone(&outer);
    let listener =
        wasm_bindgen::closure::Closure::<dyn Fn()>::new(move || seen.set(seen.get() + 1));
    document()
        .add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())
        .unwrap();
    click(&find(&backdrop, "p"));
    click(&panel());
    assert_eq!(
        (closed.get(), outer.get()),
        (0, 0),
        "panel clicks stay inside the dialog"
    );
    click(&backdrop);
    assert_eq!(closed.get(), 1);
    let close = find(&backdrop, "button.maestro-dialog-close");
    assert_eq!(close.get_attribute("type").as_deref(), Some("button"));
    click(&close);
    assert_eq!(closed.get(), 2);
    press("Escape").await;
    assert_eq!(closed.get(), 3);
    press("Enter").await;
    assert_eq!(closed.get(), 3);
    document()
        .remove_event_listener_with_callback("click", listener.as_ref().unchecked_ref())
        .unwrap();
    backdrop.remove();
}

/// Two open dialogs each close once on one Escape; a later Escape finds none.
#[wasm_bindgen]
pub async fn dom_dialog_escape_notifies_each_open_owner_once() {
    let view = view();
    let counts: Vec<_> = (0..2)
        .map(|_| {
            let base = DialogBase::new(Rc::clone(&view), MODAL_WIDTH, MODAL_HEIGHT);
            let (count, on_close) = counter();
            base.set_on_close(Some(on_close));
            base.open(vec![paragraph("stacked")], None).unwrap();
            (base, count)
        })
        .collect();
    assert_eq!(panels().len(), 2);
    press("Escape").await;
    for (base, count) in &counts {
        assert!(!base.is_open());
        assert_eq!(count.get(), 1);
    }
    assert!(panels().is_empty());
    press("Escape").await;
    assert!(counts.iter().all(|(_, count)| count.get() == 1));
}

/// Dropping a dialog root or closing an opened dialog releases its Escape listener.
#[wasm_bindgen]
pub async fn dom_dialog_releases_escape_after_close() {
    let view = view();
    let (closed, on_close) = counter();
    let props = DialogProps {
        on_close: Some(on_close),
        ..DialogProps::new(vec![])
    };
    let root = dialog(&view, &props).unwrap().unwrap();
    drop(root);
    press("Escape").await;
    assert_eq!(closed.get(), 0);
    let base = DialogBase::new(Rc::clone(&view), MODAL_WIDTH, MODAL_HEIGHT);
    let (count, on_close) = counter();
    base.set_on_close(Some(on_close));
    base.open(vec![], None).unwrap();
    base.open(vec![], None).unwrap();
    assert_eq!(
        panels().len(),
        1,
        "reopening an open dialog mounts nothing more"
    );
    base.close();
    base.close();
    assert_eq!(count.get(), 1);
    press("Escape").await;
    assert_eq!(count.get(), 1);
    let weak = {
        let base = DialogBase::new(Rc::clone(&view), "1px", "1px");
        base.open(vec![], None).unwrap();
        base.close();
        Rc::downgrade(&Rc::new(base))
    };
    assert!(weak.upgrade().is_none());
}

/// Dimensions are literal, content stays in the page tree, and focus returns after closing.
#[wasm_bindgen]
pub async fn dom_dialog_keeps_dimensions_light_dom_and_focus() {
    let view = view();
    let parent = container();
    let trigger = document().create_element("button").unwrap();
    trigger.set_text_content(Some("open"));
    parent.append_child(&trigger).unwrap();
    trigger.unchecked_ref::<HtmlElement>().focus().unwrap();
    let base = DialogBase::new(Rc::clone(&view), "min(321px, 90vw)", "123.5px");
    let header = dialog_header(
        &view.document,
        &DialogHeaderProps {
            title: "Named".into(),
            ..DialogHeaderProps::default()
        },
    )
    .unwrap();
    base.open(vec![header.into()], None).unwrap();
    let shown = panel();
    let style = shown.get_attribute("style").unwrap();
    assert!(style.contains("width: min(321px, 90vw)") && style.contains("height: 123.5px"));
    assert!((shown.get_bounding_client_rect().height() - 123.5).abs() < f64::EPSILON);
    assert!(
        js_sys::Reflect::get(&shown, &"shadowRoot".into())
            .unwrap()
            .is_null()
    );
    assert_eq!(shown.get_attribute("role").as_deref(), Some("dialog"));
    assert_eq!(shown.get_attribute("aria-modal").as_deref(), Some("true"));
    let heading = find(&shown, "h2");
    assert_eq!(shown.get_attribute("aria-labelledby"), Some(heading.id()));
    let tree = crate::contributions_support::driver("driverAria", ".maestro-dialog")
        .await
        .as_string()
        .unwrap();
    assert!(tree.contains("dialog \"Named\""), "{tree}");
    assert!(tree.contains("button \"Close\""), "{tree}");
    find(&shown, "button.maestro-dialog-close")
        .unchecked_ref::<HtmlElement>()
        .focus()
        .unwrap();
    base.close();
    assert!(
        document()
            .active_element()
            .unwrap()
            .is_same_node(Some(&trigger))
    );
    let detached = DialogBase::new(Rc::clone(&view), MODAL_WIDTH, "auto");
    detached.open(vec![], None).unwrap();
    trigger.remove();
    detached.close();
    assert!(!trigger.is_connected());
    parent.remove();
}

/// The backdrop dims and blurs the page while the panel uses each pack's own surface.
#[wasm_bindgen]
pub fn dom_dialog_background_dims_while_content_and_focus_stay_clear(import: &js_sys::Function) {
    let view = view();
    let _host = host(&view, import);
    for name in ["brand.json", "signal.json"] {
        let pack = crate::brand::pack(name).unwrap();
        for mode in [BrandMode::Dark, BrandMode::Light] {
            let presentation = pack.presentation(mode).unwrap();
            let base = DialogBase::new(Rc::clone(&view), MODAL_WIDTH, "auto");
            base.open(vec![paragraph("content")], None).unwrap();
            let shown = panel();
            let backdrop = shown.parent_element().unwrap();
            let style = backdrop.unchecked_ref::<HtmlElement>().style();
            for (key, value) in presentation.css_properties() {
                style.set_property(&key, &value).unwrap();
            }
            assert_eq!(
                computed(&backdrop, "background-color"),
                "rgba(0, 0, 0, 0.5)"
            );
            assert_eq!(computed(&backdrop, "backdrop-filter"), "blur(4px)");
            assert_eq!(computed(&backdrop, "position"), "fixed");
            assert_eq!(
                computed(&shown, "background-color"),
                rgb(presentation.colors["background"])
            );
            assert_eq!(
                computed(&shown, "border-top-color"),
                rgb(presentation.colors["border"])
            );
            assert_eq!(
                computed(&find(&shown, "p"), "color"),
                rgb(presentation.colors["text"])
            );
            let close = find(&shown, "button.maestro-dialog-close");
            assert_eq!(computed(&close, "color"), rgb(presentation.colors["muted"]));
            base.close();
        }
    }
    let german = localized_view(&view, "de");
    let base = DialogBase::new(german, MODAL_WIDTH, "auto");
    base.open(vec![], None).unwrap();
    let name = find(&panel(), "button.maestro-dialog-close").get_attribute("aria-label");
    assert_eq!(name.as_deref(), Some("Schließen"));
    base.close();
}

/// Header, content and footer keep caller text literal and children in order.
#[wasm_bindgen]
pub fn dom_dialog_header_content_and_footer_preserve_text() {
    let view = view();
    let document = &view.document;
    for (title, description, paragraphs) in [
        ("<b>雪</b>", Some(" <i>described</i> "), 2),
        ("", Some(""), 1),
        ("plain", None, 1),
    ] {
        let header = dialog_header(
            document,
            &DialogHeaderProps {
                title: title.into(),
                description: description.map(Into::into),
                class_name: "caller".into(),
            },
        )
        .unwrap();
        assert_eq!(header.class_name(), "maestro-dialog-header caller");
        assert_eq!(text(&find(&header, "h2")), title);
        assert_eq!(header.child_element_count(), paragraphs);
        assert_eq!(header.query_selector_all("b, i").unwrap().length(), 0);
        if paragraphs == 2 {
            assert_eq!(text(&find(&header, "p")), " <i>described</i> ");
        }
    }
    let children = [paragraph("one"), paragraph("two")];
    let content = dialog_content(document, &children, "").unwrap();
    assert_eq!(content.class_name(), "maestro-dialog-content");
    assert_eq!(text(&content), "onetwo");
    let footer = dialog_footer(document, &[paragraph("act")], "end").unwrap();
    assert_eq!(footer.class_name(), "maestro-dialog-footer end");
    assert_eq!(text(&footer), "act");
}

/// Recorded input callback names.
type Typed = Rc<RefCell<Vec<&'static str>>>;
/// Operands of a labelled required field whose input callback records `name`.
fn labelled(typed: &Typed, name: &'static str) -> InputProps {
    let seen = Rc::clone(typed);
    InputProps {
        id: "field".into(),
        label: "Name".into(),
        required: true,
        value: "start".into(),
        placeholder: "hint".into(),
        on_input: Some(Rc::new(move |_| seen.borrow_mut().push(name))),
        class_name: "caller".into(),
        ..InputProps::default()
    }
}
/// The field names itself by its label, keeps edits, and calls the current callbacks.
#[wasm_bindgen]
pub async fn dom_input_keeps_edits_label_and_current_callbacks() {
    let view = view();
    let parent = container();
    let typed: Typed = Rc::default();
    let input = Input::new(&view, labelled(&typed, "first")).unwrap();
    parent.append_child(input.root()).unwrap();
    let field = input.input().clone();
    assert_eq!(input.root().class_name(), "maestro-field caller");
    assert_eq!(field.id(), "field");
    assert_eq!(field.value(), "start");
    assert_eq!(field.placeholder(), "hint");
    assert!(field.required());
    let label = find(input.root(), "label");
    assert_eq!(text(&label), "Name*");
    assert_eq!(field.get_attribute("aria-labelledby"), Some(label.id()));
    field.focus().unwrap();
    type_text("!").await;
    assert_eq!(field.value(), "start!");
    assert_eq!(*typed.borrow(), ["first"]);
    edits_survive_updates(&view, &input, &typed).await;
    native_properties(&view, &input);
    parent.remove();
    type_text("x").await;
    assert_eq!(typed.borrow().len(), 2);
}
/// An update with an unchanged value keeps the edit and focus; a changed value replaces it.
async fn edits_survive_updates(view: &DomView, input: &Input, typed: &Typed) {
    let field = input.input();
    let updated = InputProps {
        error: "Wrong".into(),
        ..labelled(typed, "second")
    };
    input.update(view, updated.clone()).unwrap();
    assert_eq!(
        field.value(),
        "start!",
        "an unchanged supplied value keeps the edit"
    );
    assert!(
        view.document
            .active_element()
            .unwrap()
            .is_same_node(Some(field))
    );
    assert_eq!(field.get_attribute("aria-invalid").as_deref(), Some("true"));
    assert_eq!(text(&find(input.root(), ".maestro-input-error")), "Wrong");
    type_text("?").await;
    assert_eq!(*typed.borrow(), ["first", "second"]);
    let replaced = InputProps {
        value: "replaced".into(),
        label: String::new(),
        ..updated
    };
    input.update(view, replaced).unwrap();
    assert_eq!(field.value(), "replaced");
    assert!(input.root().query_selector("label").unwrap().is_none());
    assert!(field.get_attribute("aria-labelledby").is_none());
}
/// Mode, size, flags and numeric bounds reach the native field.
fn native_properties(view: &DomView, input: &Input) {
    let props = InputProps {
        input_type: InputType::Number,
        size: InputSize::Lg,
        disabled: true,
        readonly: true,
        name: "count".into(),
        autocomplete: "off".into(),
        min: Some(0.5),
        max: Some(10.0),
        step: Some(0.25),
        ..InputProps::default()
    };
    input.update(view, props).unwrap();
    let field = input.input();
    assert_eq!(field.type_(), "number");
    assert_eq!(field.class_name(), "maestro-input maestro-input-lg");
    assert!(field.disabled());
    assert!(field.read_only());
    let expected = [
        ("name", "count"),
        ("autocomplete", "off"),
        ("min", "0.5"),
        ("max", "10"),
        ("step", "0.25"),
    ];
    for (name, value) in expected {
        assert_eq!(field.get_attribute(name).as_deref(), Some(value));
    }
    let masked = InputProps {
        input_type: InputType::Password,
        ..InputProps::default()
    };
    input.update(view, masked).unwrap();
    assert_eq!(field.type_(), "password");
    assert!(field.get_attribute("min").is_none());
    assert!(field.get_attribute("aria-invalid").is_none());
}

/// The text a settled prompt answer carries, or null.
fn answer(asked: impl std::future::Future<Output = Option<String>> + 'static) -> js_sys::Promise {
    wasm_bindgen_futures::future_to_promise(async move {
        Ok(asked.await.map_or(wasm_bindgen::JsValue::NULL, Into::into))
    })
}
/// Await a prompt answer.
async fn answered(promise: js_sys::Promise) -> Option<String> {
    wasm_bindgen_futures::JsFuture::from(promise)
        .await
        .unwrap()
        .as_string()
}
/// The field of the open prompt.
fn prompt_field() -> web_sys::HtmlInputElement {
    find(&panel(), "input").unchecked_into()
}
/// Confirm returns the typed text; Cancel, Escape, the backdrop and Close return nothing.
#[wasm_bindgen]
pub async fn prompt_dialog_settles_once_for_every_close_route() {
    let view = view();
    let pending = answer(PromptDialog::ask(&view, "Title", "Message", "seed", true).unwrap());
    let shown = panel();
    let field = prompt_field();
    assert_eq!(field.type_(), "password");
    assert_eq!(field.value(), "seed");
    assert!(
        view.document
            .active_element()
            .unwrap()
            .is_same_node(Some(&field))
    );
    assert_eq!(text(&find(&shown, "h2")), "Title");
    assert_eq!(text(&find(&shown, "p")), "Message");
    let tree = crate::contributions_support::driver("driverAria", ".maestro-dialog-content input")
        .await
        .as_string()
        .unwrap();
    assert!(tree.contains("textbox \"Message\""), "{tree}");
    type_text(" more").await;
    press("Enter").await;
    assert_eq!(answered(pending).await.as_deref(), Some("seed more"));
    assert!(panels().is_empty());
    for route in ["Cancel", "Escape", "backdrop", "Close"] {
        cancelled_by(&view, route).await;
    }
}
/// One close route settles an untitled plain prompt with nothing.
async fn cancelled_by(view: &Rc<DomView>, route: &str) {
    let asked = PromptDialog::ask(view, "", "", "", false).unwrap();
    let shown = panel();
    assert_eq!(text(&find(&shown, "h2")), "Input Required");
    assert_eq!(prompt_field().type_(), "text");
    match route {
        "Cancel" => click(&find(&shown, ".maestro-button-outline")),
        "Escape" => press("Escape").await,
        "backdrop" => click(&shown.parent_element().unwrap()),
        _ => click(&find(&shown, "button.maestro-dialog-close")),
    }
    assert_eq!(asked.await, None, "{route}");
    assert!(panels().is_empty());
}

/// The page's local storage.
fn local_storage() -> web_sys::Storage {
    web_sys::window().unwrap().local_storage().unwrap().unwrap()
}
/// The token asked for with `view`, as a promise of its text or null.
fn token(view: &Rc<DomView>) -> js_sys::Promise {
    let asking = Rc::clone(view);
    wasm_bindgen_futures::future_to_promise(async move {
        let token = get_auth_token(&asking).await.unwrap();
        Ok(token.map_or(wasm_bindgen::JsValue::NULL, Into::into))
    })
}
/// A blank entry asks again, and cancelling settles empty without storing.
async fn cancelled_token(view: &Rc<DomView>, storage: &web_sys::Storage) {
    let pending = token(view);
    settled().await;
    assert_eq!(text(&find(&panel(), "h2")), "Enter Auth Token");
    assert_eq!(prompt_field().type_(), "password");
    type_text("   ").await;
    press("Enter").await;
    assert_eq!(panels().len(), 1, "a blank entry asks again");
    press("Escape").await;
    assert_eq!(answered(pending).await, None);
    assert!(storage.get_item("auth-token").unwrap().is_none());
}
/// A stored token is returned at once; prompting stores the trimmed token or settles empty.
#[wasm_bindgen]
pub async fn auth_token_prompts_until_entered_or_cancelled() {
    let view = view();
    let storage = local_storage();
    storage.set_item("auth-token", " stored ").unwrap();
    assert_eq!(
        get_auth_token(&view).await.unwrap().as_deref(),
        Some(" stored ")
    );
    assert!(panels().is_empty());
    clear_auth_token(&view).unwrap();
    assert!(storage.get_item("auth-token").unwrap().is_none());
    cancelled_token(&view, &storage).await;
    let pending = token(&view);
    settled().await;
    type_text(" tok ").await;
    press("Enter").await;
    assert_eq!(answered(pending).await.as_deref(), Some("tok"));
    assert_eq!(
        storage.get_item("auth-token").unwrap().as_deref(),
        Some("tok")
    );
    clear_auth_token(&view).unwrap();
}
