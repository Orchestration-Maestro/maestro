//! Settings tabs, the saved-session list and the storage permission request in a real page.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    contributions_support::{computed, container, host, page_module, rgb, text, view},
    dialog_support::{Prompts, panel, panels, press, settled, storage, type_text, until},
};
use maestro_app::{SessionData, SessionMetadata};
use maestro_theme::BrandMode;
use maestro_web::{
    ApiKeysTab, PersistentStorageDialog, ProvidersModelsTab, ProxyTab, SessionListDialog,
    SettingsDialog, SettingsTab,
    dom::{DomContent, DomView},
};
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure, prelude::wasm_bindgen};
use web_sys::{Element, HtmlButtonElement, HtmlElement, HtmlInputElement};

/// One required descendant.
fn find(root: &Element, selector: &str) -> Element {
    root.query_selector(selector)
        .unwrap()
        .unwrap_or_else(|| panic!("missing {selector}"))
}
/// Every matching descendant in order.
fn all(root: &Element, selector: &str) -> Vec<Element> {
    let nodes = root.query_selector_all(selector).unwrap();
    (0..nodes.length())
        .map(|index| nodes.item(index).unwrap().unchecked_into())
        .collect()
}
/// Click `element` as the page would.
fn click(element: &Element) {
    element.unchecked_ref::<HtmlElement>().click();
}
/// The built-in settings tabs, as a caller supplies them.
fn tabs() -> Vec<Rc<dyn SettingsTab>> {
    vec![
        Rc::new(ApiKeysTab),
        Rc::new(ProxyTab),
        Rc::new(ProvidersModelsTab),
    ]
}

/// Tab names follow caller order; edits survive switching between sidebar and phone tabs.
#[wasm_bindgen]
pub async fn settings_dialog_keeps_tab_contents_across_sidebar_and_phone_tabs(
    import: js_sys::Function,
) {
    let view = view();
    let _host = host(&view, &import);
    let _app = storage("settings-tabs");
    assert!(SettingsDialog::open(&view, &[], None).unwrap().is_none());
    assert!(panels().is_empty(), "no tabs open no dialog");
    let closed = Rc::new(Cell::new(0));
    let count = Rc::clone(&closed);
    let dialog = SettingsDialog::open(
        &view,
        &tabs(),
        Some(Rc::new(move || count.set(count.get() + 1))),
    )
    .unwrap()
    .unwrap();
    let shown = panel();
    assert_eq!(text(&find(&shown, "h2")), "Settings");
    let names = ["API Keys", "Proxy", "Providers & Models"];
    let sidebar = all(&shown, ".maestro-settings-item");
    let phone = all(&shown, ".maestro-settings-tab");
    assert_eq!(
        sidebar.iter().map(|item| text(item)).collect::<Vec<_>>(),
        names
    );
    assert_eq!(
        phone.iter().map(|item| text(item)).collect::<Vec<_>>(),
        names
    );
    assert_eq!(
        computed(&find(&shown, ".maestro-settings-tabs"), "display"),
        "none"
    );
    assert_eq!(
        computed(&find(&shown, ".maestro-settings-sidebar"), "display"),
        "block"
    );
    switched(&shown, &sidebar, &phone).await;
    press("Escape").await;
    assert_eq!(
        (closed.get(), dialog.is_open(), panels().len()),
        (1, false, 0)
    );
    press("Escape").await;
    assert_eq!(closed.get(), 1);
}
/// Switching tabs from either control set keeps the hidden tab's edit.
async fn switched(shown: &Element, sidebar: &[Element], phone: &[Element]) {
    let slots = all(shown, ".maestro-settings-content > div");
    assert_eq!(visible(&slots), [true, false, false]);
    let field: HtmlInputElement = find(&slots[0], "input").unchecked_into();
    field.focus().unwrap();
    type_text("kept").await;
    click(&sidebar[1]);
    assert_eq!(visible(&slots), [false, true, false]);
    assert_eq!(
        sidebar[1].get_attribute("aria-current").as_deref(),
        Some("page")
    );
    click(&phone[0]);
    assert_eq!(visible(&slots), [true, false, false]);
    assert!(
        field.is_connected() && field.value() == "kept",
        "the hidden tab kept its edit"
    );
}
/// Which content slots are shown.
fn visible(slots: &[Element]) -> Vec<bool> {
    slots
        .iter()
        .map(|slot| computed(slot, "display") != "none")
        .collect()
}

