//! Key entry, the model picker and the provider dialogs against controlled storage and network.
#![cfg(all(test, target_arch = "wasm32"))]
use crate::{
    contributions_support::{computed, container, host, page_module, rgb, text, view},
    dialog_support::{
        Fetch, Prompts, completed_reply, panel, panels, press, settled, storage, type_text, until,
    },
};
use maestro_app::{CustomProvider, Model, Presence, get_model};
use maestro_web::{
    ApiKeyPromptDialog, CustomProviderDialog, ModelSelector, ProviderKeyInput, ProvidersModelsTab,
    SettingsTab,
    dom::{DomContent, DomView, MountedContent},
};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
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
/// The key entry mounted in a new container.
fn key_entry(view: &DomView, provider: &str) -> (Element, maestro_web::dom::DomRoot) {
    let parent = container();
    let root = ProviderKeyInput::new(provider).mount(view).unwrap();
    parent.append_child(&root.node).unwrap();
    (root.node.clone().unchecked_into(), root)
}
/// A manual provider offering `models`.
fn manual(id: &str, name: &str, models: Vec<Model>) -> CustomProvider {
    CustomProvider {
        id: id.into(),
        name: name.into(),
        r#type: serde_json::from_value("openai-completions".into()).unwrap(),
        base_url: "http://manual.test".into(),
        api_key: Presence::Missing,
        models: Presence::Present(models),
    }
}
/// A local descriptor named `id` under `provider`, optionally reasoning.
fn local(provider: &str, id: &str, reasoning: bool) -> Model {
    serde_json::from_value(serde_json::json!({
        "id": id, "name": id, "api": "openai-completions", "provider": provider,
        "baseUrl": "http://manual.test", "reasoning": reasoning, "input": ["text"],
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0},
        "contextWindow": 32000, "maxTokens": 2048
    }))
    .unwrap()
}
/// A server listing `count` models named `m0`, `m1` and so on.
fn listing(count: usize) -> String {
    let rows: Vec<_> = (0..count)
        .map(|index| serde_json::json!({"id": format!("m{index}")}))
        .collect();
    serde_json::json!({ "data": rows }).to_string()
}

/// Whether `root` has a descendant matching `selector`.
fn has(root: &Element, selector: &str) -> bool {
    root.query_selector(selector).unwrap().is_some()
}
/// The masked field and Save control of a key entry.
fn key_controls(root: &Element) -> (HtmlInputElement, HtmlButtonElement) {
    (
        find(root, "input").unchecked_into(),
        find(root, "button").unchecked_into(),
    )
}

