//! Author event channels exercised through both component adapters.
use super::bus_host::Emitter;
use super::controlled::Controlled;
use super::scenario::Driver;
use crate::ExtensionResult;
use crate::component_adapter::{Exports, Imports, events, release};
use serde::Deserialize;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// Oracle corpus; every field drives an assertion or operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Unique literal queries.
    inputs: Vec<Case>,
    /// Separate signed-zero assertion.
    #[serde(rename = "negativeZero")]
    negative_zero: SignedZero,
}
/// One independently recorded query and expected delivery sequence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Assertion attribution.
    name: String,
    /// Literal channel.
    channel: String,
    /// Delivered data.
    input: Value,
    /// Ordered received payloads.
    expected: Vec<Value>,
}
/// Floating-point sign witness.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SignedZero {
    /// Numeric input syntax.
    input: String,
    /// Expected negative sign bit.
    expected: bool,
}
/// Consumes the complete source corpus through registered author callbacks.
async fn literal_cases(driver: &mut impl Driver) -> ExtensionResult<()> {
    let corpus: Corpus =
        serde_json::from_str(include_str!("bus_corpus.json")).map_err(|e| e.to_string())?;
    let mut queries = HashSet::new();
    for case in corpus.inputs {
        assert!(
            queries.insert((case.channel.clone(), case.input.to_string())),
            "duplicate corpus query"
        );
        let before = driver.transcript().len();
        let request = json!({"channel":case.channel,"input":case.input});
        driver
            .run_plain_command("capture", &format!("bus {request}"))
            .await?;
        let received: Vec<Value> = driver.transcript()[before..]
            .iter()
            .filter_map(|line| line.strip_prefix("entry bus-prefix "))
            .map(|data| serde_json::from_str(data).expect("recorded payload"))
            .collect();
        assert_eq!(received, case.expected, "{}", case.name);
        let emitted = driver.bus_emissions();
        assert_eq!(
            emitted[emitted.len() - 2..],
            [
                (case.channel.clone(), case.input.clone()),
                ("received".into(), case.input.clone())
            ],
            "{}",
            case.name
        );
        if let Some(input) = case.input.as_object() {
            let callback = emitted.last().expect("callback emission");
            assert_eq!(callback.0, "received");
            assert_eq!(
                callback
                    .1
                    .as_object()
                    .expect("object emission")
                    .keys()
                    .collect::<Vec<_>>(),
                input.keys().collect::<Vec<_>>(),
                "callback-emitted key order: {}",
                case.name
            );
        }
        for (actual, expected) in received.iter().zip(&case.expected) {
            if let (Some(actual), Some(expected)) = (actual.as_object(), expected.as_object()) {
                assert_eq!(
                    actual.keys().collect::<Vec<_>>(),
                    expected.keys().collect::<Vec<_>>(),
                    "{}",
                    case.name
                );
            }
        }
    }
    signed_zero(driver, corpus.negative_zero).await
}
/// Checks the numeric sign separately from JSON equality.
async fn signed_zero(driver: &mut impl Driver, signed: SignedZero) -> ExtensionResult<()> {
    let input: Value = serde_json::from_str(&signed.input).map_err(|e| e.to_string())?;
    let before = driver.transcript().len();
    driver
        .run_plain_command(
            "capture",
            &format!("bus {}", json!({"channel":"signed","input":input})),
        )
        .await?;
    let transcript = driver.transcript();
    let received = transcript[before..]
        .iter()
        .find_map(|line| line.strip_prefix("entry bus-prefix "))
        .expect("signed-zero callback");
    let value: Value = serde_json::from_str(received).map_err(|e| e.to_string())?;
    assert_eq!(
        value.as_f64().expect("number").is_sign_negative(),
        signed.expected
    );
    let emitted = driver.bus_emissions();
    let callback = emitted.last().expect("signed-zero callback emission");
    assert_eq!(callback.0, "received");
    assert_eq!(
        callback
            .1
            .as_f64()
            .expect("number emission")
            .is_sign_negative(),
        signed.expected,
        "callback-emitted signed zero"
    );
    Ok(())
}
#[test]
fn maestro_bus_channels_and_payloads_remain_literal() -> ExtensionResult<()> {
    let component = super::path_of_component()?;
    let mut driver = super::ComponentDriver::new(component);
    super::block_on(async {
        driver.start().await?;
        literal_cases(&mut driver).await
    })
}
#[test]
fn maestro_controlled_bus_channels_and_payloads_remain_literal() -> ExtensionResult<()> {
    let mut driver = super::ControlledDriver::new();
    super::block_on(async {
        driver.start().await?;
        literal_cases(&mut driver).await
    })
}