/// Wait for a stored value through storage reads alone.
async fn stored<T: serde::de::DeserializeOwned + PartialEq>(
    app: &maestro_app::AppStorage,
    key: &str,
    expected: &T,
) {
    while app.settings.get::<T>(key).await.unwrap().as_ref() != Some(expected) {}
}
/// The proxy switch and address field of a mounted proxy tab.
fn proxy_controls(root: &Element) -> (HtmlButtonElement, HtmlInputElement) {
    (
        find(root, "[role=switch]").unchecked_into(),
        find(root, "input").unchecked_into(),
    )
}
/// Defaults show until switched; switching saves both values and the address saves on change.
#[wasm_bindgen]
pub async fn proxy_tab_loads_defaults_and_saves_switch_and_address(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let app = storage("proxy-tab");
    assert_eq!(ProxyTab.get_tab_name(&view).unwrap(), "Proxy");
    let parent = container();
    let root = ProxyTab.mount(&view).unwrap();
    parent.append_child(&root.node).unwrap();
    let element: Element = root.node.clone().unchecked_into();
    let (switch, field) = proxy_controls(&element);
    assert_eq!(
        (
            switch.get_attribute("aria-checked"),
            field.value(),
            field.disabled()
        ),
        (Some("false".into()), "http://localhost:3001".into(), true)
    );
    let tree = crate::contributions_support::driver("driverAria", ".maestro-settings-proxy")
        .await
        .as_string()
        .unwrap();
    assert!(tree.contains("switch \"Use CORS Proxy\""), "{tree}");
    switch.click();
    assert_eq!(
        (switch.get_attribute("aria-checked"), field.disabled()),
        (Some("true".into()), false)
    );
    stored(&app, "proxy.enabled", &true).await;
    stored(&app, "proxy.url", &"http://localhost:3001".to_owned()).await;
    field.focus().unwrap();
    type_text("/x").await;
    assert_eq!(
        app.settings
            .get::<String>("proxy.url")
            .await
            .unwrap()
            .as_deref(),
        Some("http://localhost:3001")
    );
    press("Tab").await;
    stored(&app, "proxy.url", &"http://localhost:3001/x".to_owned()).await;
    parent.remove();
    loaded_values(&view, &page).await;
}
/// Stored values replace the defaults when the tab appears.
async fn loaded_values(view: &DomView, page: &JsValue) {
    let app = storage("proxy-loaded");
    app.settings.set("proxy.enabled", &true).await.unwrap();
    app.settings
        .set("proxy.url", &"https://p.test")
        .await
        .unwrap();
    let parent = container();
    let root = ProxyTab.mount(view).unwrap();
    parent.append_child(&root.node).unwrap();
    let element: Element = root.node.clone().unchecked_into();
    let (switch, field) = proxy_controls(&element);
    until(page, || {
        switch.get_attribute("aria-checked").as_deref() == Some("true")
    })
    .await;
    assert_eq!(
        (field.value(), field.disabled()),
        ("https://p.test".into(), false)
    );
    parent.remove();
}