/// A key is tested with the stored proxy settings and stored only after a normal stop.
#[wasm_bindgen]
pub async fn provider_key_input_tests_with_current_proxy_before_saving(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let app = storage("key-proxy");
    app.settings.set("proxy.enabled", &true).await.unwrap();
    app.settings
        .set("proxy.url", &"https://proxy.test")
        .await
        .unwrap();
    let fetch = Fetch::install(vec![(200, "text/event-stream", completed_reply())]);
    let (root, _mounted) = key_entry(&view, "anthropic");
    let (field, save) = key_controls(&root);
    assert_eq!(
        (
            text(&find(&root, ".maestro-key-provider")),
            field.type_(),
            field.placeholder(),
            save.disabled()
        ),
        (
            "anthropic".into(),
            "password".into(),
            "Enter API key".into(),
            true
        ),
        "an empty masked entry cannot be saved"
    );
    field.focus().unwrap();
    let tree = crate::contributions_support::driver("driverAria", ".maestro-key-input input")
        .await
        .as_string()
        .unwrap();
    assert!(tree.contains("textbox \"anthropic\""), "{tree}");
    type_text("sk-ant-oat-1").await;
    assert!(!save.disabled());
    save.click();
    assert_eq!(text(&find(&root, ".maestro-badge-secondary")), "Testing...");
    assert!(save.disabled(), "testing blocks another save");
    until(&page, || has(&root, ".maestro-key-stored")).await;
    proxied_request(&fetch.requests());
    let stored = app.provider_keys.get("anthropic").await.unwrap();
    assert_eq!(
        (stored.as_deref(), save.disabled(), field.placeholder()),
        (Some("sk-ant-oat-1"), true, "••••••••••••".into()),
        "a saved key is unchanged until edited"
    );
    drop(fetch);
    direct_after_disabling(&page, &app, &root).await;
}
/// After the proxy is switched off, an edited key is tested directly.
async fn direct_after_disabling(page: &JsValue, app: &maestro_app::AppStorage, root: &Element) {
    let (_, save) = key_controls(root);
    type_text("2").await;
    assert!(!save.disabled());
    app.settings.set("proxy.enabled", &false).await.unwrap();
    let fetch = Fetch::install(vec![(200, "text/event-stream", completed_reply())]);
    save.click();
    until(page, || {
        fetch.requests().len() == 1 && save.disabled() && !has(root, ".maestro-badge")
    })
    .await;
    assert!(
        fetch.requests()[0]
            .url
            .starts_with("https://api.anthropic.com"),
        "disabled proxy is not used"
    );
}
/// The one test request went through the proxy with a short completion.
fn proxied_request(requests: &[crate::dialog_support::Request]) {
    assert_eq!(requests.len(), 1);
    let url = &requests[0].url;
    assert!(
        url.starts_with("https://proxy.test/?url=https%3A%2F%2Fapi.anthropic.com"),
        "{url}"
    );
    let body: serde_json::Value = serde_json::from_str(&requests[0].body).unwrap();
    assert_eq!(body["max_tokens"], 200);
    assert!(requests[0].body.contains("Reply with: ok"));
}

/// A refused key shows the invalid badge until it expires; an unmapped provider is not tested.
#[wasm_bindgen]
pub async fn provider_key_input_reports_failures_and_skips_unmapped_providers(
    import: js_sys::Function,
) {
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let app = storage("key-failures");
    let refusal =
        r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"}}"#;
    let fetch = Fetch::install(vec![(401, "application/json", refusal.into())]);
    let (root, _mounted) = key_entry(&view, "anthropic");
    let (field, save) = key_controls(&root);
    field.focus().unwrap();
    type_text("bad").await;
    save.click();
    until(&page, || has(&root, ".maestro-badge-destructive")).await;
    assert_eq!(
        text(&find(&root, ".maestro-badge-destructive")),
        "✗ Invalid"
    );
    assert!(app.provider_keys.get("anthropic").await.unwrap().is_none());
    assert_eq!(fetch.requests().len(), 1);
    until(&page, || !has(&root, ".maestro-badge-destructive")).await;
    assert_eq!(field.value(), "bad", "the refused key stays editable");
    unmapped_and_stored(&view, &page, &app).await;
    assert_eq!(
        fetch.requests().len(),
        1,
        "no request tests an unmapped provider"
    );
}
/// An unmapped provider's key is stored untested, and a stored key shows on mounting.
async fn unmapped_and_stored(view: &DomView, page: &JsValue, app: &maestro_app::AppStorage) {
    let (other, _second) = key_entry(view, "my-local");
    let (field, save) = key_controls(&other);
    field.focus().unwrap();
    type_text("anything").await;
    save.click();
    until(page, || has(&other, ".maestro-key-stored")).await;
    assert_eq!(
        app.provider_keys.get("my-local").await.unwrap().as_deref(),
        Some("anything")
    );
    app.provider_keys.set("openai", "stored").await.unwrap();
    let (stored, _third) = key_entry(view, "openai");
    until(page, || has(&stored, ".maestro-key-stored")).await;
    assert!(key_controls(&stored).1.disabled());
}

