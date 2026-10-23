//! Retained message-renderer calls through real registrations and both callback families.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
#[path = "../../maestro-chat/tests/message_support/palette.rs"]
mod palette;

use crate::local_support::{
    LocalBus, Released, Trace, listener, loaded, loaded_for, local, mark, runner, runner_for,
};
use maestro_extensions::{
    CallbackEmitter, EventBus, ExtensionRunner, MessageRenderOptions, MessageRenderer,
    create_event_bus_for,
};
use maestro_models::{Local, Threaded};
use maestro_session::Presence;
use maestro_session::messages::CustomMessage;
use maestro_theme::{LiveTheme, ThemeColor, paint};
use maestro_tui::Text;
use maestro_tui::tui::ComponentHandle;
use serde::Deserialize;
use serde_json::{Number, Value};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::{Arc, Mutex},
};

/// One recorded details-domain case.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Domain {
    /// Case name.
    kind: String,
    /// Recorded message input.
    input: DomainInput,
    /// What the callback must observe.
    expected: Value,
}

/// Recorded details and signed-zero flag.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DomainInput {
    /// Details property, keeping absence apart from null.
    #[serde(default)]
    details: Presence<Value>,
    /// Whether the recorded number input was negative zero.
    negative_zero: bool,
}

/// Record fields every recorded observation carries.
const OBSERVED_FIELDS: &[&str] = &[
    "present",
    "details",
    "negative_zero",
    "custom_type",
    "display",
    "timestamp",
    "expanded",
    "theme_color",
];

/// Rejects an unrecorded observation field or a missing shared one.
fn observed_fields(expected: &Value, kind: &str) -> Value {
    let fields = expected.as_object().expect("recorded observation");
    for key in fields.keys() {
        assert!(
            OBSERVED_FIELDS.contains(&key.as_str()),
            "{kind}: unrecorded field {key}"
        );
    }
    for key in OBSERVED_FIELDS {
        assert!(
            *key == "details" || fields.contains_key(*key),
            "{kind}: missing field {key}"
        );
    }
    expected.clone()
}

/// A custom message with the supplied details property.
fn message<T>(details: Presence<T>) -> CustomMessage<T> {
    CustomMessage {
        custom_type: "note".into(),
        content: maestro_models::UserContent::Text("body".into()),
        display: false,
        details,
        timestamp: 1.5,
    }
}

/// The recorded details property for one domain case.
fn details(case: &DomainInput) -> Presence<Value> {
    if case.negative_zero {
        return Presence::Present(Value::Number(Number::from_f64(-0.0).expect("signed zero")));
    }
    case.details.clone()
}

/// Builds the recording palette's live theme.
fn theme() -> LiveTheme {
    LiveTheme::from_theme(palette::palette("a"))
}

/// Registers a renderer that records what it receives and selects one outcome.
fn recording(
    trace: &Trace,
    theme_color: &Arc<Mutex<Vec<String>>>,
    outcome: Outcome,
) -> MessageRenderer<Value, Local> {
    let trace = trace.clone();
    let sampled = Arc::clone(theme_color);
    Rc::new(
        move |message: Rc<CustomMessage<Value>>,
              options: MessageRenderOptions,
              theme: LiveTheme,
              _: CallbackEmitter<'_, Local>| {
            sampled.lock().expect("sample").push(paint(
                theme.get().ok().as_deref(),
                &ThemeColor::CustomMessageText,
                "sample",
            ));
            mark(
                &trace,
                format!(
                    "{} {} {} {}",
                    message.custom_type, options.expanded, message.display, message.timestamp
                ),
            );
            match outcome {
                Outcome::Component => Ok(Some(
                    Rc::new(Text::new("RENDERED".into(), 0, 0, None)) as ComponentHandle
                )),
                Outcome::None => Ok(None),
                Outcome::Error => Err("creation failed".into()),
            }
        },
    )
}

/// The creation outcome a recording renderer selects.
#[derive(Clone, Copy)]
enum Outcome {
    /// A component of its own.
    Component,
    /// No component.
    None,
    /// A failure.
    Error,
}

/// Real registration to runner lookup keeps details, options, theme and original errors.
#[test]
fn registered_calls_keep_typed_details_theme_and_original_errors() {
    let cases: Vec<Domain> =
        serde_json::from_str(include_str!("fixtures/message_renderer_inputs.json"))
            .expect("recorded domains parse");
    assert_eq!(cases.len(), 8);
    let live = theme();
    for case in &cases {
        check_domain(case, &live);
    }
    registration_outcomes();
    non_clone_details();
}