/// A stored session with `id`, `title`, modification time and message count.
fn session(id: &str, title: &str, modified: &str, count: f64) -> (SessionData, SessionMetadata) {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../maestro-app/tests/fixtures/browser_sessions.json"
    ))
    .unwrap();
    let row = fixture["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| {
            row["test"] == "session_records_roundtrip_standard_browser_and_open_custom_history"
                && row["input"]["data"]["messages"][0]["role"] == "user-with-attachments"
        })
        .unwrap();
    let mut data = row["input"]["data"].clone();
    data["id"] = id.into();
    data["title"] = title.into();
    data["lastModified"] = modified.into();
    let mut metadata = row["input"]["metadata"].clone();
    metadata["id"] = id.into();
    metadata["title"] = title.into();
    metadata["lastModified"] = modified.into();
    metadata["messageCount"] = count.into();
    (
        serde_json::from_value(data).unwrap(),
        serde_json::from_value(metadata).unwrap(),
    )
}
/// An ISO time `days` days before now.
fn days_ago(days: f64) -> String {
    let time = js_sys::Date::now() - days * 86_400_000.0 - 60_000.0;
    js_sys::Date::new(&time.into()).to_iso_string().into()
}
/// Store sessions modified 0, 1, 3 and 10 days ago.
async fn four_sessions(app: &maestro_app::AppStorage) {
    for (id, days) in [
        ("today", 0.0),
        ("yesterday", 1.0),
        ("three", 3.0),
        ("old", 10.0),
    ] {
        let (data, metadata) = session(id, &format!("Title {id}"), &days_ago(days), 4.0);
        app.sessions.save(&data, &metadata).await.unwrap();
    }
}
/// Recorded selections and deletions.
type Calls = Rc<RefCell<Vec<String>>>;
/// Open the list recording selections as `select:` and deletions as `delete:`.
fn open_list(view: &Rc<DomView>, calls: &Calls) -> SessionListDialog {
    let (selected, deleted) = (Rc::clone(calls), Rc::clone(calls));
    SessionListDialog::open(
        view,
        Rc::new(move |id| selected.borrow_mut().push(format!("select:{id}"))),
        Some(Rc::new(move |id| {
            deleted.borrow_mut().push(format!("delete:{id}"));
        })),
    )
    .unwrap()
}
/// Rows show storage order, dates, counts and usage; choosing one returns its identifier.
#[wasm_bindgen]
pub async fn session_list_orders_dates_and_selects_sessions(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let app = storage("session-list");
    let calls: Calls = Rc::default();
    let empty = open_list(&view, &calls);
    let shown = panel();
    assert_eq!(text(&find(&shown, ".maestro-session-notice")), "Loading...");
    until(&page, || text(&shown).contains("No sessions yet")).await;
    empty.close();
    four_sessions(&app).await;
    let list = open_list(&view, &calls);
    let shown = panel();
    assert_eq!(text(&find(&shown, "h2")), "Sessions");
    until(&page, || all(&shown, ".maestro-session-row").len() == 4).await;
    let expected: Vec<String> = app
        .sessions
        .get_all_metadata()
        .await
        .unwrap()
        .into_iter()
        .map(|meta| meta.title)
        .collect();
    let titles: Vec<String> = all(&shown, ".maestro-session-title")
        .iter()
        .map(|title| text(title))
        .collect();
    assert_eq!(titles, expected, "rows keep the stored order");
    dated(&shown, &titles);
    assert!(text(&all(&shown, ".maestro-session-meta")[1]).starts_with("4 messages · "));
    keyboard_choice(&view, &all(&shown, ".maestro-session-row")[0]).await;
    assert_eq!(
        *calls.borrow(),
        [format!(
            "select:{}",
            expected[0].trim_start_matches("Title ")
        )]
    );
    assert!(!list.is_open() && panels().is_empty());
}