/// Updating the provider replaces the entry rather than carrying its credential to another provider.
#[wasm_bindgen]
pub async fn provider_key_input_keeps_credentials_with_their_provider(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    let _app = storage("key-identity");
    let parent = container();
    let mut mounted = MountedContent::default();
    mounted
        .show(&ProviderKeyInput::new("first"), &parent, &view)
        .unwrap();
    let first: Element = mounted.root().unwrap().clone().unchecked_into();
    let (field, _) = key_controls(&first);
    field.focus().unwrap();
    type_text("first-secret").await;
    mounted
        .show(&ProviderKeyInput::new("second"), &parent, &view)
        .unwrap();
    let second: Element = mounted.root().unwrap().clone().unchecked_into();
    let (field, save) = key_controls(&second);
    assert!(
        !first.is_connected(),
        "a different provider owns a different entry"
    );
    assert_eq!(
        field.value(),
        "",
        "another provider never inherits the credential"
    );
    assert!(save.disabled());
    assert_eq!(text(&find(&second, ".maestro-key-provider")), "second");
    parent.remove();
}

/// An edit made during a save stays unsaved and can be tested on the next activation.
#[wasm_bindgen]
pub async fn provider_key_input_keeps_edits_made_while_saving(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let app = storage("key-edited-during-save");
    let (root, _mounted) = key_entry(&view, "local");
    let (field, save) = key_controls(&root);
    field.focus().unwrap();
    type_text("submitted").await;
    save.click();
    field.set_value("later-edit");
    field
        .dispatch_event(&web_sys::Event::new("input").unwrap())
        .unwrap();
    until(&page, || has(&root, ".maestro-key-stored")).await;
    assert_eq!(
        app.provider_keys.get("local").await.unwrap().as_deref(),
        Some("submitted")
    );
    assert_eq!(field.value(), "later-edit");
    assert!(
        !save.disabled(),
        "saving the old value must not acknowledge the later edit"
    );
    save.click();
    while app.provider_keys.get("local").await.unwrap().as_deref() != Some("later-edit") {}
    until(&page, || {
        save.disabled() && !has(&root, ".maestro-badge-secondary")
    })
    .await;
}