/// Replays one recorded details-domain case through a real registration.
fn check_domain(case: &Domain, live: &LiveTheme) {
    let (bus, trace) = (LocalBus::new(), Trace::default());
    let sampled = Arc::new(Mutex::new(Vec::new()));
    let owner = loaded("/domain", bus.controller.for_extension());
    let runner = runner(&owner.owners);
    owner
        .api
        .register_message_renderer("note".into(), recording(&trace, &sampled, Outcome::None))
        .expect("registration");
    let call = runner
        .get_message_renderer_call::<Value>("note")
        .expect("registered call");
    let message = Rc::new(message(details(&case.input)));
    let rendered = call
        .call(
            message.clone(),
            MessageRenderOptions { expanded: false },
            live.clone(),
        )
        .expect("rendered");
    assert!(rendered.is_none(), "{} keeps the default body", case.kind);
    check_observation(case, &message.details, &sampled);
    assert_eq!(
        *trace.borrow(),
        [format!(
            "{} {} {} {}",
            case.expected["custom_type"].as_str().unwrap_or_default(),
            case.expected["expanded"],
            case.expected["display"],
            case.expected["timestamp"]
        )]
    );
}

/// Compares what the callback observed with the recorded observation.
fn check_observation(case: &Domain, presence: &Presence<Value>, sampled: &Arc<Mutex<Vec<String>>>) {
    let seen = match presence {
        Presence::Present(value) => value.clone(),
        Presence::Missing | Presence::Null => Value::Null,
    };
    let mut observed = serde_json::json!({
        "present": !presence.is_missing(),
        "details": seen,
        "negative_zero": seen
            .as_f64()
            .is_some_and(|value| value == 0.0 && value.is_sign_negative()),
        "custom_type": case.expected["custom_type"],
        "display": case.expected["display"],
        "timestamp": case.expected["timestamp"],
        "expanded": case.expected["expanded"],
        "theme_color": sampled.lock().expect("sample").first().cloned(),
    });
    if observed["negative_zero"] == Value::Bool(true) {
        observed["details"] = Value::from(0);
    }
    if !case
        .expected
        .as_object()
        .expect("recorded")
        .contains_key("details")
    {
        observed
            .as_object_mut()
            .expect("observation")
            .shift_remove("details");
    }
    assert_eq!(
        observed,
        observed_fields(&case.expected, &case.kind),
        "{}",
        case.kind
    );
}

/// Forwarding preserves the remaining supplied fields and returns the original error.
fn registration_outcomes() {
    let (bus, trace) = (LocalBus::new(), Trace::default());
    let sampled = Arc::new(Mutex::new(Vec::new()));
    let owner = loaded("/outcomes", bus.controller.for_extension());
    let runner = runner(&owner.owners);
    for (kind, outcome) in [
        ("component", Outcome::Component),
        ("none", Outcome::None),
        ("error", Outcome::Error),
    ] {
        owner
            .api
            .register_message_renderer(kind.into(), recording(&trace, &sampled, outcome))
            .expect("registration");
        let call = runner
            .get_message_renderer_call::<Value>(kind)
            .expect("registered call");
        let result = call.call(
            Rc::new(message(Presence::Present(Value::Null))),
            MessageRenderOptions { expanded: true },
            theme(),
        );
        match outcome {
            Outcome::Component => assert!(result.expect("rendered").is_some()),
            Outcome::None => assert!(result.expect("rendered").is_none()),
            Outcome::Error => {
                let Err(error) = result else {
                    panic!("a creation failure is returned");
                };
                assert_eq!(error.to_string(), "creation failed");
            }
        }
    }
    assert_eq!(
        trace.borrow().as_slice(),
        [
            "note true false 1.5",
            "note true false 1.5",
            "note true false 1.5"
        ]
    );
    let live = theme();
    assert!(live.get().is_ok());
}