#[test]
fn maestro_bus_teardown_releases_announced_tails_once() -> ExtensionResult<()> {
    let host = Controlled::default();
    let bus = events(host.clone());
    bus.on("tail", Rc::new(|_, _| Ok(Some(Box::pin(async { Ok(()) })))))?;
    super::block_on(bus.emit("tail", Value::Null))?;
    let tail = host.observed().bus.tails[0];
    loop {
        let next = host.observe().next_to_release();
        let Some((_, key)) = next else { break };
        release(key);
    }
    assert_dropped_once(&host, tail);
    Ok(())
}

#[test]
fn maestro_bus_keeps_prefix_tail_and_unsubscribe_ownership() -> ExtensionResult<()> {
    let host = Controlled::default();
    let bus = events(host.clone());
    let calls = Rc::new(RefCell::new(Vec::new()));
    let captured = Rc::clone(&calls);
    let subscription = bus.on(
        "prefix",
        Rc::new(move |data, _| {
            captured.borrow_mut().push(data.clone());
            if data == json!("none") {
                return Ok(None);
            }
            let captured = Rc::clone(&captured);
            Ok(Some(Box::pin(async move {
                captured.borrow_mut().push(json!("tail"));
                Ok(())
            })))
        }),
    )?;
    let listener = host.observed().bus.listeners[0].1;
    let alias = subscription.clone();
    drop(subscription);
    super::block_on(bus.emit("prefix", json!("none")))?;
    super::block_on(bus.emit("prefix", json!("ready")))?;
    assert_eq!(*calls.borrow(), [json!("none"), json!("ready")]);
    let tail = host.observed().bus.tails[0];
    alias.unsubscribe()?;
    alias.unsubscribe()?;
    release(listener);
    super::block_on(Exports::<Controlled>::invoke_tail(tail))?;
    assert_eq!(
        *calls.borrow(),
        [json!("none"), json!("ready"), json!("tail")]
    );
    assert_eq!(host.observed().bus.removals, [listener, listener]);
    assert_eq!(
        super::block_on(Exports::<Controlled>::invoke_tail(tail)),
        Err(format!("no pending continuation for identity {tail}"))
    );
    release(tail);
    duplicate_registrations(&host, &bus)
}

/// Repeated registration of one callable owns distinct removable identities.
fn duplicate_registrations(host: &Controlled, bus: &crate::EventBus) -> ExtensionResult<()> {
    let calls = Rc::new(std::cell::Cell::new(0));
    let count = Rc::clone(&calls);
    let listener: crate::EventListener = Rc::new(move |_, _| {
        count.set(count.get() + 1);
        Ok(None)
    });
    let first = bus.on("duplicate", Rc::clone(&listener))?;
    let second = bus.on("duplicate", listener)?;
    let ids: Vec<u32> = host
        .observed()
        .bus
        .listeners
        .iter()
        .map(|(_, id)| *id)
        .collect();
    assert_ne!(ids[0], ids[1]);
    super::block_on(bus.emit("duplicate", Value::Null))?;
    assert_eq!(calls.get(), 2);
    first.unsubscribe()?;
    super::block_on(bus.emit("duplicate", Value::Null))?;
    assert_eq!(calls.get(), 3);
    second.unsubscribe()?;
    for id in ids {
        release(id);
    }
    Ok(())
}