/// Rows of the open picker.
fn rows(shown: &Element) -> Vec<Element> {
    all(shown, "[data-model-item]")
}
/// Whether the row at `index` is highlighted.
fn highlighted(shown: &Element, index: usize) -> bool {
    rows(shown)[index]
        .class_list()
        .contains("maestro-model-selected")
}
/// One stored manual provider offering a plain and a reasoning model.
async fn seed_models(app: &maestro_app::AppStorage) {
    let models = vec![
        local("Local Box", "local-model", false),
        local("Local Box", "local-thinker", true),
    ];
    app.custom_providers
        .set(&manual("box", "Local Box", models))
        .await
        .unwrap();
}
/// Search ranks rows, filters intersect, arrows stay within the rows and Enter chooses.
#[wasm_bindgen]
pub async fn model_selector_searches_filters_and_selects_by_keyboard(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    seed_models(&storage("model-selector")).await;
    let current = get_model("anthropic", "claude-haiku-4-5").unwrap();
    let chosen: Rc<RefCell<Vec<Model>>> = Rc::default();
    let seen = Rc::clone(&chosen);
    let on_select = Rc::new(move |model| seen.borrow_mut().push(model));
    let selector = ModelSelector::open(&view, Some(current.clone()), on_select, None).unwrap();
    let shown = panel();
    let search: HtmlInputElement = find(&shown, "input").unchecked_into();
    let focused = view
        .document
        .active_element()
        .unwrap()
        .is_same_node(Some(&search));
    assert_eq!(
        (focused, search.placeholder()),
        (true, "Search models...".into())
    );
    until(&page, || text(&shown).contains("local-thinker")).await;
    let first = rows(&shown).remove(0);
    let current_first = (
        has(&first, ".maestro-model-current"),
        text(&find(&first, ".maestro-model-id")),
    );
    assert_eq!(
        current_first,
        (true, current.id.clone()),
        "the current model leads without search"
    );
    assert!(highlighted(&shown, 0));
    separated(&view, &shown);
    searched(&shown, &search).await;
    no_results(&shown, &chosen).await;
    let thinking = all(&shown, ".maestro-model-filters button").remove(0);
    click(&thinking);
    assert_eq!(
        thinking.get_attribute("aria-pressed").as_deref(),
        Some("true")
    );
    search.focus().unwrap();
    press("Enter").await;
    let ids: Vec<_> = chosen
        .borrow()
        .iter()
        .map(|model| model.id.clone())
        .collect();
    assert_eq!(
        (ids, selector.is_open(), panels().len()),
        (vec!["local-thinker".to_owned()], false, 0)
    );
    restricted(&view, &page).await;
}
/// The highlighted row carries an accent cue and an idle filter a border, since a panel fill alone
/// disappears where panel and canvas share a swatch.
fn separated(view: &DomView, shown: &Element) {
    let presentation = view.pack.presentation(view.mode).unwrap();
    let [accent, border] = ["accent", "border"].map(|role| rgb(presentation.colors[role]));
    assert!(
        computed(&rows(shown)[0], "box-shadow").contains(&accent),
        "the highlighted row keeps an accent cue"
    );
    let filter = all(shown, ".maestro-model-filters button").remove(0);
    assert!(
        computed(&filter, "box-shadow").contains(&border),
        "the idle filter keeps a border ring"
    );
}
/// Typed search ranks the tight match first, and the arrows stop at the first and last rows.
async fn searched(shown: &Element, search: &HtmlInputElement) {
    type_text("localthinker").await;
    let first = rows(shown).remove(0);
    assert_eq!(text(&find(&first, ".maestro-model-id")), "local-thinker");
    assert_eq!(text(&find(&first, ".maestro-badge-outline")), "Local Box");
    assert_eq!(text(&find(&first, ".maestro-model-meta")), "32K/2KFree");
    let count = rows(shown).len();
    for _ in 0..count + 2 {
        press("ArrowDown").await;
    }
    assert!(highlighted(shown, count - 1));
    for _ in 0..count + 2 {
        press("ArrowUp").await;
    }
    composing_arrow(search);
    assert!(highlighted(shown, 0));
}
/// A composing arrow key does not move the highlight.
fn composing_arrow(search: &HtmlInputElement) {
    let init = js_sys::Object::new();
    for (key, value) in [
        ("key", JsValue::from("ArrowDown")),
        ("isComposing", true.into()),
        ("bubbles", true.into()),
    ] {
        js_sys::Reflect::set(&init, &key.into(), &value).unwrap();
    }
    let constructor: js_sys::Function =
        js_sys::Reflect::get(&js_sys::global(), &"KeyboardEvent".into())
            .unwrap()
            .unchecked_into();
    let event =
        js_sys::Reflect::construct(&constructor, &js_sys::Array::of2(&"keydown".into(), &init))
            .unwrap();
    search.dispatch_event(event.unchecked_ref()).unwrap();
}
/// Without matching rows, arrows keep the first position and Enter chooses nothing.
async fn no_results(shown: &Element, chosen: &Rc<RefCell<Vec<Model>>>) {
    type_text("zzzzqqqq").await;
    assert!(rows(shown).is_empty());
    press("ArrowDown").await;
    press("Enter").await;
    assert!(chosen.borrow().is_empty());
    assert_eq!(panels().len(), 1);
    for _ in 0.."zzzzqqqq".len() {
        press("Backspace").await;
    }
    assert!(!rows(shown).is_empty());
}
/// Allowed providers limit the rows, and real pointer movement highlights the row under it.
async fn restricted(view: &Rc<DomView>, page: &JsValue) {
    let selector =
        ModelSelector::open(view, None, Rc::new(|_| {}), Some(vec!["Local Box".into()])).unwrap();
    let shown = panel();
    until(page, || rows(&shown).len() == 2).await;
    press("ArrowDown").await;
    assert!(highlighted(&shown, 1));
    let rect = rows(&shown)[0].get_bounding_client_rect();
    let point = format!(
        "[{},{},false]",
        rect.x() + rect.width() / 2.0,
        rect.y() + rect.height() / 2.0
    );
    crate::contributions_support::driver("driverPointer", &point).await;
    assert!(highlighted(&shown, 0));
    assert!(
        find(&shown, ".maestro-model-list")
            .class_list()
            .contains("maestro-model-pointer")
    );
    selector.close();
    assert!(panels().is_empty());
}