/// Each row titled after its age reads Today, Yesterday, a number of days or the local date.
fn dated(shown: &Element, titles: &[String]) {
    let dates: Vec<String> = all(shown, ".maestro-session-meta")
        .iter()
        .step_by(2)
        .map(|date| text(date))
        .collect();
    let old = js_sys::Date::new(&days_ago(10.0).into());
    let local: String = old
        .to_locale_date_string("default", &JsValue::UNDEFINED)
        .into();
    for (title, date) in titles.iter().zip(&dates) {
        let want = match title.as_str() {
            "Title today" => "Today",
            "Title yesterday" => "Yesterday",
            "Title three" => "3 days ago",
            _ => local.as_str(),
        };
        assert_eq!(date, want, "{title}");
    }
}
/// Tab reaches the first row's own control before its delete control, and Enter chooses it.
async fn keyboard_choice(view: &DomView, row: &Element) {
    let focused = || view.document.active_element();
    while !focused().is_some_and(|node| row.contains(Some(&node))) {
        press("Tab").await;
    }
    let control = focused().unwrap();
    assert_eq!(
        (control.tag_name(), control.class_name()),
        ("BUTTON".into(), "maestro-session-details".into()),
        "the row opens from the keyboard"
    );
    press("Enter").await;
}
/// Confirmed deletion removes the session, and only a close without a choice reports it.
#[wasm_bindgen]
pub async fn session_list_deletes_after_confirmation_and_reports_on_close(
    import: js_sys::Function,
) {
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let app = storage("session-delete");
    four_sessions(&app).await;
    let prompts = Prompts::install(vec![false, true, true]);
    let calls: Calls = Rc::default();
    let _list = open_list(&view, &calls);
    let shown = panel();
    until(&page, || all(&shown, ".maestro-session-row").len() == 4).await;
    let first = all(&shown, ".maestro-session-row").remove(0);
    let id = text(&find(&first, ".maestro-session-title"))
        .trim_start_matches("Title ")
        .to_owned();
    click(&find(&first, ".maestro-session-delete"));
    assert!(
        calls.borrow().is_empty(),
        "the delete control never chooses the row"
    );
    assert_eq!(prompts.messages(), ["?Delete this session?"]);
    assert_eq!(all(&shown, ".maestro-session-row").len(), 4);
    click(&find(&first, ".maestro-session-delete"));
    until(&page, || all(&shown, ".maestro-session-row").len() == 3).await;
    assert!(app.sessions.get_metadata(&id).await.unwrap().is_none());
    let history = app
        .sessions
        .get_backend()
        .unwrap()
        .get("sessions", &id)
        .await
        .unwrap();
    assert!(
        history.is_none(),
        "the history row is deleted with its metadata"
    );
    press("Escape").await;
    assert_eq!(*calls.borrow(), [format!("delete:{id}")]);
    calls.borrow_mut().clear();
    chosen_after_deleting(&view, &page, &calls).await;
}
/// Closing during deletion still reports a committed deletion exactly once.
#[wasm_bindgen]
pub async fn session_list_reports_deletion_when_closed_during_write(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let app = storage("session-close-during-delete");
    let (data, metadata) = session("removed", "Removed", &days_ago(0.0), 1.0);
    app.sessions.save(&data, &metadata).await.unwrap();
    let _prompts = Prompts::install(vec![true]);
    let calls: Calls = Rc::default();
    let list = open_list(&view, &calls);
    let shown = panel();
    until(&page, || all(&shown, ".maestro-session-row").len() == 1).await;
    click(&find(&shown, ".maestro-session-delete"));
    list.close();
    list.close();
    drop(list);
    assert!(panels().is_empty());
    while app
        .sessions
        .get_metadata("removed")
        .await
        .unwrap()
        .is_some()
    {}
    settled().await;
    assert_eq!(*calls.borrow(), ["delete:removed"]);
}

/// A choice after a deletion reports only the choice.
async fn chosen_after_deleting(view: &Rc<DomView>, page: &JsValue, calls: &Calls) {
    let _again = open_list(view, calls);
    let shown = panel();
    until(page, || all(&shown, ".maestro-session-row").len() == 3).await;
    click(&find(
        &all(&shown, ".maestro-session-row")[0],
        ".maestro-session-delete",
    ));
    until(page, || all(&shown, ".maestro-session-row").len() == 2).await;
    click(&all(&shown, ".maestro-session-row")[0]);
    assert_eq!(
        calls.borrow().len(),
        1,
        "a choice suppresses the deletion report"
    );
    assert!(calls.borrow()[0].starts_with("select:"));
}

