//! Native message registrations, openers and frame routes observed below the adapter.
use super::native::{equal, equal_json};
use crate::browser_fixture_support::{
    Patch, construct, dom_exception, error, get, global, object, set,
};
use crate::contributions_support::settle;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast as _, JsValue, closure::Closure};
use web_sys::{Element, HtmlIFrameElement};

/// A registered message callback and the counting wrapper the page actually holds.
type Registration = (JsValue, Closure<dyn Fn(JsValue) -> Result<(), JsValue>>);

/// Everything one routing transaction observed.
#[derive(Default)]
struct Observed {
    /// Live message registrations.
    active: Vec<Registration>,
    /// Arguments of each window opening.
    opened: Vec<js_sys::Array>,
    /// Options of each extension tab creation.
    tabs: js_sys::Array,
    /// Consumer messages, in order.
    messages: Vec<JsValue>,
    /// Provider messages, in order.
    provider_messages: Vec<JsValue>,
    /// Loads the frame reported receiving.
    received: Vec<JsValue>,
    /// Replies the frame reported receiving.
    replies: Vec<JsValue>,
    /// Message deliveries through registered callbacks.
    deliveries: u32,
    /// The selected fault.
    mode: String,
    /// Registrations attempted under the current fault.
    adds: u32,
    /// Frame windows seen as current, oldest first.
    frames: Vec<JsValue>,
    /// The awaited frame signal.
    wait: Option<js_sys::Promise>,
}

/// A running routing transaction.
struct Transaction {
    /// Shared observations.
    observed: Rc<RefCell<Observed>>,
    /// The page's own registration, for the transaction's own listeners.
    add: js_sys::Function,
    /// The page's own removal.
    remove: js_sys::Function,
    /// The installed replacements of registration, removal and opening.
    patches: Vec<Patch>,
    /// Faults installed by `fault`, restored before the replacements.
    faults: Vec<Patch>,
    /// The transaction's own listener recording frame reports.
    listener: Closure<dyn Fn(web_sys::MessageEvent)>,
}

/// One routing transaction at a time below the adapter.
#[derive(Clone, Default)]
pub(crate) struct Routes(Rc<RefCell<Option<Transaction>>>);

/// The page window.
fn window() -> JsValue {
    global("window")
}

/// The newest mounted test frame.
fn current() -> Option<HtmlIFrameElement> {
    frames().pop()
}

/// Every mounted test frame, in document order.
fn frames() -> Vec<HtmlIFrameElement> {
    let found = crate::contributions_support::document()
        .query_selector_all("[data-sandbox-test] iframe")
        .unwrap();
    (0..found.length())
        .map(|index| found.item(index).unwrap().unchecked_into())
        .collect()
}

/// Post `value` to `frame`'s window.
fn send(frame: &HtmlIFrameElement, value: &JsValue) {
    frame
        .content_window()
        .unwrap()
        .post_message(value, "*")
        .unwrap();
}

/// Settle with the data of the next page message of `kind`, listening through the page's own
/// `add` and `remove`.
pub(crate) fn once_message(
    add: &js_sys::Function,
    remove: &js_sys::Function,
    kind: &str,
) -> js_sys::Promise {
    let (add, remove, wanted) = (add.clone(), remove.clone(), kind.to_owned());
    js_sys::Promise::new(&mut |resolve, _| {
        let listener: Rc<RefCell<JsValue>> = Rc::default();
        let (remove, registered, wanted) = (remove.clone(), Rc::clone(&listener), wanted.clone());
        let callback =
            Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
                if message_type(&event.data()).as_deref() != Some(wanted.as_str()) {
                    return;
                }
                remove
                    .call2(&window(), &"message".into(), &registered.borrow())
                    .unwrap();
                resolve.call1(&JsValue::UNDEFINED, &event.data()).unwrap();
            })
            .into_js_value();
        add.call2(&window(), &"message".into(), &callback).unwrap();
        *listener.borrow_mut() = callback;
    })
}

/// Dispatch a page `message` event carrying `data` from `source`.
pub(crate) fn dispatch_message(data: &JsValue, source: &JsValue) {
    let event = construct(
        "MessageEvent",
        &[
            &"message".into(),
            &object(&[("data", data), ("source", source)]),
        ],
    );
    web_sys::window()
        .unwrap()
        .dispatch_event(event.unchecked_ref())
        .unwrap();
}