/// Choose a provider type as the user would.
fn choose(select: &Element, value: &str) {
    js_sys::Reflect::set(select, &"value".into(), &value.into()).unwrap();
    let event = web_sys::Event::new("change").unwrap();
    select.dispatch_event(&event).unwrap();
}
/// The current value of a native choice.
fn chosen_value(select: &Element) -> Option<String> {
    js_sys::Reflect::get(select, &"value".into())
        .unwrap()
        .as_string()
}
/// The fields of the open provider dialog: name, address and key.
fn provider_fields(shown: &Element) -> Vec<HtmlInputElement> {
    all(shown, "input")
        .into_iter()
        .map(JsCast::unchecked_into)
        .collect()
}
/// Discovery is tested before saving; a new provider gets an identifier, an edit keeps its own.
#[wasm_bindgen]
pub async fn custom_provider_dialog_tests_saves_and_keeps_edits(import: js_sys::Function) {
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let app = storage("provider-dialog");
    let prompts = Prompts::install(vec![]);
    let fetch = Fetch::install(vec![(200, "application/json", listing(7))]);
    let saved = Rc::new(std::cell::Cell::new(0));
    let count = Rc::clone(&saved);
    let on_save: Rc<dyn Fn()> = Rc::new(move || count.set(count.get() + 1));
    let kind = serde_json::from_value("vllm".into()).unwrap();
    let dialog = CustomProviderDialog::open(&view, None, Some(kind), Some(on_save)).unwrap();
    let shown = panel();
    assert_eq!(text(&find(&shown, "h2")), "Add Provider");
    defaults_and_labels(&view, &shown);
    let fields = provider_fields(&shown);
    let save = all(&shown, ".maestro-provider-dialog-footer button").remove(1);
    assert!(
        save.unchecked_ref::<HtmlButtonElement>().disabled(),
        "a name is required"
    );
    fields[2].focus().unwrap();
    type_text("secret").await;
    tested(&page, &shown, &fetch).await;
    fields[0].focus().unwrap();
    type_text("My vLLM").await;
    click(&save);
    until(&page, || !dialog.is_open()).await;
    assert_eq!(saved.get(), 1);
    let stored = app.custom_providers.get_all().await.unwrap();
    assert_eq!(stored.len(), 1);
    let provider = &stored[0];
    assert!(!provider.id.is_empty());
    assert_eq!(
        (
            provider.name.as_str(),
            provider.base_url.as_str(),
            &provider.api_key,
            &provider.models
        ),
        (
            "My vLLM",
            "http://localhost:8000",
            &Presence::Present("secret".into()),
            &Presence::Missing
        )
    );
    edit_keeps_identity(&view, &page, &app, &prompts).await;
}
/// Identifier-generation failure preserves the form and never creates an empty storage key.
#[wasm_bindgen]
pub async fn custom_provider_dialog_preserves_edits_when_identifier_generation_fails(
    import: js_sys::Function,
) {
    let view = view();
    let _host = host(&view, &import);
    let app = storage("provider-id-failure");
    let prompts = Prompts::install(vec![]);
    let kind = serde_json::from_value("openai-completions".into()).unwrap();
    let dialog = CustomProviderDialog::open(&view, None, Some(kind), None).unwrap();
    let shown = panel();
    let fields = provider_fields(&shown);
    fields[0].focus().unwrap();
    type_text("Kept provider").await;
    fields[1].focus().unwrap();
    type_text("https://local.test").await;
    let window = web_sys::window().unwrap();
    let crypto = js_sys::Reflect::get(&window, &"crypto".into()).unwrap();
    let original = js_sys::Reflect::get(&crypto, &"randomUUID".into()).unwrap();
    js_sys::Reflect::set(&crypto, &"randomUUID".into(), &JsValue::UNDEFINED).unwrap();
    click(&all(&shown, ".maestro-provider-dialog-footer button")[1]);
    while dialog.is_open() && prompts.messages().is_empty() {
        app.custom_providers.get_all().await.unwrap();
    }
    js_sys::Reflect::set(&crypto, &"randomUUID".into(), &original).unwrap();
    assert!(
        dialog.is_open(),
        "failed identifier generation must not save or close"
    );
    assert_eq!(prompts.messages(), ["Failed to save provider"]);
    assert!(app.custom_providers.get_all().await.unwrap().is_empty());
    assert_eq!(fields[0].value(), "Kept provider");
    assert_eq!(fields[1].value(), "https://local.test");
    dialog.close();
}