/// A payload whose type cannot be cloned stays usable through a projected call.
fn non_clone_details() {
    /// A local payload without a clone operation.
    struct Payload {
        /// Observed text.
        text: String,
    }
    let (bus, trace) = (LocalBus::new(), Trace::default());
    let owner = loaded("/non-clone", bus.controller.for_extension());
    let runner = runner(&owner.owners);
    let recorded = trace.clone();
    let renderer: MessageRenderer<Payload, Local> = Rc::new(
        move |message: Rc<CustomMessage<Payload>>,
              _options: MessageRenderOptions,
              _theme: LiveTheme,
              _: CallbackEmitter<'_, Local>| {
            if let Presence::Present(payload) = &message.details {
                mark(&recorded, payload.text.clone());
            }
            Ok(None)
        },
    );
    owner
        .api
        .register_message_renderer("typed".into(), renderer)
        .expect("registration");
    let call = runner
        .get_message_renderer_call::<Payload>("typed")
        .expect("registered call");
    let message = Rc::new(message(Presence::Present(Payload {
        text: "body".into(),
    })));
    assert!(
        call.call(message, MessageRenderOptions { expanded: false }, theme())
            .expect("rendered")
            .is_none()
    );
    assert_eq!(*trace.borrow(), ["body"]);
    assert!(
        runner
            .get_message_renderer_call::<String>("typed")
            .is_none()
    );
    assert!(
        runner
            .get_message_renderer_call::<Value>("missing")
            .is_none()
    );
}

/// The projected call uses its registering bus and outlives a replacement.
#[test]
fn projected_calls_capture_registration_bus_and_callback() {
    local(async {
        let registering = LocalBus::new();
        let owner_events = registering.controller.for_extension();
        let owner = loaded("/bus", owner_events.clone());
        let runner = Rc::new(runner(&owner.owners));
        let trace = Trace::default();
        let reentered = Trace::default();
        let guard = Rc::clone(&runner);
        let query = reentered.clone();
        let same = trace.clone();
        owner_events.on(
            "channel",
            Rc::new(move |_, _| {
                mark(&same, "same");
                assert!(guard.get_message_renderer_call::<Value>("note").is_some());
                mark(&query, "reentered");
                Ok(None)
            }),
        );
        bystanders(&registering, &trace);
        owner
            .api
            .register_message_renderer("note".into(), framed(&trace))
            .expect("registration");
        let retained = runner
            .get_message_renderer_call::<Value>("note")
            .expect("registered call");
        retained
            .call(
                Rc::new(message(Presence::Missing)),
                MessageRenderOptions { expanded: false },
                LiveTheme::default(),
            )
            .expect("rendered");
        assert_eq!(
            *trace.borrow(),
            ["before", "same", "after", "foreign"],
            "the registering bus delivers its own listener in the callback frame"
        );
        assert_eq!(*reentered.borrow(), ["reentered"]);
        let replacement = Trace::default();
        replaced(&owner.api, &retained, &replacement, &runner);
        assert_eq!(
            *trace.borrow(),
            [
                "before", "same", "after", "foreign", "before", "same", "after", "foreign"
            ]
        );
    });
}

/// A renderer that emits once, recording its own callback frame.
fn framed(trace: &Trace) -> MessageRenderer<Value, Local> {
    let order = trace.clone();
    Rc::new(
        move |_message: Rc<CustomMessage<Value>>,
              _options: MessageRenderOptions,
              _theme: LiveTheme,
              emitter: CallbackEmitter<'_, Local>| {
            mark(&order, "before");
            emitter.emit("channel", Value::Null);
            mark(&order, "after");
            Ok(None)
        },
    )
}

/// Registers a listener on another bus and a foreign listener on the same bus.
fn bystanders(registering: &LocalBus, trace: &Trace) {
    LocalBus::new()
        .controller
        .for_extension()
        .on("channel", listener(trace, "wrong-bus"));
    registering
        .controller
        .for_extension()
        .on("channel", listener(trace, "foreign"));
}