/// A replaced navigator storage operation; the original is restored on drop.
struct Platform {
    /// Replaced property name and its original value.
    original: (String, JsValue),
    /// Installed replacement, when one is a function.
    _replacement: Option<Closure<dyn Fn() -> js_sys::Promise>>,
}
impl Platform {
    /// Replace `navigator.storage[name]` with an operation settling to `outcome`, or remove it.
    fn install(name: &str, outcome: Option<Result<bool, &'static str>>) -> Self {
        let manager =
            js_sys::Reflect::get(&web_sys::window().unwrap().navigator(), &"storage".into())
                .unwrap();
        let original = js_sys::Reflect::get(&manager, &name.into()).unwrap();
        let replacement = outcome.map(|outcome| {
            Closure::<dyn Fn() -> js_sys::Promise>::new(move || match outcome {
                Ok(value) => js_sys::Promise::resolve(&JsValue::from(value)),
                Err(message) => js_sys::Promise::reject(&JsValue::from(message)),
            })
        });
        let value = replacement
            .as_ref()
            .map_or(JsValue::UNDEFINED, |closure| closure.as_ref().clone());
        js_sys::Reflect::set(&manager, &name.into(), &value).unwrap();
        Self {
            original: (name.to_owned(), original),
            _replacement: replacement,
        }
    }
}
impl Drop for Platform {
    fn drop(&mut self) {
        let manager =
            js_sys::Reflect::get(&web_sys::window().unwrap().navigator(), &"storage".into())
                .unwrap();
        js_sys::Reflect::set(&manager, &self.original.0.as_str().into(), &self.original.1).unwrap();
    }
}
/// Request persistence, answering the dialog with `answer` when it appears.
async fn request(view: &Rc<DomView>, answer: &str) -> bool {
    let asking = Rc::clone(view);
    let pending = wasm_bindgen_futures::future_to_promise(async move {
        Ok(PersistentStorageDialog::request(&asking)
            .await
            .unwrap()
            .into())
    });
    settled().await;
    let shown = panel();
    match answer {
        "Grant" => click(&all(&shown, ".maestro-storage-actions button")[1]),
        "Continue" => click(&all(&shown, ".maestro-storage-actions button")[0]),
        _ => press("Escape").await,
    }
    wasm_bindgen_futures::JsFuture::from(pending)
        .await
        .unwrap()
        .as_bool()
        .unwrap()
}
/// Granted storage short-circuits; refusal, closing, absence, denial and failure all return false.
#[wasm_bindgen]
pub async fn persistent_storage_asks_then_reports_each_outcome() {
    let view = view();
    let _app = storage("persistence");
    {
        let _persisted = Platform::install("persisted", Some(Ok(true)));
        assert!(PersistentStorageDialog::request(&view).await.unwrap());
        assert!(
            panels().is_empty(),
            "already persisted storage asks nothing"
        );
    }
    let _persisted = Platform::install("persisted", Some(Ok(false)));
    let shown_texts = {
        let asking = Rc::clone(&view);
        let pending = wasm_bindgen_futures::future_to_promise(async move {
            Ok(PersistentStorageDialog::request(&asking)
                .await
                .unwrap()
                .into())
        });
        settled().await;
        let shown = panel();
        let texts = (text(&find(&shown, "h2")), all(&shown, "li").len());
        click(&all(&shown, ".maestro-storage-actions button")[0]);
        assert!(
            !wasm_bindgen_futures::JsFuture::from(pending)
                .await
                .unwrap()
                .as_bool()
                .unwrap()
        );
        texts
    };
    assert_eq!(shown_texts, ("Storage Permission Required".into(), 4));
    assert!(!request(&view, "Escape").await, "closing refuses");
    for (outcome, expected) in [
        (Some(Ok(true)), true),
        (Some(Ok(false)), false),
        (Some(Err("failed")), false),
        (None, false),
    ] {
        let _persist = Platform::install("persist", outcome);
        assert_eq!(request(&view, "Grant").await, expected, "{outcome:?}");
        assert!(panels().is_empty());
    }
}