/// Each type starts from its default address, and every label names its field.
fn defaults_and_labels(view: &DomView, shown: &Element) {
    let fields = provider_fields(shown);
    assert_eq!(fields[1].value(), "http://localhost:8000");
    let select = find(shown, "select");
    choose(&select, "ollama");
    assert_eq!(fields[1].value(), "http://localhost:11434");
    choose(&select, "vllm");
    assert_eq!(chosen_value(&select).as_deref(), Some("vllm"));
    let labels = all(shown, "label");
    assert_eq!(labels.len(), 4);
    for label in &labels {
        let target = view
            .document
            .get_element_by_id(&label.get_attribute("for").unwrap());
        assert!(target.is_some_and(|field| shown.contains(Some(&field))));
    }
}
/// A connection test lists five models and the remainder, then a failed test shows its error.
async fn tested(page: &JsValue, shown: &Element, fetch: &Fetch) {
    let test = find(shown, ".maestro-button-outline");
    click(&test);
    assert_eq!(text(&test), "Testing...");
    until(page, || has(shown, ".maestro-provider-dialog-found")).await;
    let found = find(shown, ".maestro-provider-dialog-found");
    assert!(text(&found).starts_with("Discovered 7 models:"));
    let items = all(&found, "li");
    assert_eq!(items.len(), 6);
    assert_eq!(text(&items[5]), "...and 2 more");
    let request = &fetch.requests()[0];
    assert_eq!(request.url, "http://localhost:8000/v1/models");
    assert!(
        request
            .headers
            .contains(&("authorization".into(), "Bearer secret".into()))
    );
    click(&test);
    until(page, || {
        has(shown, ".maestro-provider-dialog-error")
            && !has(shown, ".maestro-provider-dialog-found")
    })
    .await;
    assert!(!text(&find(shown, ".maestro-provider-dialog-error")).is_empty());
}
/// Editing a manual provider keeps its identifier and models; a failed save keeps the edits.
async fn edit_keeps_identity(
    view: &Rc<DomView>,
    page: &JsValue,
    app: &Rc<maestro_app::AppStorage>,
    prompts: &Prompts,
) {
    let original = manual("kept-id", "Manual", vec![local("Manual", "one", false)]);
    app.custom_providers.set(&original).await.unwrap();
    let dialog = CustomProviderDialog::open(view, Some(original.clone()), None, None).unwrap();
    let shown = panel();
    assert_eq!(text(&find(&shown, "h2")), "Edit Provider");
    assert!(
        text(&shown).contains("For manual provider types, add models after saving the provider.")
    );
    assert!(
        !has(&shown, ".maestro-button-outline"),
        "no test for manual types"
    );
    let name = provider_fields(&shown).remove(0);
    assert_eq!(name.value(), "Manual");
    name.focus().unwrap();
    type_text(" 2").await;
    let unbound = Rc::new(maestro_app::AppStorage::new(
        Rc::default(),
        Rc::default(),
        Rc::default(),
        Rc::default(),
        Rc::clone(&app.backend),
    ));
    maestro_app::set_app_storage(unbound);
    let save = all(&shown, ".maestro-provider-dialog-footer button").remove(1);
    click(&save);
    // The unbound store refuses at once, so the alert follows within the started work.
    settled().await;
    assert_eq!(prompts.messages(), ["Failed to save provider"]);
    assert!(dialog.is_open());
    assert_eq!(name.value(), "Manual 2", "a failed save keeps the edit");
    maestro_app::set_app_storage(Rc::clone(app));
    click(&save);
    until(page, || !dialog.is_open()).await;
    let stored = app.custom_providers.get("kept-id").await.unwrap().unwrap();
    assert_eq!(stored.name, "Manual 2");
    assert_eq!(stored.models, original.models);
}