/// Registers a second callback, then checks the retained and the current lookups.
fn replaced(
    api: &maestro_extensions::ExtensionAPI<Local>,
    retained: &maestro_extensions::MessageRendererCall<Value, Local>,
    replacement: &Trace,
    runner: &ExtensionRunner<Local>,
) {
    api.register_message_renderer(
        "note".into(),
        recording(replacement, &Arc::default(), Outcome::None),
    )
    .expect("replacement");
    retained
        .call(
            Rc::new(message(Presence::Missing)),
            MessageRenderOptions { expanded: false },
            LiveTheme::default(),
        )
        .expect("retained call still renders");
    runner
        .get_message_renderer_call::<Value>("note")
        .expect("replacement call")
        .call(
            Rc::new(message(Presence::Missing)),
            MessageRenderOptions { expanded: false },
            LiveTheme::default(),
        )
        .expect("replacement renders");
    assert_eq!(
        *replacement.borrow(),
        ["note false false 1.5"],
        "a new lookup selects the replacement"
    );
}
/// Both callback families keep their capture contracts through a projected call.
#[test]
fn projected_calls_preserve_local_and_threaded_captures() {
    local(async {
        let bus = LocalBus::new();
        let owner = loaded("/local-capture", bus.controller.for_extension());
        let runner = runner(&owner.owners);
        let captured = Rc::new(RefCell::new(Vec::new()));
        let local = Rc::clone(&captured);
        let renderer: MessageRenderer<Value, Local> = Rc::new(
            move |_message: Rc<CustomMessage<Value>>,
                  _options: MessageRenderOptions,
                  _theme: LiveTheme,
                  _: CallbackEmitter<'_, Local>| {
                local.borrow_mut().push("local");
                Ok(None)
            },
        );
        owner
            .api
            .register_message_renderer("local".into(), renderer)
            .expect("registration");
        runner
            .get_message_renderer_call::<Value>("local")
            .expect("registered call")
            .call(
                Rc::new(message(Presence::Missing)),
                MessageRenderOptions { expanded: false },
                LiveTheme::default(),
            )
            .expect("rendered");
        assert_eq!(*captured.borrow(), ["local"]);
    });
    let events: EventBus<Threaded> =
        (*create_event_bus_for::<Threaded>(Arc::new(|_| {}), Arc::new(|_| {}))).clone();
    let owner = loaded_for("/threaded-capture", events);
    let runner = runner_for::<Threaded>(&owner.owners, Arc::new(|_| {}));
    let captured = Arc::new(Mutex::new(Vec::new()));
    let threaded = Arc::clone(&captured);
    let renderer: MessageRenderer<Value, Threaded> = Arc::new(
        move |_message: Arc<CustomMessage<Value>>,
              _options: MessageRenderOptions,
              _theme: LiveTheme,
              _: CallbackEmitter<'_, Threaded>| {
            threaded.lock().expect("trace").push("threaded");
            Ok(None)
        },
    );
    owner
        .api
        .register_message_renderer("threaded".into(), renderer)
        .expect("registration");
    runner
        .get_message_renderer_call::<Value>("threaded")
        .expect("registered call")
        .call(
            Arc::new(message(Presence::Missing)),
            MessageRenderOptions { expanded: false },
            LiveTheme::default(),
        )
        .expect("rendered");
    assert_eq!(*captured.lock().expect("trace"), ["threaded"]);
}

/// Retained calls outlive their registration and release it with the last handle.
#[test]
fn projected_calls_release_removed_registration_after_last_handle() {
    let (bus, trace) = (LocalBus::new(), Trace::default());
    let owner = loaded("/release", bus.controller.for_extension());
    let runner = runner(&owner.owners);
    let released = Rc::new(Cell::new(false));
    let capture = Released(released.clone());
    let recorded = trace.clone();
    let renderer: MessageRenderer<Value, Local> = Rc::new(
        move |_message: Rc<CustomMessage<Value>>,
              _options: MessageRenderOptions,
              _theme: LiveTheme,
              _: CallbackEmitter<'_, Local>| {
            let _ = &capture;
            mark(&recorded, "retained");
            Ok(None)
        },
    );
    owner
        .api
        .register_message_renderer("note".into(), renderer)
        .expect("registration");
    let call = runner
        .get_message_renderer_call::<Value>("note")
        .expect("registered call");
    let raw = runner
        .get_message_renderer::<Value>("note")
        .expect("raw renderer");
    owner
        .owner
        .write()
        .expect("owner")
        .message_renderers
        .clear();
    assert!(runner.get_message_renderer_call::<Value>("note").is_none());
    call.call(
        Rc::new(message(Presence::Missing)),
        MessageRenderOptions { expanded: false },
        LiveTheme::default(),
    )
    .expect("a retained call still renders");
    assert_eq!(*trace.borrow(), ["retained"]);
    assert!(!released.get(), "the raw getter also retains the callback");
    let alias = call.clone();
    drop(call);
    assert!(!released.get());
    drop(alias);
    assert!(!released.get(), "the raw renderer still retains it");
    drop(raw);
    assert!(released.get(), "the last handle releases the callback");
}