/// Preparation's emission occurs before its success or authored failure is returned.
async fn preparation_emission(driver: &mut impl Driver) -> ExtensionResult<()> {
    let mut config = super::tool_callbacks::configuration();
    driver
        .run_plain_command("capture", &format!("tools {config}"))
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(&driver.prepare("echo", r#"{"mark":"before"}"#).await?)
            .unwrap(),
        json!({"mark":"after"})
    );
    assert!(driver.bus_emissions().is_empty());
    release_preparation(driver).await;
    driver.run_plain_command("capture", "bus-prepare").await?;
    config["emissions"] = json!([false, {"nested":[3,1,3]}]);
    driver
        .run_plain_command("capture", &format!("tools {config}"))
        .await?;
    let result = driver.prepare("echo", r#"{"mark":"before"}"#).await?;
    assert_eq!(
        serde_json::from_str::<Value>(&result).unwrap(),
        json!({"mark":"after"})
    );
    assert_eq!(
        driver.bus_emissions(),
        [
            ("prepared-event".into(), json!(false)),
            ("prepare-nested".into(), json!(false)),
            ("prepared-event".into(), json!({"nested":[3,1,3]})),
            ("prepare-nested".into(), json!({"nested":[3,1,3]})),
        ]
    );
    let transcript = driver.transcript();
    let nested = transcript
        .iter()
        .position(|line| line == "entry prepare-nested false")
        .expect("nested preparation callback");
    let returned = transcript
        .iter()
        .rposition(|line| line == r#"entry prepared {"mark":"after"}"#)
        .expect("preparation result recorded");
    assert!(nested < returned);
    release_preparation(driver).await;
    preparation_failures(driver, config).await
}
/// Releases the callbacks registered by the preparation scenario.
async fn release_preparation(driver: &mut impl Driver) {
    assert!(driver.release("prepare echo").await);
    assert!(driver.release("tool echo").await);
}
/// Checks effects surviving authored failure and caught host rejection.
async fn preparation_failures(driver: &mut impl Driver, mut config: Value) -> ExtensionResult<()> {
    config["prepareError"] = json!("authored after emission Ω");
    driver
        .run_plain_command("capture", &format!("tools {config}"))
        .await?;
    assert_eq!(
        driver.prepare("echo", "{}").await,
        Err("authored after emission Ω".into())
    );
    assert_eq!(driver.bus_emissions().len(), 8);
    release_preparation(driver).await;
    config
        .as_object_mut()
        .expect("configuration object")
        .shift_remove("prepareError");
    config["catchEmission"] = json!(true);
    config["emissions"] = json!([7]);
    driver.reject_bus_emission(Some("host emission rejected Ω".into()));
    driver
        .run_plain_command("capture", &format!("tools {config}"))
        .await?;
    assert_eq!(
        serde_json::from_str::<Value>(&driver.prepare("echo", r#"{"mark":"before"}"#).await?)
            .unwrap(),
        json!({"mark":"after"})
    );
    assert!(
        driver
            .transcript()
            .iter()
            .any(|line| line == "entry caught-emission \"host emission rejected Ω\"")
    );
    assert_eq!(driver.bus_emissions().len(), 8);
    driver.reject_bus_emission(None);
    release_preparation(driver).await;
    Ok(())
}
#[test]
fn maestro_prepare_arguments_can_emit_before_return() -> ExtensionResult<()> {
    on_both_adapters!(preparation_emission)
}

/// Reports destruction by emitting through the same host, exercising table reentry.
pub(super) struct Reenter {
    /// Alternate adapter retained by the capture.
    pub(super) host: Controlled,
}
impl Drop for Reenter {
    fn drop(&mut self) {
        self.host.observe().bus.registration_error = None;
        let registration = events(self.host.clone())
            .on("during-drop", Rc::new(|_, _| Ok(None)))
            .expect("reentrant registration");
        registration.unsubscribe().expect("reentrant removal");
        let id = self
            .host
            .observed()
            .bus
            .removals
            .last()
            .copied()
            .expect("removed identity");
        release(id);
    }
}
#[test]
fn maestro_bus_callbacks_release_on_errors_and_reentry() -> ExtensionResult<()> {
    let host = Controlled::default();
    let bus = events(host.clone());
    host.observe().bus.registration_error = Some("registration denied Ω".into());
    let guard = Reenter { host: host.clone() };
    let listener: crate::EventListener = Rc::new(move |_, _| {
        let _ = &guard;
        Ok(None)
    });
    assert_eq!(
        bus.on("reject", listener).err(),
        Some("registration denied Ω".into())
    );
    assert_eq!(host.observed().dropped, [1, 2]);
    assert_eq!(host.observed().bus.removals, [2]);
    let guard = Reenter { host: host.clone() };
    let subscription = bus.on(
        "accepted",
        Rc::new(move |_, _| {
            let _ = &guard;
            Ok(None)
        }),
    )?;
    let id = host.observed().bus.listeners[0].1;
    release(id);
    assert_eq!(host.observed().bus.removals, [2, 4]);
    drop(subscription);
    tail_capture_release(&host, &bus)
}
/// Announcement rollback and unused-tail release permit destructor reentry.
fn tail_capture_release(host: &Controlled, bus: &crate::EventBus) -> ExtensionResult<()> {
    let pending_capture = Rc::new(RefCell::new(Some(Reenter { host: host.clone() })));
    let captured = Rc::clone(&pending_capture);
    let accepted = bus.on(
        "tail",
        Rc::new(move |_, _| {
            let guard = captured.borrow_mut().take().expect("capture for tail");
            Ok(Some(Box::pin(async move {
                drop(guard);
                Ok(())
            })))
        }),
    )?;
    let listener_id = host
        .observed()
        .bus
        .listeners
        .last()
        .expect("listener retained")
        .1;
    host.observe().bus.tail_error = Some("announcement denied Ω".into());
    assert_eq!(
        super::block_on(bus.emit("tail", Value::Null)),
        Err("announcement denied Ω".into())
    );
    assert!(pending_capture.borrow().is_none());
    assert!(host.observed().bus.tails.is_empty());
    assert_eq!(host.observed().bus.removals.last(), Some(&7));
    assert_dropped_once(host, 6);
    assert!(
        host.observed()
            .bus
            .listeners
            .iter()
            .any(|(_, id)| *id == listener_id)
    );
    pending_capture.replace(Some(Reenter { host: host.clone() }));
    host.observe().bus.tail_error = None;
    super::block_on(bus.emit("tail", Value::Null))?;
    let unused = host.observed().bus.tails[0];
    release(unused);
    assert!(
        host.observed()
            .bus
            .removals
            .last()
            .is_some_and(|id| *id > unused)
    );
    assert_dropped_once(host, unused);
    accepted.unsubscribe()?;
    release(listener_id);
    Ok(())
}

#[test]
fn maestro_bus_errors_leave_later_calls_usable() -> ExtensionResult<()> {
    let emitter = Emitter { key: 0 };
    let host = Controlled::default();
    let bus = events(host.clone());
    let calls = Rc::new(RefCell::new(0));
    let count = Rc::clone(&calls);
    let subscription = bus.on(
        "channel",
        Rc::new(move |data, emitter| {
            *count.borrow_mut() += 1;
            if data == json!("prefix-error") {
                emitter.emit("completed", json!(7))?;
                return Err("prefix failure Ω".into());
            }
            if data == json!("tail-error") {
                return Ok(Some(Box::pin(async { Err("tail failure Ω".into()) })));
            }
            Ok(None)
        }),
    )?;
    let id = host.observed().bus.listeners[0].1;
    let exports = Exports::new(host.clone());
    assert!(exports.invoke_listener(id, "{", &emitter).is_err());
    assert_eq!(*calls.borrow(), 0);
    exports.invoke_listener(id, "null", &emitter)?;
    exports.invoke_listener(id, "false", &emitter)?;
    assert_eq!(*calls.borrow(), 2);
    assert_eq!(
        exports.invoke_listener(id, "\"prefix-error\"", &emitter),
        Err("prefix failure Ω".into())
    );
    assert_eq!(
        host.observed().bus.emissions,
        [("completed".into(), json!(7))]
    );
    exports.invoke_listener(id, "\"tail-error\"", &emitter)?;
    let tail = host.observed().bus.tails[0];
    assert_eq!(
        super::block_on(Exports::<Controlled>::invoke_tail(tail)),
        Err("tail failure Ω".into())
    );
    assert_eq!(
        exports.invoke_listener(tail, "null", &emitter),
        Err(format!("no callback registered for identity {tail}"))
    );
    assert_eq!(
        super::block_on(Exports::<Controlled>::invoke_tail(tail)),
        Err(format!("no pending continuation for identity {tail}"))
    );
    emission_failures(&host, &bus, &subscription);
    super::block_on(bus.emit("channel", json!(false)))?;
    assert_eq!(*calls.borrow(), 5);
    subscription.unsubscribe()?;
    release(id);
    release(tail);
    assert_eq!(
        exports.invoke_listener(id, "null", &emitter),
        Err(format!("no callback registered for identity {id}"))
    );
    Ok(())
}
/// Failed host requests leave the retained registration usable.
fn emission_failures(host: &Controlled, bus: &crate::EventBus, subscription: &crate::Subscription) {
    host.observe().bus.emission_error = Some("emit rejected Ω".into());
    assert_eq!(
        super::block_on(bus.emit("channel", json!(0))),
        Err("emit rejected Ω".into())
    );
    assert_eq!(
        host.callback_emit(&Emitter { key: 0 }, "channel", "0"),
        Err("emit rejected Ω".into())
    );
    host.observe().bus.emission_error = None;
    host.observe().bus.removal_error = Some("removal rejected Ω".into());
    assert_eq!(subscription.unsubscribe(), Err("removal rejected Ω".into()));
    host.observe().bus.removal_error = None;
}

#[test]
fn maestro_component_bus_uses_borrowed_emitters_and_native_async_tails() -> ExtensionResult<()> {
    super::block_on(async {
        let path = super::build::author_component()?;
        let mut harness = super::host::Harness::start(path)
            .await
            .map_err(|e| format!("{e:#}"))?;
        assert_eq!(
            component_emission(
                &mut harness,
                json!({"channel":"failure","input":"prefix-error"}),
                false
            )
            .await,
            Err("component prefix failed Ω".into())
        );
        component_emission(
            &mut harness,
            json!({"channel":"outer","input":[3,1,3]}),
            true,
        )
        .await?;
        let transcript = &harness.store.data().observed.transcript;
        let prefix = transcript
            .iter()
            .position(|s| s == "entry bus-prefix [3,1,3]")
            .expect("prefix ran");
        let nested = transcript
            .iter()
            .position(|s| s == "entry bus-nested [3,1,3]")
            .expect("nested callback ran inside emitter import");
        assert!(prefix < nested);
        assert!(!transcript.iter().any(|s| s.starts_with("entry bus-tail")));
        let tails = harness.store.data().observed.bus.tails.clone();
        assert_eq!(tails.len(), 1);
        let guest = harness.exports.clone();
        harness
            .store
            .run_concurrent(async |accessor| {
                guest
                    .call_invoke_tail(accessor, super::host::borrow(tails[0]))
                    .await
            })
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())??;
        assert!(
            harness
                .store
                .data()
                .observed
                .transcript
                .iter()
                .any(|s| s == "entry bus-tail \"done\"")
        );
        harness
            .release_all()
            .await
            .into_iter()
            .try_for_each(|(_, outcome)| outcome.map_err(|e| e.to_string()))?;
        Ok(())
    })
}

/// Drives ordinary emission through an actual command export.
async fn component_emission(
    harness: &mut super::host::Harness,
    request: Value,
    suspend: bool,
) -> ExtensionResult<()> {
    let handler = harness
        .callback("command capture")
        .map_err(|e| e.to_string())?;
    let ctx = harness
        .command_context("/work")
        .map_err(|e| e.to_string())?;
    let guest = harness.exports.clone();
    let (hold, pending, open) = super::observed::gate();
    if suspend {
        harness.observed().hold_next_idle(hold);
    }
    let result = harness
        .store
        .run_concurrent(async |accessor| {
            let call = guest.call_invoke_command(
                accessor,
                super::host::borrow(handler),
                format!("bus {request}"),
                ctx,
            );
            super::join::alongside(
                call,
                super::host::release_held(accessor, suspend.then_some((pending, open))),
            )
            .await
        })
        .await
        .map_err(|e| format!("{e:#}"))?
        .map_err(|e| format!("{e:#}"))?;
    result?;
    Ok(())
}

thread_local! {
    /// Factory's API retained for alias tests.
    static CAPTURED_API: RefCell<Option<crate::ExtensionAPI>> = const { RefCell::new(None) };
}
/// Factory exposing the actual adapter-backed API.
struct CaptureApi;
impl crate::Extension for CaptureApi {
    fn load(api: crate::ExtensionAPI) -> crate::ExtensionFuture<'static, ()> {
        CAPTURED_API.with(|slot| slot.replace(Some(api)));
        Box::pin(async { Ok(()) })
    }
}
/// The actual adapter-backed API a factory receives from `host`.
pub(super) fn captured_api(host: &Controlled) -> ExtensionResult<crate::ExtensionAPI> {
    super::block_on(Exports::new(host.clone()).start::<CaptureApi>())?;
    Ok(CAPTURED_API.with(RefCell::take).expect("captured API"))
}
#[test]
fn maestro_bus_facades_retain_the_same_host_resources() -> ExtensionResult<()> {
    let host = Controlled::default();
    let api = captured_api(&host)?;
    let alias = api.clone();
    let bus = api.events();
    let bus_alias = bus.clone();
    drop(api);
    drop(bus);
    host.observe().bus.emission_error = Some("live host rejected Ω".into());
    assert_eq!(
        super::block_on(bus_alias.emit("state", json!(1))),
        Err("live host rejected Ω".into())
    );
    // The infallible getter still returns the same host while another operation fails.
    assert!(
        alias
            .on("rejected", Rc::new(|_, _| Box::pin(async { Ok(None) })))
            .is_err()
    );
    let later = alias.events();
    assert_eq!(
        super::block_on(later.emit("state", json!(2))),
        Err("live host rejected Ω".into())
    );
    host.observe().bus.emission_error = None;
    super::block_on(bus_alias.emit("state", json!(3)))?;
    assert_eq!(host.observed().bus.emissions, [("state".into(), json!(3))]);
    let subscription = later.on("retained", Rc::new(|_, _| Ok(None)))?;
    let id = host.observed().bus.listeners[0].1;
    let alias_subscription = subscription.clone();
    drop(subscription);
    assert_eq!(
        host.observed()
            .bus
            .subscription_drops
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    drop(alias_subscription);
    assert_eq!(
        host.observed()
            .bus
            .subscription_drops
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
    assert_eq!(host.observed().bus.listeners, [("retained".into(), id)]);
    assert!(host.observed().bus.removals.is_empty());
    release(id);
    drop(alias);
    drop(later);
    drop(bus_alias);
    assert_eq!(host.observed().dropped, [1, id]);
    Ok(())
}

#[test]
fn maestro_listener_tail_keeps_its_origin_and_can_resume_its_emitter() -> ExtensionResult<()> {
    super::block_on(async {
        let host = Controlled::default();
        let bus = events(host.clone());
        let (first_open, first_gate) = tokio::sync::oneshot::channel::<()>();
        let (second_open, second_gate) = tokio::sync::oneshot::channel::<()>();
        let (first_entered, first_seen) = tokio::sync::oneshot::channel::<()>();
        let (second_entered, second_seen) = tokio::sync::oneshot::channel::<()>();
        let gates = Rc::new(RefCell::new(std::collections::VecDeque::from([
            (first_gate, first_entered),
            (second_gate, second_entered),
        ])));
        let captured_bus = bus.clone();
        let registration = bus.on(
            "start",
            Rc::new(move |data, _| {
                let (gate, entered) = gates
                    .borrow_mut()
                    .pop_front()
                    .expect("one gate per invocation");
                let bus = captured_bus.clone();
                Ok(Some(Box::pin(finish_origin(bus, data, gate, entered))))
            }),
        )?;
        let listener = host.observed().bus.listeners[0].1;
        bus.emit("start", json!("first")).await?;
        bus.emit("start", json!("second")).await?;
        let tails = host.observed().bus.tails.clone();
        registration.unsubscribe()?;
        release(listener);
        finish_tails(
            &host,
            &tails,
            (first_seen, second_seen),
            (first_open, second_open),
        )
        .await?;
        for tail in tails {
            release(tail);
        }
        Ok(())
    })
}

/// Joins pending tails, opening the older invocation first.
async fn finish_tails(
    host: &Controlled,
    tails: &[u32],
    seen: (
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Receiver<()>,
    ),
    open: (
        tokio::sync::oneshot::Sender<()>,
        tokio::sync::oneshot::Sender<()>,
    ),
) -> ExtensionResult<()> {
    let (first_seen, second_seen) = seen;
    let (first_open, second_open) = open;
    let local = tokio::task::LocalSet::new();
    local
        .run_until(async {
            let first = tokio::task::spawn_local(Exports::<Controlled>::invoke_tail(tails[0]));
            let second = tokio::task::spawn_local(Exports::<Controlled>::invoke_tail(tails[1]));
            first_seen.await.expect("first tail entered");
            second_seen.await.expect("second tail entered");
            first_open.send(()).expect("first still pending");
            first.await.expect("first joined")?;
            assert_eq!(
                host.observed().bus.emissions.last(),
                Some(&("completed".into(), json!("first")))
            );
            second_open.send(()).expect("second still pending");
            assert_eq!(
                second.await.expect("second joined"),
                Err("second suspended tail failed Ω".into())
            );
            assert_eq!(
                &host.observed().bus.emissions[2..],
                [
                    ("completed".into(), json!("first")),
                    ("completed".into(), json!("second"))
                ]
            );
            Ok::<_, String>(())
        })
        .await?;
    Ok(())
}

/// Checks ownership teardown for one observed identity.
fn assert_dropped_once(host: &Controlled, identity: u32) {
    assert_eq!(
        host.observed()
            .dropped
            .iter()
            .filter(|id| **id == identity)
            .count(),
        1
    );
}

/// Resumes one captured invocation, preserving its data and eventual error.
async fn finish_origin(
    bus: crate::EventBus,
    data: Value,
    gate: tokio::sync::oneshot::Receiver<()>,
    entered: tokio::sync::oneshot::Sender<()>,
) -> ExtensionResult<()> {
    entered.send(()).expect("completion witness waiting");
    gate.await.map_err(|e| e.to_string())?;
    let rejected = data == json!("second");
    bus.emit("completed", data).await?;
    if rejected {
        return Err("second suspended tail failed Ω".into());
    }
    Ok(())
}