/// Active settings controls, session rows and the permission notice use each pack's roles.
#[wasm_bindgen]
pub async fn settings_dialogs_follow_both_brand_packs(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let app = storage("settings-brand");
    four_sessions(&app).await;
    for name in ["brand.json", "signal.json"] {
        let pack = crate::brand::pack(name).unwrap();
        for mode in [BrandMode::Dark, BrandMode::Light] {
            let presentation = pack.presentation(mode).unwrap();
            let dialog = SettingsDialog::open(&view, &tabs(), None).unwrap().unwrap();
            let shown = panel();
            apply(&shown, &presentation);
            let active = find(&shown, ".maestro-settings-item.maestro-settings-active");
            assert_eq!(
                computed(&active, "background-color"),
                rgb(presentation.colors["panel"])
            );
            assert_eq!(computed(&active, "color"), rgb(presentation.colors["text"]));
            let idle = all(&shown, ".maestro-settings-item")[1].clone();
            assert_eq!(computed(&idle, "color"), rgb(presentation.colors["muted"]));
            cues(&shown, &active, &presentation).await;
            dialog.close();
            let list = open_list(&view, &Rc::default());
            let shown = panel();
            apply(&shown, &presentation);
            until(&page, || !all(&shown, ".maestro-session-row").is_empty()).await;
            let row = all(&shown, ".maestro-session-row").remove(0);
            assert_eq!(
                computed(&row, "border-top-color"),
                rgb(presentation.colors["border"])
            );
            assert_eq!(
                computed(&find(&row, ".maestro-session-title"), "color"),
                rgb(presentation.colors["text"])
            );
            assert_eq!(
                computed(&find(&row, ".maestro-session-delete"), "color"),
                rgb(presentation.colors["error"])
            );
            list.close();
            notice(&view, &presentation).await;
        }
    }
}
/// The active tab, the Close control and the idle proxy switch stay visible where panel and
/// canvas share a swatch, without dimming muted text.
async fn cues(
    shown: &Element,
    active: &Element,
    presentation: &maestro_theme::BrandPresentation<'_>,
) {
    let [accent, border, muted] =
        ["accent", "border", "muted"].map(|role| rgb(presentation.colors[role]));
    assert!(
        computed(active, "box-shadow").contains(&accent),
        "the active tab keeps an accent cue"
    );
    let close = find(shown, ".maestro-dialog-close");
    assert_eq!(
        (computed(&close, "color"), computed(&close, "opacity")),
        (muted, "1".into())
    );
    click(&all(shown, ".maestro-settings-item")[1]);
    settled().await;
    let switch = find(shown, ".maestro-switch");
    assert_eq!(
        switch.get_attribute("aria-checked").as_deref(),
        Some("false")
    );
    assert_eq!(computed(&switch, "border-top-color"), border);
    assert!(
        computed(&find(&switch, ".maestro-switch-thumb"), "box-shadow").contains(&border),
        "the idle thumb keeps a border cue"
    );
}
/// The permission notice is a panel bounded by the warning role.
async fn notice(view: &Rc<DomView>, presentation: &maestro_theme::BrandPresentation<'_>) {
    let _persisted = Platform::install("persisted", Some(Ok(false)));
    let asking = Rc::clone(view);
    let pending = wasm_bindgen_futures::future_to_promise(async move {
        Ok(PersistentStorageDialog::request(&asking)
            .await
            .unwrap()
            .into())
    });
    settled().await;
    let shown = panel();
    apply(&shown, presentation);
    let framed = find(&shown, ".maestro-storage-notice");
    assert_eq!(
        (
            computed(&framed, "background-color"),
            computed(&framed, "border-top-color")
        ),
        (
            rgb(presentation.colors["panel"]),
            rgb(presentation.colors["warning"])
        )
    );
    press("Escape").await;
    wasm_bindgen_futures::JsFuture::from(pending).await.unwrap();
}
/// Apply a pack's properties to an open dialog's backdrop.
fn apply(panel: &Element, presentation: &maestro_theme::BrandPresentation<'_>) {
    let style = panel
        .parent_element()
        .unwrap()
        .unchecked_into::<HtmlElement>()
        .style();
    for (key, value) in presentation.css_properties() {
        style.set_property(&key, &value).unwrap();
    }
}