/// The `type` of a message's data, when it has one.
fn message_type(data: &JsValue) -> Option<String> {
    if !data.is_object() {
        return None;
    }
    get(data, "type").as_string()
}

impl Routes {
    /// Instrument native registrations and openers below the adapter.
    pub(crate) fn begin(&self) {
        assert!(self.0.borrow().is_none(), "one native transaction");
        let observed: Rc<RefCell<Observed>> = Rc::default();
        let window = window();
        let add: js_sys::Function = get(&window, "addEventListener").unchecked_into();
        let remove: js_sys::Function = get(&window, "removeEventListener").unchecked_into();
        let reports = Rc::clone(&observed);
        let listener =
            Closure::<dyn Fn(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
                let data = event.data();
                match message_type(&data).as_deref() {
                    Some("code-received") => reports.borrow_mut().received.push(data),
                    Some("response-received") => {
                        reports.borrow_mut().replies.push(get(&data, "reply"));
                    }
                    _ => {}
                }
            });
        add.call2(&window, &"message".into(), listener.as_ref())
            .unwrap();
        let patches = vec![
            registrations(&window, &observed),
            removals(&window, &observed),
            openings(&window, &observed),
        ];
        *self.0.borrow_mut() = Some(Transaction {
            observed,
            add,
            remove,
            patches,
            faults: Vec::new(),
            listener,
        });
    }

    /// Restore instrumentation after native cleanup.
    pub(crate) fn end(&self) {
        let transaction = self.0.borrow_mut().take().unwrap();
        drop(transaction.faults);
        transaction
            .remove
            .call2(&window(), &"message".into(), transaction.listener.as_ref())
            .unwrap();
        let live = transaction.observed.borrow().active.len();
        equal(&live.into(), &0.into(), "all native registrations released");
        drop(transaction.patches);
    }

    /// The running transaction's observations.
    fn observed(&self) -> Rc<RefCell<Observed>> {
        Rc::clone(&self.0.borrow().as_ref().unwrap().observed)
    }

    /// Start waiting for the next frame message of `kind`.
    pub(crate) fn arm(&self, kind: &str) {
        let transaction = self.0.borrow();
        let transaction = transaction.as_ref().unwrap();
        let wait = once_message(&transaction.add, &transaction.remove, kind);
        transaction.observed.borrow_mut().wait = Some(wait);
    }

    /// Await the armed frame message.
    pub(crate) async fn waited(&self) -> JsValue {
        let wait = self.observed().borrow().wait.clone().unwrap();
        settle(wait.into()).await.unwrap()
    }

    /// Post a controlled message from `source`: the current, previous or sibling frame, the
    /// page itself, or no window.
    pub(crate) fn dispatch(&self, kind: &str, source: &str) {
        let observed = self.observed();
        let frames = frames();
        let frame = frames
            .last()
            .and_then(HtmlIFrameElement::content_window)
            .map(JsValue::from);
        if let Some(frame) = &frame {
            let mut observed = observed.borrow_mut();
            if !observed.frames.contains(frame) {
                observed.frames.push(frame.clone());
            }
        }
        let selected = match source {
            "current" => frame.unwrap_or(JsValue::NULL),
            "previous" => observed
                .borrow()
                .frames
                .first()
                .cloned()
                .unwrap_or(JsValue::NULL),
            "sibling" => frames
                .first()
                .and_then(HtmlIFrameElement::content_window)
                .map_or(JsValue::NULL, Into::into),
            "foreign" => window(),
            _ => JsValue::NULL,
        };
        let data = object(&[
            ("type", &kind.into()),
            ("url", &"https://example.org/controlled".into()),
            ("error", &"bootstrap problem".into()),
            ("stack", &"native stack".into()),
        ]);
        dispatch_message(&data, &selected);
    }

    /// Retain the provider's observation independently of the consumer.
    pub(crate) fn record_provider(&self, message: &JsValue) {
        self.observed()
            .borrow_mut()
            .provider_messages
            .push(message.clone());
    }

    /// Record the same consumer payload received through normal routing.
    pub(crate) fn record(&self, message: &JsValue) {
        self.observed().borrow_mut().messages.push(message.clone());
    }

    /// Require `expected` live registrations and prove closed routes cannot deliver or open.
    pub(crate) fn closed(&self, expected: u32) {
        let observed = self.observed();
        let (live, before, opens) = {
            let observed = observed.borrow();
            (
                observed.active.len(),
                observed.deliveries,
                observed.opened.len(),
            )
        };
        equal(
            &live.into(),
            &expected.into(),
            "native registration set after close",
        );
        let data = object(&[
            ("type", &"open-external-url".into()),
            ("url", &"https://example.org/closed".into()),
        ]);
        dispatch_message(&data, &window());
        let observed = observed.borrow();
        if expected == 0 {
            equal(
                &observed.deliveries.into(),
                &before.into(),
                "no native callback survives close",
            );
        }
        equal(
            &observed.opened.len().into(),
            &opens.into(),
            "closed source cannot open",
        );
    }

    /// Select one controlled failure below the adapter; an empty mode selects none.
    pub(crate) fn fault(&self, mode: &str) {
        let observed = self.observed();
        {
            let mut observed = observed.borrow_mut();
            observed.mode = mode.to_owned();
            observed.adds = 0;
        }
        let installed = match mode {
            "sibling" => {
                sibling();
                None
            }
            "tabs" => Some(tabs(&observed)),
            "parse-throw" | "parse-no-message" => Some(parse_failure(&observed)),
            "create" | "append" => Some(creation_failure(mode, &observed)),
            _ => None,
        };
        if let Some(patch) = installed {
            self.0.borrow_mut().as_mut().unwrap().faults.push(patch);
        }
    }

    /// Exercise the mounted frame's routes after a completed load, selected by `mode`.
    pub(crate) async fn route_check(&self, mode: &str) {
        let observed = self.observed();
        let frame = current();
        match mode {
            "srcdoc" => self.srcdoc_route(&frame.unwrap()).await,
            "click" => self.click_route(&frame.unwrap()).await,
            "sibling-probe" => {
                let before = observed.borrow().messages.len();
                self.exchange(&frame.unwrap(), "send-probe", "probe-done")
                    .await;
                let texts = console(&observed.borrow().messages[before..], false);
                equal_json(&texts, r#"["normal"]"#, "surviving sibling consumer");
            }
            "unrelated" => self.unrelated_route(),
            "ready" => self.ready_route(&frame.unwrap()).await,
            "replayed" => self.replayed_route(&frame.unwrap()).await,
            "error" => self.error_route(&frame.unwrap()).await,
            "tabs" => self.tabs_route(&frame.unwrap()).await,
            "reset-opens" => {
                let mut observed = observed.borrow_mut();
                observed.opened.clear();
                observed.received.clear();
            }
            "empty" => {
                equal(
                    &frames().len().into(),
                    &0.into(),
                    "failed attempt removed frame",
                );
                self.closed(0);
            }
            other => panic!("unknown route check {other}"),
        }
    }

    /// A nested link click opens once, through the adapter and the shared router.
    async fn click_route(&self, frame: &HtmlIFrameElement) {
        let observed = self.observed();
        let before = observed.borrow().opened.len();
        self.exchange(frame, "click-now", "clicked").await;
        let observed = observed.borrow();
        equal_json(
            &opened(&observed.opened[before..]),
            r#"[["https://example.org/fixture","_blank"]]"#,
            "one native open for nested click",
        );
        equal(
            &observed.active.len().into(),
            &2.into(),
            "one adapter plus shared router registration",
        );
    }

    /// An extension opens through its tabs rather than a window.
    async fn tabs_route(&self, frame: &HtmlIFrameElement) {
        self.exchange(frame, "send-ready", "url-done").await;
        let observed = self.observed();
        let observed = observed.borrow();
        equal_json(
            &observed.tabs,
            r#"[{"url":"https://example.org/extension"}]"#,
            "extension native tabs route",
        );
        equal_json(
            &opened(&observed.opened),
            "[]",
            "tabs does not also open a window",
        );
    }

    /// Send `message` to the frame and await its `reply`.
    async fn exchange(&self, frame: &HtmlIFrameElement, message: &str, reply: &str) {
        self.arm(reply);
        send(frame, &object(&[("type", &message.into())]));
        self.waited().await;
    }

    /// The srcdoc frame's attributes and its normal console route.
    async fn srcdoc_route(&self, frame: &HtmlIFrameElement) {
        let attribute = |name: &str| JsValue::from(frame.get_attribute(name));
        equal(
            &attribute("sandbox"),
            &"allow-scripts allow-modals".into(),
            "sandbox flags",
        );
        let style = frame.style();
        let sizing = js_sys::Array::of3(
            &style.get_property_value("width").unwrap().into(),
            &style.get_property_value("height").unwrap().into(),
            &style.get_property_value("border-style").unwrap().into(),
        );
        equal_json(&sizing, r#"["100%","100%","none"]"#, "frame sizing");
        self.exchange(frame, "send-probe", "probe-done").await;
        let lines = console(&self.observed().borrow().messages, true);
        equal_json(
            &lines,
            r#"[["log","normal"]]"#,
            "normal console consumer route",
        );
    }

    /// Stale, foreign and unrelated messages never open; the live source does.
    fn unrelated_route(&self) {
        let observed = self.observed();
        let before = observed.borrow().opened.len();
        for source in ["previous", "sibling", "absent", "foreign"] {
            self.dispatch("open-external-url", source);
        }
        self.dispatch("unrelated", "current");
        dispatch_message(&"ordinary traffic".into(), &window());
        equal(
            &observed.borrow().opened.len().into(),
            &before.into(),
            "unrelated or stale source ignored",
        );
        self.dispatch("open-external-url", "current");
        equal(
            &observed.borrow().opened.len().into(),
            &(before + 1).into(),
            "live source accepted",
        );
    }

    /// Readiness settles the current URL load exactly once.
    async fn ready_route(&self, frame: &HtmlIFrameElement) {
        let observed = self.observed();
        assert!(
            !frame.src().is_empty() && !frame.has_attribute("srcdoc"),
            "URL not srcdoc"
        );
        equal(
            &observed.borrow().opened.len().into(),
            &0.into(),
            "nothing sent before readiness",
        );
        self.dispatch("sandbox-ready", "foreign");
        self.dispatch("sandbox-error", "absent");
        self.exchange(frame, "send-ready", "url-done").await;
        {
            let observed = observed.borrow();
            equal_json(
                &opened(&observed.opened),
                r#"[["https://example.org/extension","_blank"]]"#,
                "ready settles current URL load",
            );
            equal(
                &observed.received.len().into(),
                &1.into(),
                "one sandbox-load",
            );
            let received = &observed.received[0];
            let code = get(received, "code").as_string().unwrap_or_default();
            let id = get(received, "id").as_string().unwrap_or_default();
            assert!(
                code.contains("url branch") && id.starts_with("url-"),
                "actual id and document forwarded"
            );
        }
        let before = observed.borrow().opened.len();
        self.exchange(frame, "send-ready", "ready-sent").await;
        self.exchange(frame, "send-error", "error-sent").await;
        equal(
            &observed.borrow().opened.len().into(),
            &before.into(),
            "late ready and error cannot resettle",
        );
    }

    /// A handshake the written document replays neither loads again nor reaches consumers.
    async fn replayed_route(&self, frame: &HtmlIFrameElement) {
        self.exchange(frame, "flush", "flushed").await;
        let observed = self.observed();
        let observed = observed.borrow();
        equal(
            &observed.received.len().into(),
            &0.into(),
            "replayed readiness cannot load again",
        );
        let errors = observed
            .messages
            .iter()
            .filter(|message| message_type(message).as_deref() == Some("execution-error"))
            .count();
        equal(
            &errors.into(),
            &0.into(),
            "replayed error cannot reach consumers",
        );
    }

    /// A bootstrap error reaches the provider and consumers once, and never loads afterwards.
    async fn error_route(&self, frame: &HtmlIFrameElement) {
        let observed = self.observed();
        self.dispatch("sandbox-error", "foreign");
        self.dispatch("sandbox-error", "absent");
        self.exchange(frame, "send-error", "response-received")
            .await;
        bootstrap_error(&observed.borrow());
        self.exchange(frame, "send-ready", "ready-sent").await;
        equal(
            &observed.borrow().opened.len().into(),
            &0.into(),
            "error cannot later send sandbox-load",
        );
    }

    /// Observe the mounted validation document, or the accepted recovered input when
    /// `expected` is empty; `css` names the selected pack's expected roles.
    pub(crate) async fn validation_check(&self, expected: &str, css: &str) {
        let frame = current().unwrap();
        super::native::loaded(&frame).await;
        if expected.is_empty() {
            equal(
                &frame.get_attribute("sandbox").into(),
                &"allow-scripts allow-modals".into(),
                "native recovery is accepted",
            );
            assert!(
                !frame.srcdoc().contains("HTML Validation Error"),
                "no invented validation failure"
            );
            return;
        }
        assert!(!frame.has_attribute("sandbox"), "host error document");
        let document = frame.content_document().unwrap();
        let text = |selector: &str| {
            JsValue::from(
                document
                    .query_selector(selector)
                    .unwrap()
                    .unwrap()
                    .text_content(),
            )
        };
        equal(&text("h3"), &"HTML Validation Error".into(), "heading");
        equal(&text("pre"), &expected.into(), "literal validation text");
        let markup = document
            .query_selector_all("pre script,pre tag")
            .unwrap()
            .length();
        equal(&markup.into(), &0.into(), "error text is not markup");
        equal(
            &get(&frame.content_window().unwrap(), "executed"),
            &JsValue::UNDEFINED,
            "validation text cannot execute",
        );
        if !css.is_empty() {
            validation_styles(&frame, &document, css);
        }
    }
}

/// The bootstrap error reached the provider, its admitted reply and the source-checked consumer.
fn bootstrap_error(observed: &Observed) {
    let errors: js_sys::Array = observed
        .provider_messages
        .iter()
        .map(|message| get(message, "error"))
        .collect();
    equal_json(
        &errors,
        r#"[{"message":"bootstrap problem","stack":"native stack"}]"#,
        "provider error delivery",
    );
    let replies: js_sys::Array = observed
        .replies
        .iter()
        .map(|reply| -> JsValue {
            js_sys::Array::of3(
                &get(reply, "type"),
                &get(reply, "sandboxId"),
                &get(reply, "bootstrapAcknowledged"),
            )
            .into()
        })
        .collect();
    equal_json(
        &replies,
        r#"[["runtime-response","url-error",true]]"#,
        "normal admitted response construction",
    );
    // A consumer acknowledgement is synchronous before the forwarding promise settles.
    let errors: js_sys::Array = observed
        .messages
        .iter()
        .filter(|message| message_type(message).as_deref() == Some("execution-error"))
        .map(|message| -> JsValue {
            object(&[
                ("sandboxId", &get(message, "sandboxId")),
                ("type", &get(message, "type")),
                ("error", &get(message, "error")),
            ])
            .into()
        })
        .collect();
    equal_json(
        &errors,
        r#"[{"sandboxId":"url-error","type":"execution-error","error":{"message":"bootstrap problem","stack":"native stack"}}]"#,
        "source-checked execution error",
    );
}

/// The validation document resolves the selected pack's roles and monospaced error text.
fn validation_styles(frame: &HtmlIFrameElement, document: &web_sys::Document, css: &str) {
    let roles: serde_json::Map<String, serde_json::Value> = serde_json::from_str(css).unwrap();
    let window = frame.content_window().unwrap();
    let computed = |element: &Element| window.get_computed_style(element).unwrap().unwrap();
    let root = computed(&document.document_element().unwrap());
    for (key, value) in &roles {
        let actual = root.get_property_value(key).unwrap();
        equal(
            &actual.trim().into(),
            &value.as_str().unwrap().into(),
            &format!("selected pack {key}"),
        );
    }
    let pre = document.query_selector("pre").unwrap().unwrap();
    let (body, pre_style) = (computed(&document.body().unwrap()), computed(&pre));
    let probe: web_sys::HtmlElement = document.create_element("span").unwrap().unchecked_into();
    let family = roles["--maestro-font-mono"].as_str().unwrap();
    probe.style().set_property("font-family", family).unwrap();
    let expected = probe.style().get_property_value("font-family").unwrap();
    let family_of =
        |style: &web_sys::CssStyleDeclaration| style.get_property_value("font-family").unwrap();
    equal(
        &family_of(&body).into(),
        &expected.clone().into(),
        "selected mono family",
    );
    equal(
        &family_of(&pre_style).into(),
        &expected.into(),
        "visible validation mono family",
    );
    equal(
        &pre_style.get_property_value("white-space").unwrap().into(),
        &"pre-wrap".into(),
        "error wrapping",
    );
    equal(
        &pre_style.get_property_value("overflow-x").unwrap().into(),
        &"auto".into(),
        "error scrolling",
    );
}

/// Recorded window openings as a page array.
fn opened(openings: &[js_sys::Array]) -> js_sys::Array {
    openings.iter().collect()
}

/// Console lines among `messages`: method and text pairs, or texts alone.
fn console(messages: &[JsValue], methods: bool) -> js_sys::Array {
    messages
        .iter()
        .filter(|message| message_type(message).as_deref() == Some("console"))
        .map(|message| -> JsValue {
            if methods {
                js_sys::Array::of2(&get(message, "method"), &get(message, "text")).into()
            } else {
                get(message, "text")
            }
        })
        .collect()
}

/// Count message deliveries, refusing registration where the fault selects it.
fn registrations(window: &JsValue, observed: &Rc<RefCell<Observed>>) -> Patch {
    let observed = Rc::clone(observed);
    Patch::method(window, "addEventListener", move |original, this, args| {
        if args.get(0) != "message" {
            return original.apply(&this, &args);
        }
        let refused = {
            let mut observed = observed.borrow_mut();
            observed.adds += 1;
            observed.mode == "register"
                || (observed.mode == "register-adapter" && observed.adds == 2)
        };
        if refused {
            return Err(dom_exception(
                "controlled registration",
                "InvalidStateError",
            ));
        }
        let (callback, counted) = (args.get(1), Rc::clone(&observed));
        let target: js_sys::Function = callback.clone().unchecked_into();
        let wrapped =
            Closure::<dyn Fn(JsValue) -> Result<(), JsValue>>::new(move |event: JsValue| {
                counted.borrow_mut().deliveries += 1;
                target.call1(&JsValue::UNDEFINED, &event).map(|_| ())
            });
        let forwarded = js_sys::Array::from(&args);
        forwarded.set(1, wrapped.as_ref().clone());
        original.apply(&this, &forwarded)?;
        observed.borrow_mut().active.push((callback, wrapped));
        Ok(JsValue::UNDEFINED)
    })
}

/// Remove the counting wrapper of a registered callback, refusing in `detach` mode.
fn removals(window: &JsValue, observed: &Rc<RefCell<Observed>>) -> Patch {
    let observed = Rc::clone(observed);
    Patch::method(
        window,
        "removeEventListener",
        move |original, this, args| {
            let callback = args.get(1);
            let position = observed
                .borrow()
                .active
                .iter()
                .position(|(known, _)| *known == callback);
            let Some(position) = position.filter(|_| args.get(0) == "message") else {
                return original.apply(&this, &args);
            };
            if observed.borrow().mode == "detach" {
                return Err(dom_exception("controlled detach", "InvalidStateError"));
            }
            let forwarded = js_sys::Array::from(&args);
            forwarded.set(1, observed.borrow().active[position].1.as_ref().clone());
            original.apply(&this, &forwarded)?;
            // The page no longer holds the wrapper, so dropping it is safe.
            drop(observed.borrow_mut().active.remove(position));
            Ok(JsValue::UNDEFINED)
        },
    )
}

/// Record window openings without opening anything.
fn openings(window: &JsValue, observed: &Rc<RefCell<Observed>>) -> Patch {
    let observed = Rc::clone(observed);
    Patch::method(window, "open", move |_, _, args| {
        observed.borrow_mut().opened.push(args);
        Ok(JsValue::NULL)
    })
}

/// Mount a sibling test parent with its own frame.
fn sibling() {
    let document = crate::contributions_support::document();
    let parent = document.create_element("div").unwrap();
    parent.set_attribute("data-sandbox-test", "").unwrap();
    parent
        .append_child(&document.create_element("iframe").unwrap())
        .unwrap();
    document.body().unwrap().append_child(&parent).unwrap();
}

/// Route extension tab creation to the recorder.
fn tabs(observed: &Rc<RefCell<Observed>>) -> Patch {
    let chrome = get(&window(), "chrome");
    let chrome = if chrome.is_object() {
        chrome
    } else {
        let created = js_sys::Object::new();
        set(&window(), "chrome", &created);
        created.into()
    };
    let recorded = observed.borrow().tabs.clone();
    let create = Closure::<dyn Fn(JsValue)>::new(move |options: JsValue| {
        recorded.push(&options);
    });
    Patch::value(&chrome, "tabs", &object(&[("create", create.as_ref())])).keeping(create)
}

/// Fail document parsing while the selected parse fault stays selected.
fn parse_failure(observed: &Rc<RefCell<Observed>>) -> Patch {
    let observed = Rc::clone(observed);
    let prototype = get(&global("DOMParser"), "prototype");
    Patch::method(
        &prototype,
        "parseFromString",
        move |original, this, args| match observed.borrow().mode.as_str() {
            "parse-throw" => Err(error("controlled parse failure")),
            "parse-no-message" => Err(js_sys::Object::new().into()),
            _ => original.apply(&this, &args),
        },
    )
}

/// Fail frame creation or test-parent appends while that fault stays selected.
fn creation_failure(mode: &str, observed: &Rc<RefCell<Observed>>) -> Patch {
    let (selected, observed) = (mode.to_owned(), Rc::clone(observed));
    let (target, key): (JsValue, &str) = if mode == "create" {
        (
            crate::contributions_support::document().into(),
            "createElement",
        )
    } else {
        let parent = crate::contributions_support::document()
            .query_selector("[data-sandbox-test]")
            .unwrap()
            .unwrap();
        (parent.into(), "appendChild")
    };
    Patch::method(&target, key, move |original, this, args| {
        let refused =
            observed.borrow().mode == selected && (selected == "append" || args.get(0) == "iframe");
        if refused {
            return Err(dom_exception(
                &format!("controlled {selected}"),
                "InvalidStateError",
            ));
        }
        original.apply(&this, &args)
    })
}

/// The document [`content`] returns.
const CONTENT: &str = r#"<html><head></head><body><a href="https://example.org/fixture"><span>link</span></a><script>
window.addEventListener('message', e=>{if(e.data.type==='click-now'){document.querySelector('span').click(); parent.postMessage({type:'clicked'},'*');}if(e.data.type==='send-probe'){sendRuntimeMessage({type:'console',method:'log',text:'normal',args:[]}).then(()=>parent.postMessage({type:'probe-done'},'*'));}});
parent.postMessage({type:'loaded',first:window.firstProvider,second:window.secondProvider},'*');</script></body></html>"#;

/// The document [`url`] encodes, with `__MODE__` standing for the loading mode.
const URL: &str = r"<script>
window.addEventListener('message',e=>{
 if(e.data.type==='runtime-response'){parent.postMessage({type:'response-received',reply:e.data},'*');}
 if(e.data.type==='send-ready'){parent.postMessage({type:'sandbox-ready'},'*');parent.postMessage({type:'ready-sent'},'*');}
 if(e.data.type==='send-error'){parent.postMessage({type:'sandbox-error',error:'bootstrap problem',stack:'native stack'},'*');parent.postMessage({type:'error-sent'},'*');}
 if(e.data.type==='sandbox-load'){parent.postMessage({type:'code-received',id:e.data.sandboxId,code:e.data.code},'*');parent.postMessage({type:'open-external-url',url:'https://example.org/extension'},'*');parent.postMessage({type:'url-done'},'*');}
});parent.postMessage({type:'url-mounted',mode:__MODE__},'*');</script>";

/// The probe document a srcdoc load mounts: inside the isolated frame it clicks its nested
/// link or sends a console line on request, and reports its providers when loaded.
pub(crate) fn content() -> &'static str {
    CONTENT
}

/// A URL bootstrap whose handshake the test drives: inside the isolated frame it reports what
/// it receives and sends readiness or an error on request.
pub(crate) fn url(mode: &str) -> String {
    let document = URL.replace("__MODE__", &serde_json::Value::from(mode).to_string());
    format!(
        "data:text/html,{}",
        String::from(js_sys::encode_uri_component(&document))
    )
}
