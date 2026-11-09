//! Page-side observations of prepared sandbox documents written into real frames.
use crate::browser_fixture_support::{
    Patch, construct, get, global, invoke, object, set, signal, variadic,
};
use crate::contributions_support::settle;
use maestro_web::components::sandbox::runtime_bundle::BUNDLE;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Element, HtmlIFrameElement};

/// The page's JSON text of `value`, or `None` where the page serializes nothing.
pub(crate) fn stringify(value: &JsValue) -> Option<String> {
    js_sys::JSON::stringify(value)
        .ok()
        .and_then(|text| text.as_string())
}

/// Require `actual` to serialize exactly as `expected` does.
pub(crate) fn equal(actual: &JsValue, expected: &JsValue, label: &str) {
    let actual = stringify(actual);
    assert!(
        actual == stringify(expected),
        "{label}: {}",
        actual.unwrap_or_default()
    );
}

/// Require `actual` to serialize exactly as the JSON `expected` text does.
pub(crate) fn equal_json(actual: &JsValue, expected: &str, label: &str) {
    equal(actual, &js_sys::JSON::parse(expected).unwrap(), label);
}

/// The page value a tagged recorded value describes, which JSON alone cannot carry.
pub(crate) fn decode(value: &JsValue) -> JsValue {
    if !value.is_object() {
        return value.clone();
    }
    let tagged = |key: &str| js_sys::Reflect::has(value, &key.into()).unwrap();
    let field = |key: &str| get(value, key);
    let convert = |name: &str, key: &str| {
        global(name)
            .unchecked_into::<js_sys::Function>()
            .call1(&JsValue::UNDEFINED, &field(key))
            .unwrap()
    };
    if tagged("$undefined") {
        return JsValue::UNDEFINED;
    }
    if tagged("$bigint") {
        return convert("BigInt", "$bigint");
    }
    if tagged("$number") {
        return convert("Number", "$number");
    }
    if tagged("$date") {
        return construct("Date", &[&field("$date")]);
    }
    if tagged("$bytes") {
        return construct("Uint8Array", &[&field("$bytes")]);
    }
    if tagged("$cycle") {
        let cyclic = js_sys::Object::new();
        set(&cyclic, "self", &cyclic);
        return cyclic.into();
    }
    if js_sys::Array::is_array(value) {
        return js_sys::Array::from(value)
            .iter()
            .map(|item| decode(&item))
            .collect::<js_sys::Array>()
            .into();
    }
    let decoded = js_sys::Object::new();
    for entry in js_sys::Object::entries(value.unchecked_ref()).iter() {
        let entry = js_sys::Array::from(&entry);
        js_sys::Reflect::set(&decoded, &entry.get(0), &decode(&entry.get(1))).unwrap();
    }
    decoded.into()
}

/// Remove every test parent the transaction mounted.
pub(crate) fn finish() {
    let parents = crate::contributions_support::document()
        .query_selector_all("[data-sandbox-test]")
        .unwrap();
    for index in 0..parents.length() {
        parents
            .item(index)
            .unwrap()
            .unchecked_into::<Element>()
            .remove();
    }
}

/// The program texts a prepared document carries: its inert provider carriers, then its module.
pub(crate) fn program_sources(html: &str) -> Vec<String> {
    let parser = web_sys::DomParser::new().unwrap();
    let parsed = parser
        .parse_from_string(html, web_sys::SupportedType::TextHtml)
        .unwrap();
    let mut scripts: Vec<Element> = Vec::new();
    let templates = parsed.query_selector_all("template").unwrap();
    for index in 0..templates.length() {
        let template: Element = templates.item(index).unwrap().unchecked_into();
        let carried = js_sys::Array::from(&get(&get(&template, "content"), "children"));
        for child in carried.iter() {
            collect(child.unchecked_ref(), "script", &mut scripts);
        }
    }
    let vector = parsed.query_selector_all("svg script").unwrap();
    for index in 0..vector.length() {
        scripts.push(vector.item(index).unwrap().unchecked_into());
    }
    scripts
        .iter()
        .map(|script| script.text_content().unwrap_or_default())
        .collect()
}