/// Two stored providers: a manual one with two models and a discovery server.
async fn two_providers(app: &maestro_app::AppStorage) {
    let models = vec![local("Manual A", "x", false), local("Manual A", "y", false)];
    app.custom_providers
        .set(&manual("a", "Manual A", models))
        .await
        .unwrap();
    let mut server = manual("b", "Server B", vec![]);
    server.r#type = serde_json::from_value("vllm".into()).unwrap();
    server.base_url = "http://b.test".into();
    server.models = Presence::Missing;
    app.custom_providers.set(&server).await.unwrap();
}
/// Built-in keys and custom cards appear; refresh, delete and add reach their owners.
#[wasm_bindgen]
pub async fn providers_models_tab_lists_keys_and_manages_custom_providers(
    import: js_sys::Function,
) {
    let view = view();
    let _host = host(&view, &import);
    let page = page_module(&import).await;
    let app = storage("providers-tab");
    two_providers(&app).await;
    let _fetch = Fetch::install(vec![(200, "application/json", listing(3))]);
    let prompts = Prompts::install(vec![false, true]);
    assert_eq!(
        ProvidersModelsTab.get_tab_name(&view).unwrap(),
        "Providers & Models"
    );
    let parent = container();
    let mounted = ProvidersModelsTab.mount(&view).unwrap();
    parent.append_child(&mounted.node).unwrap();
    assert_eq!(
        all(&parent, ".maestro-key-input").len(),
        maestro_app::get_providers().len()
    );
    until(&page, || text(&parent).contains("3 models")).await;
    let cards = all(&parent, ".maestro-provider-card");
    let refresh = all(&cards[1], "button").remove(0);
    assert_eq!(
        (
            cards.len(),
            text(&cards[0]).contains("Models: 2"),
            text(&refresh)
        ),
        (2, true, "Refresh".into())
    );
    click(&refresh);
    until(&page, || text(&parent).contains("Disconnected")).await;
    assert!(
        prompts
            .messages()
            .iter()
            .any(|message| message.starts_with("Failed to refresh provider:"))
    );
    let delete = all(&cards[0], "button").pop().unwrap();
    click(&delete);
    assert_eq!(
        app.custom_providers.get_all().await.unwrap().len(),
        2,
        "a refused confirmation keeps it"
    );
    click(&delete);
    until(&page, || all(&parent, ".maestro-provider-card").len() == 1).await;
    assert!(
        prompts
            .messages()
            .contains(&"?Are you sure you want to delete this provider?".to_owned())
    );
    added(&parent).await;
    parent.remove();
}
/// The Add Provider menu opens the dialog for the chosen type and shows its placeholder again.
async fn added(parent: &Element) {
    let menu = find(parent, "select");
    choose(&menu, "openai-completions");
    let shown = panel();
    assert_eq!(text(&find(&shown, "h2")), "Add Provider");
    assert_eq!(
        chosen_value(&find(&shown, "select")).as_deref(),
        Some("openai-completions")
    );
    assert_eq!(chosen_value(&menu).as_deref(), Some(""));
    press("Escape").await;
    assert!(panels().is_empty());
}

/// The key prompt settles true once a key is stored, and false when closed first.
#[wasm_bindgen]
pub async fn api_key_prompt_settles_on_stored_key_or_close() {
    let view = view();
    let app = storage("key-prompt");
    let prompted = ApiKeyPromptDialog::prompt(&view, "anthropic").unwrap();
    let shown = panel();
    assert_eq!(text(&find(&shown, "h2")), "API Key Required");
    assert_eq!(text(&find(&shown, ".maestro-key-provider")), "anthropic");
    app.provider_keys.set("anthropic", "stored").await.unwrap();
    assert!(prompted.await);
    assert!(panels().is_empty());
    let prompted = ApiKeyPromptDialog::prompt(&view, "openai").unwrap();
    settled().await;
    press("Escape").await;
    assert!(!prompted.await);
    assert!(panels().is_empty());
}