/// `root` and its descendants matching `selector`, in document order.
fn collect(root: &Element, selector: &str, found: &mut Vec<Element>) {
    if root.matches(selector).unwrap() {
        found.push(root.clone());
    }
    let below = root.query_selector_all(selector).unwrap();
    for index in 0..below.length() {
        found.push(below.item(index).unwrap().unchecked_into());
    }
}

/// The inert configuration block of a prepared document, as JSON.
pub(crate) fn configuration(html: &str) -> serde_json::Value {
    let (_, rest) = html
        .split_once("<script type=\"application/json\" data-maestro-sandbox>")
        .unwrap();
    serde_json::from_str(rest.split_once("</script>").unwrap().0).unwrap()
}

/// The marker that starts a prepared program's scaffold.
const SCAFFOLD: &str =
    "<svg display=\"none\" data-maestro-sandbox-program><script type=\"module\">";

/// Write `parts` in order into a fresh same-origin frame, calling `between` after each part
/// except the last, so a witness can act between two stages of parsing.
pub(crate) async fn written(
    parts: &[&str],
    mut between: impl FnMut(&JsValue),
) -> HtmlIFrameElement {
    let frame = realm().await;
    let window = window_of(&frame);
    let document = get(&window, "document");
    invoke(&document, "open", &[]).unwrap();
    for (index, part) in parts.iter().enumerate() {
        invoke(&document, "write", &[&(*part).into()]).unwrap();
        if index + 1 < parts.len() {
            between(&window);
        }
    }
    invoke(&document, "close", &[]).unwrap();
    frame
}

/// Lines of `text` that are not line comments.
fn without_comments(text: &str) -> String {
    text.split('\n')
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A same-origin frame realm, connected and loaded.
pub(crate) async fn realm() -> HtmlIFrameElement {
    let document = crate::contributions_support::document();
    let frame: HtmlIFrameElement = document.create_element("iframe").unwrap().unchecked_into();
    let loaded = loaded(&frame);
    document.body().unwrap().append_child(&frame).unwrap();
    loaded.await;
    frame
}

/// Settle when `target` next fires `load`.
pub(crate) fn loaded(target: &web_sys::EventTarget) -> impl std::future::Future<Output = ()> {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let once = object(&[("once", &JsValue::TRUE)]);
        invoke(
            target,
            "addEventListener",
            &[&"load".into(), &resolve, &once],
        )
        .unwrap();
    });
    async move {
        settle(promise.into()).await.unwrap();
    }
}

/// The frame realm's window.
pub(crate) fn window_of(frame: &HtmlIFrameElement) -> JsValue {
    frame.content_window().unwrap().into()
}

/// Compare a generated document and execute it in a real frame.
pub(crate) async fn observe_document(name: &str, html: &str, output: &JsValue) {
    let scripts = program_sources(html);
    let config = configuration(html);
    let expected = |key: &str| get(output, key);
    let marker = format!("<script>{BUNDLE}</script>");
    let suffix = &html[html.find(&marker).unwrap() + marker.len()..];
    // The lowered program is observed by running it; only its envelope is compared here.
    let authored = match suffix.find(SCAFFOLD) {
        Some(at) => {
            let end = suffix[at..].find("</script></svg>").unwrap() + at;
            format!("{}{}", &suffix[..at + SCAFFOLD.len()], &suffix[end..])
        }
        None => suffix.to_owned(),
    };
    equal(
        &html[..html.find("<style>").unwrap()].into(),
        &expected("prefix"),
        &format!("{name} prefix"),
    );
    equal(
        &without_comments(&authored).into(),
        &without_comments(&expected("suffix").as_string().unwrap()).into(),
        &format!("{name} suffix"),
    );
    let module = usize::from(suffix.contains(SCAFFOLD));
    equal(
        &(1 + module).into(),
        &expected("script_count"),
        &format!("{name} scripts"),
    );
    for key in ["bridge", "navigation"] {
        equal(
            &(!config["standalone"].as_bool().unwrap()).into(),
            &expected(key),
            &format!("{name} {key}"),
        );
    }
    if expected("completions").is_truthy() {
        completions(name, html, &scripts[scripts.len() - 1], output).await;
        return;
    }
    let frame = written(&[html], |_| {}).await;
    observe_values(name, &config, &window_of(&frame));
    frame.remove();
}

/// The values a prepared runtime defines on its window, by case name.
fn observe_values(name: &str, config: &serde_json::Value, window: &JsValue) {
    let value = || get(window, "value");
    let label = |part: &str| format!("{name}{part}");
    match name {
        "id_literal</script>tail" => equal(
            &get(window, "sandboxId"),
            &"literal</script>tail".into(),
            name,
        ),
        "runtime_closing_script" => equal(&get(window, "literal"), &"</script>".into(), name),
        "invalid_key" => equal(&get(window, "hyphen-name"), &1.into(), name),
        "numeric_keys" => numeric_keys(name, config, window),
        "merge_order" => {
            let read = js_sys::Array::of3(&get(window, "a"), &get(window, "b"), &get(window, "c"));
            equal_json(&read, r#"[1,"new",3]"#, name);
            let keys: Vec<_> = config["globals"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| pair[0].as_str().unwrap().to_owned())
                .collect();
            assert_eq!(keys, ["a", "b", "c"], "{}", label(" insertion order"));
        }
        "provider_id_override" => {
            equal(&get(window, "sandboxId"), &"provider override".into(), name);
            equal_json(
                &calls(window),
                r#"["fixture-id"]"#,
                &label(" invocation id"),
            );
        }
        "value_null" | "value_nonfinite" => assert!(value().is_null(), "{}", label(" native null")),
        "value_number" => assert!(
            js_sys::Object::is(&value(), &0.into()),
            "{}",
            label(" native positive zero")
        ),
        "value_array" => assert!(
            get(&value(), "0").is_null(),
            "{}",
            label(" native null element")
        ),
        _ if name.starts_with("id_quote") => {
            equal(&get(window, "sandboxId"), &"quote\"\\雪\n".into(), name);
        }
        _ => {}
    }
    let expected = match name {
        "value_string" => Some(r#""</ScRiPt>雪""#),
        "value_boolean" => Some("true"),
        "value_object" => Some(r#"{"1":"one","2":"two","b":1,"a":2}"#),
        "value_array" => Some(r#"[null,"x",2]"#),
        "value_date" => Some(r#""2020-01-02T03:04:05.000Z""#),
        "value_typed_array" => Some(r#"{"0":1,"1":255}"#),
        _ => None,
    };
    if let Some(expected) = expected {
        equal_json(&value(), expected, &label(" native value"));
    }
}

/// The recorded provider invocations on a window, as an array.
fn calls(window: &JsValue) -> JsValue {
    js_sys::Array::from(&get(window, "calls")).into()
}

/// Generated numeric keys keep canonical order in the configuration, and the window's indexed
/// setters keep them off the window.
fn numeric_keys(name: &str, config: &serde_json::Value, window: &JsValue) {
    let keys: Vec<_> = config["globals"]
        .as_array()
        .unwrap()
        .iter()
        .map(|pair| pair[0].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(keys, ["1", "2", "a"], "{name} canonical order");
    assert!(
        get(window, "1").is_undefined() && get(window, "2").is_undefined() && get(window, "a") == 3,
        "{name} native Window indexed setters"
    );
    equal_json(
        &calls(window),
        r#"["fixture-id"]"#,
        &format!("{name} native provider"),
    );
}

/// Run the prepared program in a real frame with recorded completions and diagnostics.
///
/// The runtime installs from the first part of the document; the witness then replaces the
/// completion hooks before the program's scaffold is parsed.
async fn completions(name: &str, html: &str, module: &str, output: &JsValue) {
    let (completions, logs) = (js_sys::Array::new(), js_sys::Array::new());
    let (done, finish) = signal();
    let recorded = completions.clone();
    let callback = variadic(move |args| {
        recorded.push(&js_sys::Array::of2(&"callback".into(), &args.get(0)));
        js_sys::Promise::resolve(&JsValue::UNDEFINED).into()
    });
    let completed = completions.clone();
    let complete = variadic(move |args| {
        completed.push(&args);
        finish.call0(&JsValue::UNDEFINED).unwrap();
        js_sys::Promise::resolve(&JsValue::UNDEFINED).into()
    });
    let logged = logs.clone();
    let error = variadic(move |args| {
        let texts: js_sys::Array = args
            .iter()
            .map(|arg| JsValue::from(native_text(&arg)))
            .collect();
        logged.push(&texts);
        JsValue::UNDEFINED
    });
    let at = html.find(SCAFFOLD).unwrap_or(html.len());
    assert!(html[at..].contains(module.lines().next().unwrap_or_default()));
    let frame = written(&[&html[..at], &html[at..]], |window| {
        set(
            window,
            "__completionCallbacks",
            &js_sys::Array::of1(&callback),
        );
        set(window, "complete", &complete);
        set(&get(window, "console"), "error", &error);
    })
    .await;
    settle(done.into()).await.unwrap();
    let projected = js_sys::JSON::parse(&stringify(&completions).unwrap()).unwrap();
    if name.starts_with("repl_error") {
        let last = get(&js_sys::Array::from(&projected).at(-1), "0");
        assert!(has_stack(&last), "{name} native stack");
        set(&last, "stack", &"<native stack>".into());
    }
    equal(
        &projected,
        &get(output, "completions"),
        &format!("{name} completions"),
    );
    equal(&logs, &get(output, "logs"), &format!("{name} diagnostics"));
    frame.remove();
}

/// Whether an error carries a nonempty native stack.
fn has_stack(error: &JsValue) -> bool {
    get(error, "stack")
        .as_string()
        .is_some_and(|stack| !stack.is_empty())
}

/// The page's own text of a value, as `String(value)` gives it.
fn native_text(value: &JsValue) -> String {
    global("String")
        .unchecked_into::<js_sys::Function>()
        .call1(&JsValue::UNDEFINED, value)
        .unwrap()
        .as_string()
        .unwrap()
}

/// The `window` names matching `pattern` after `html` is written into a fresh frame, followed
/// by the type of each named helper.
pub(crate) async fn defined(html: &str, pattern: &str, helpers: &[&str]) -> JsValue {
    let frame = written(&[html], |_| {}).await;
    let window = window_of(&frame);
    let expression = regress::Regex::new(pattern).unwrap();
    let names: js_sys::Array =
        js_sys::Object::get_own_property_names(window.unchecked_ref::<js_sys::Object>())
            .iter()
            .filter(|name| expression.find(&name.as_string().unwrap()).is_some())
            .collect();
    let types: js_sys::Array = helpers
        .iter()
        .map(|helper| JsValue::from(get(&window, helper).js_typeof().as_string().unwrap()))
        .collect();
    frame.remove();
    js_sys::Array::of2(&names, &types).into()
}

/// Observe native registration synchronously around standalone runtime execution.
pub(crate) async fn standalone(html: &str) {
    let frame = realm().await;
    let window = window_of(&frame);
    let registered = Rc::new(RefCell::new(Vec::<String>::new()));
    let document_events = Rc::clone(&registered);
    let window_events = Rc::clone(&registered);
    let patches = [
        Patch::method(
            &get(&window, "document"),
            "addEventListener",
            move |original, this, args| {
                document_events.borrow_mut().push(format!(
                    "document:{}",
                    args.get(0).as_string().unwrap_or_default()
                ));
                original.apply(&this, &args)
            },
        ),
        Patch::method(&window, "addEventListener", move |original, this, args| {
            window_events.borrow_mut().push(format!(
                "window:{}",
                args.get(0).as_string().unwrap_or_default()
            ));
            original.apply(&this, &args)
        }),
    ];
    let document = get(&window, "document");
    invoke(&document, "open", &[]).unwrap();
    invoke(&document, "write", &[&html.into()]).unwrap();
    invoke(&document, "close", &[]).unwrap();
    equal(
        &invoke(&window, "localHelper", &[]).unwrap(),
        &"offline".into(),
        "standalone helper",
    );
    assert!(
        registered.borrow().is_empty(),
        "finite synchronous registration set: {:?}",
        registered.borrow()
    );
    assert!(
        get(&window, "sendRuntimeMessage").is_undefined(),
        "no online bridge"
    );
    assert!(
        get(&window, "complete").is_undefined(),
        "no implicit console"
    );
    drop(patches);
    frame.remove();
}

/// Execute the wrapper with controlled ordered completion gates.
pub(crate) async fn callbacks(html: &str, input: &serde_json::Value, expected: &serde_json::Value) {
    let events = js_sys::Array::new();
    let mode = input["callbacks"].as_str().unwrap_or_default();
    let (gate, release) = signal();
    let (ready, started) = signal();
    let callbacks = completion_callbacks(mode, &events, &gate, &started);
    let registered = if mode == "none" {
        js_sys::Array::new()
    } else {
        js_sys::Array::of2(callbacks[0].as_ref(), callbacks[1].as_ref())
    };
    let (done, finish) = signal();
    let at = html.find(SCAFFOLD).unwrap();
    let recorders = RefCell::new(None);
    let frame = written(&[&html[..at], &html[at..]], |window| {
        set(window, "__completionCallbacks", &registered);
        *recorders.borrow_mut() = Some(completion_recorders(window, &events, &finish));
    })
    .await;
    if mode == "gated" {
        settle(ready.into()).await.unwrap();
        equal_json(
            &events,
            r#"[["first",true],["second",true]]"#,
            "completion still pending",
        );
        release.call0(&JsValue::UNDEFINED).unwrap();
    }
    settle(done.into()).await.unwrap();
    equal_json(
        &events,
        &expected["events"].to_string(),
        "ordered completion observations",
    );
    drop(recorders);
    frame.remove();
}

/// A completion callback recording its settlement.
type Callback = Closure<dyn Fn(JsValue) -> js_sys::Promise>;

/// Two completion callbacks recording into `events`; the first rejects or waits for `gate`,
/// after reporting `started`, as `mode` selects.
fn completion_callbacks(
    mode: &str,
    events: &js_sys::Array,
    gate: &js_sys::Promise,
    started: &js_sys::Function,
) -> [Callback; 2] {
    let (first_events, first_mode) = (events.clone(), mode.to_owned());
    let (gate, started) = (gate.clone(), started.clone());
    // Like an async callback, each records its call before its first wait.
    let first = Closure::<dyn Fn(JsValue) -> js_sys::Promise>::new(move |ok: JsValue| {
        first_events.push(&js_sys::Array::of2(&"first".into(), &ok));
        match first_mode.as_str() {
            "reject" => js_sys::Promise::reject(&js_sys::Error::new("callback failure")),
            "gated" => {
                started.call0(&JsValue::UNDEFINED).unwrap();
                let (events, gate) = (first_events.clone(), gate.clone());
                wasm_bindgen_futures::future_to_promise(async move {
                    settle(gate.into()).await?;
                    events.push(&js_sys::Array::of2(&"settled".into(), &ok));
                    Ok(JsValue::UNDEFINED)
                })
            }
            _ => js_sys::Promise::resolve(&JsValue::UNDEFINED),
        }
    });
    let second_events = events.clone();
    let second = Closure::<dyn Fn(JsValue) -> js_sys::Promise>::new(move |ok: JsValue| {
        second_events.push(&js_sys::Array::of2(&"second".into(), &ok));
        js_sys::Promise::resolve(&JsValue::UNDEFINED)
    });
    [first, second]
}

/// The frame's console error and completion function, recording into `events` while kept.
fn completion_recorders(
    window: &JsValue,
    events: &js_sys::Array,
    finished: &js_sys::Function,
) -> (Closure<dyn Fn(JsValue)>, Completion) {
    let diagnostics = events.clone();
    let error = Closure::<dyn Fn(JsValue)>::new(move |message: JsValue| {
        diagnostics.push(&js_sys::Array::of2(&"diagnostic".into(), &message));
    });
    set(&get(window, "console"), "error", error.as_ref());
    let completed = events.clone();
    let finished = finished.clone();
    let complete = Closure::<dyn Fn(JsValue, JsValue) -> js_sys::Promise>::new(
        move |error: JsValue, value: JsValue| {
            let message = if error.is_truthy() {
                assert!(has_stack(&error), "native stack");
                get(&error, "message")
            } else {
                JsValue::NULL
            };
            let value = if value.is_null() || value.is_undefined() {
                JsValue::NULL
            } else {
                value
            };
            completed.push(&js_sys::Array::of3(&"complete".into(), &message, &value));
            finished.call0(&JsValue::UNDEFINED).unwrap();
            js_sys::Promise::resolve(&JsValue::UNDEFINED)
        },
    );
    set(window, "complete", complete.as_ref());
    (error, complete)
}

/// A completion function taking an error and a value.
type Completion = Closure<dyn Fn(JsValue, JsValue) -> js_sys::Promise>;

/// Dispatch native events through the injected navigation handler.
pub(crate) async fn navigation(
    html: &str,
    input: &serde_json::Value,
    expected: &serde_json::Value,
) {
    let events = js_sys::Array::new();
    let frame = written(&[html], |_| {}).await;
    let window = window_of(&frame);
    let document: web_sys::Document = get(&window, "document").unchecked_into();
    let base = document.create_element("base").unwrap();
    base.set_attribute("href", "https://example.org/a").unwrap();
    document
        .query_selector("head")
        .unwrap()
        .unwrap()
        .append_child(&base)
        .unwrap();
    let kind = input["kind"].as_str().unwrap();
    let target = navigation_target(&document, kind, input["href"].as_str());
    if !target.is_connected() {
        document.body().unwrap().append_child(&target).unwrap();
    }
    let event = navigation_event(&window, kind);
    let _recorders = navigation_recorders(&event, &events);
    target.dispatch_event(&event).unwrap();
    equal_json(
        &events,
        &expected["events"].to_string(),
        &format!("native resolved navigation {kind}"),
    );
    let prevents = expected["events"]
        .as_array()
        .is_some_and(|events| !events.is_empty());
    equal(
        &event.default_prevented().into(),
        &prevents.into(),
        &format!("default action decision {kind}"),
    );
    frame.remove();
}

/// A bubbling, cancelable `submit` for a form, else `click`, from the frame's realm.
fn navigation_event(window: &JsValue, kind: &str) -> web_sys::Event {
    let options = object(&[("bubbles", &JsValue::TRUE), ("cancelable", &JsValue::TRUE)]);
    let name = if kind == "form" { "submit" } else { "click" };
    js_sys::Reflect::construct(
        get(window, "Event").unchecked_ref::<js_sys::Function>(),
        &js_sys::Array::of2(&name.into(), &options),
    )
    .unwrap()
    .unchecked_into()
}

/// Record the event's prevention and propagation stops and the page's posted messages.
fn navigation_recorders(event: &web_sys::Event, events: &js_sys::Array) -> [Patch; 3] {
    let recorded = |label: &'static str, entry: &'static str| {
        let events = events.clone();
        Patch::method(event, label, move |original, this, args| {
            events.push(&js_sys::Array::of1(&entry.into()));
            original.apply(&this, &args)
        })
    };
    let posted = events.clone();
    let post = Patch::method(
        &global("window"),
        "postMessage",
        move |original, this, args| {
            posted.push(&js_sys::Array::of3(
                &"post".into(),
                &args.get(0),
                &args.get(1),
            ));
            original.apply(&this, &args)
        },
    );
    [
        recorded("preventDefault", "prevent"),
        recorded("stopPropagation", "stop"),
        post,
    ]
}

/// The element a navigation case dispatches its event at.
fn navigation_target(document: &web_sys::Document, kind: &str, href: Option<&str>) -> Element {
    let svg = "http://www.w3.org/2000/svg";
    match kind {
        "form" => {
            let form = document.create_element("form").unwrap();
            form.set_attribute("action", href.unwrap_or_default())
                .unwrap();
            form
        }
        "svg" => {
            let image = document.create_element_ns(Some(svg), "svg").unwrap();
            let anchor = document.create_element_ns(Some(svg), "a").unwrap();
            anchor
                .set_attribute("href", href.unwrap_or_default())
                .unwrap();
            let target = document.create_element_ns(Some(svg), "text").unwrap();
            anchor.append_child(&target).unwrap();
            image.append_child(&anchor).unwrap();
            document.body().unwrap().append_child(&image).unwrap();
            target
        }
        "nonlink" => document.create_element("span").unwrap(),
        _ => {
            let anchor = document.create_element("a").unwrap();
            if let Some(href) = href.filter(|href| !href.is_empty()) {
                let href = if kind == "resolved_hash" {
                    "#part"
                } else {
                    href
                };
                anchor.set_attribute("href", href).unwrap();
            }
            let target = document.create_element("span").unwrap();
            anchor.append_child(&target).unwrap();
            document.body().unwrap().append_child(&anchor).unwrap();
            target
        }
    }
}
