//! Registration, tool, bus and renderer callbacks through the author facade.
#![cfg(test)]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod support;

use std::cell::{Cell, RefCell};
use std::fmt::Write as _;
use std::rc::Rc;
use std::task::Poll;

use maestro_extensions_wasm::{
    AbortSignal, AgentToolResult, AutocompleteItem, CallbackEmitter, CommandOptions, Component,
    ComponentView, ContentBlock, CustomMessage, EventListener, ExtensionAPI, ExtensionEvent,
    ExtensionEventResult, ExtensionFactory, ExtensionFuture, ExtensionResult, FlagOptions,
    FlagType, FlagValue, ImageContent, MessageRenderOptions, MessageRenderer, PrepareArguments,
    RenderShell, ShortcutOptions, TextContent, Theme, ThemePort, ToolCallEvent, ToolDefinition,
    ToolExecute, ToolMetadata, ToolResultEvent, is_bash_tool_result, is_edit_tool_result,
    is_find_tool_result, is_grep_tool_result, is_ls_tool_result, is_read_tool_result,
    is_tool_call_event_type, is_write_tool_result, load_extension_from_factory,
};
use serde_json::{Value, json};
use support::controlled::{ControlledHost, Flag, Log};
use support::{block_on, guest_family::events, poll_once};
use tokio::sync::oneshot;

/// Lines a callback wrote about the calls it saw.
type Seen = Rc<RefCell<Vec<String>>>;

/// A fresh host and the facade its extension registers through.
fn host() -> (ControlledHost, ExtensionAPI, Log) {
    let log = Log::default();
    let host = ControlledHost::new(log.clone());
    let api = host.api();
    (host, api, log)
}

/// A handler that answers every event with no result.
fn quiet() -> maestro_extensions_wasm::ExtensionHandler {
    Rc::new(|_event, _ctx| Box::pin(async { Ok(None) }))
}

#[test]
fn maestro_factory_finishes_before_registration_is_observed() {
    let (host, api, _) = host();
    let immediate: ExtensionFactory =
        Box::new(|api| Box::pin(async move { api.on("input", quiet()) }));
    block_on(load_extension_from_factory(immediate, api.clone())).unwrap();
    assert_eq!(host.registered(), ["event input"]);

    let (release, released) = oneshot::channel::<()>();
    let pending: ExtensionFactory = Box::new(|api| {
        Box::pin(async move {
            released.await.map_err(|error| error.to_string())?;
            api.on("session_start", quiet())
        })
    });
    let mut load = Box::pin(load_extension_from_factory(pending, api.clone()));
    assert!(matches!(poll_once(&mut load), Poll::Pending));
    assert_eq!(
        host.registered(),
        ["event input"],
        "nothing is observed while the factory is pending"
    );
    release.send(()).unwrap();
    assert_eq!(block_on(load), Ok(()));
    assert_eq!(host.registered(), ["event input", "event session_start"]);

    let failing: ExtensionFactory = Box::new(|api| {
        Box::pin(async move {
            api.on("agent_end", quiet())?;
            Err("factory failed \u{1f4a5}".to_owned())
        })
    });
    let outcome = block_on(load_extension_from_factory(failing, api));
    assert_eq!(outcome, Err("factory failed \u{1f4a5}".to_owned()));
    assert_eq!(
        host.registered(),
        ["event input", "event session_start", "event agent_end"],
        "a failed factory keeps what it registered before failing"
    );
}

/// Completions that answer immediately.
fn immediate_completions(
    items: Option<Vec<AutocompleteItem>>,
) -> maestro_extensions_wasm::ArgumentCompletions {
    Rc::new(move |_prefix| {
        let items = items.clone();
        Box::pin(async move { Ok(items) })
    })
}

/// A completion source that waits for a gate before suggesting.
fn gated_completions(
    gated: oneshot::Receiver<Vec<AutocompleteItem>>,
) -> maestro_extensions_wasm::ArgumentCompletions {
    let gated = RefCell::new(Some(gated));
    Rc::new(move |_prefix| {
        let gated = gated.borrow_mut().take();
        Box::pin(async move {
            match gated {
                Some(gated) => Ok(Some(gated.await.map_err(|error| error.to_string())?)),
                None => Ok(None),
            }
        })
    })
}

/// Registers five commands whose completions answer differently; returns the suggestion they
/// give and the gate that releases the pending one.
fn register_commands(
    api: &ExtensionAPI,
) -> (AutocompleteItem, oneshot::Sender<Vec<AutocompleteItem>>) {
    let handler: maestro_extensions_wasm::CommandHandler =
        Rc::new(|_args, _ctx| Box::pin(async { Ok(()) }));
    let suggestion = AutocompleteItem {
        value: "v".into(),
        label: "label \u{e9}".into(),
        description: Some("d".into()),
    };
    let (gate, gated) = oneshot::channel();
    for (name, completions) in [
        (
            "with-items",
            Some(immediate_completions(Some(vec![suggestion.clone()]))),
        ),
        (
            "with-empty-list",
            Some(immediate_completions(Some(Vec::new()))),
        ),
        ("with-none", Some(immediate_completions(None))),
        ("with-pending", Some(gated_completions(gated))),
        ("without", None),
    ] {
        let options = CommandOptions {
            description: Some(format!("description of {name}")),
            get_argument_completions: completions,
            handler: Rc::clone(&handler),
        };
        api.register_command(name, options).unwrap();
    }
    (suggestion, gate)
}

/// Checks the registrations a host saw, in order, and the commands' completions.
fn assert_completions(
    host: &ControlledHost,
    suggestion: &AutocompleteItem,
    gate: oneshot::Sender<Vec<AutocompleteItem>>,
) {
    let completed = |name: &str| {
        let completions = host
            .command(name)
            .unwrap()
            .get_argument_completions
            .unwrap();
        block_on(completions("x".to_owned()))
    };
    assert_eq!(completed("with-items"), Ok(Some(vec![suggestion.clone()])));
    assert_eq!(
        completed("with-empty-list"),
        Ok(Some(Vec::new())),
        "an empty list is not absence"
    );
    assert_eq!(completed("with-none"), Ok(None));
    assert!(
        host.command("without")
            .unwrap()
            .get_argument_completions
            .is_none()
    );
    let completions = host
        .command("with-pending")
        .unwrap()
        .get_argument_completions
        .unwrap();
    let mut waiting = Box::pin(completions("x".to_owned()));
    assert!(matches!(poll_once(&mut waiting), Poll::Pending));
    gate.send(vec![suggestion.clone()]).unwrap();
    assert_eq!(block_on(waiting), Ok(Some(vec![suggestion.clone()])));
    assert_eq!(
        host.command("with-pending").unwrap().description.as_deref(),
        Some("description of with-pending")
    );
}

#[test]
fn maestro_registration_forwards_each_owned_callback() {
    let (host, api, log) = host();
    api.on("input", quiet()).unwrap();
    api.on("input", quiet()).unwrap();
    api.on("my_extension_event", quiet()).unwrap();
    let (suggestion, gate) = register_commands(&api);
    let shortcut = ShortcutOptions {
        description: None,
        handler: Rc::new(|_ctx| Box::pin(async { Ok(()) })),
    };
    api.register_shortcut("ctrl+x", shortcut).unwrap();
    api.register_message_renderer("note", Rc::new(|_, _, _, _| Ok(None)))
        .unwrap();
    assert_eq!(
        host.registered(),
        [
            "event input",
            "event input",
            "event my_extension_event",
            "command with-items",
            "command with-empty-list",
            "command with-none",
            "command with-pending",
            "command without",
            "shortcut ctrl+x",
            "renderer note",
        ],
        "every registration is forwarded individually, in order, under its open name"
    );
    assert_eq!(
        host.handlers("input").len(),
        2,
        "same-name handlers are not collapsed by the guest"
    );
    assert_eq!(log.lines().len(), 10);
    assert_completions(&host, &suggestion, gate);
}

#[test]
fn maestro_flag_values_preserve_absence_false_and_empty() {
    let (host, api, _) = host();
    let flag = |kind, default| FlagOptions {
        description: None,
        type_: kind,
        default,
    };
    api.register_flag("bare", flag(FlagType::Boolean, None))
        .unwrap();
    api.register_flag(
        "off",
        flag(FlagType::Boolean, Some(FlagValue::Boolean(false))),
    )
    .unwrap();
    api.register_flag(
        "blank",
        flag(FlagType::String, Some(FlagValue::Text(String::new()))),
    )
    .unwrap();
    api.register_flag(
        "name",
        flag(FlagType::String, Some(FlagValue::Text("default".into()))),
    )
    .unwrap();
    host.supply_flag("name", &FlagValue::Text("override".into()));
    assert_eq!(api.get_flag("bare"), Ok(None), "no default means no value");
    assert_eq!(
        api.get_flag("off"),
        Ok(Some(FlagValue::Boolean(false))),
        "false is a value"
    );
    assert_eq!(
        api.get_flag("blank"),
        Ok(Some(FlagValue::Text(String::new()))),
        "an empty string is a value"
    );
    assert_eq!(
        api.get_flag("name"),
        Ok(Some(FlagValue::Text("override".into()))),
        "the command line wins"
    );
    assert_eq!(api.get_flag("unregistered"), Ok(None));
}

/// A text block with an optional provider signature.
fn text(text: &str, signature: Option<&str>) -> ContentBlock {
    ContentBlock::Text(TextContent {
        text: text.into(),
        text_signature: signature.map(Into::into),
    })
}

/// Metadata with every optional field filled.
fn full_metadata() -> ToolMetadata {
    ToolMetadata {
        name: "echo \u{e9}".into(),
        label: "Echo".into(),
        description: "Echoes".into(),
        prompt_snippet: Some("snippet".into()),
        prompt_guidelines: Some(vec!["one".into(), String::new()]),
        parameters: json!({ "type": "object" }).to_string(),
        render_shell: Some(RenderShell::OwnFrame),
        execution_mode: Some(maestro_extensions_wasm::ToolExecutionMode::Sequential),
    }
}

/// The echo tool: it wraps its arguments, reports half of its answer and records each call.
fn echo_tool(seen: &Seen) -> ToolDefinition {
    let prepare: PrepareArguments = Rc::new(|raw, emitter: CallbackEmitter<'_>| {
        emitter.emit("prepared", raw.clone())?;
        if raw["fail"] == json!(true) {
            return Err("bad arguments \u{1f6ab}".to_owned());
        }
        Ok(json!({ "wrapped": raw }))
    });
    let record = Rc::clone(seen);
    let execute: ToolExecute = Rc::new(move |call_id, params, signal, update, ctx| {
        record.borrow_mut().push(format!(
            "{call_id} {params} signal={:?} update={} cwd={:?}",
            signal.map(|signal| signal.aborted()),
            update.is_some(),
            ctx.cwd()
        ));
        Box::pin(async move {
            if params["fail"] == json!(true) {
                return Err("execution failed".to_owned());
            }
            if let Some(update) = update {
                let half = AgentToolResult {
                    content: vec![text("half", None)],
                    details: None,
                    terminate: None,
                };
                update(half)?;
            }
            let image = ContentBlock::Image(ImageContent {
                data: "AAAA".into(),
                mime_type: "image/png".into(),
            });
            Ok(AgentToolResult {
                content: vec![text("done \u{e9}", Some("sig-1")), image],
                details: Some(json!({ "n": null, "z": 0 }).to_string()),
                terminate: Some(false),
            })
        })
    });
    ToolDefinition {
        metadata: full_metadata(),
        prepare_arguments: Some(prepare),
        execute,
    }
}

/// Checks the argument preparation the host can run.
fn assert_preparation(host: &ControlledHost, prepare: &PrepareArguments) {
    let emitter = host.emitter();
    assert_eq!(
        prepare(json!({ "a": 1 }), CallbackEmitter::new(&emitter)),
        Ok(json!({ "wrapped": { "a": 1 } }))
    );
    assert_eq!(
        prepare(json!({ "fail": true }), CallbackEmitter::new(&emitter)),
        Err("bad arguments \u{1f6ab}".to_owned()),
        "a preparation error reaches the host unchanged"
    );
    assert!(
        host.log_lines()
            .iter()
            .any(|line| line == r#"emit prepared {"a":1}"#)
    );
}

/// Checks a run of the tool with progress, and a failing one.
fn assert_execution(execute: &ToolExecute) {
    let partials: Rc<RefCell<Vec<AgentToolResult>>> = Rc::default();
    let sink = Rc::clone(&partials);
    let update: maestro_extensions_wasm::AgentToolUpdateCallback = Rc::new(move |partial| {
        sink.borrow_mut().push(partial);
        Ok(())
    });
    let flag = Flag::default();
    let params = json!({ "wrapped": { "a": 1 } });
    let result = block_on(execute(
        "call-1".into(),
        params,
        Some(flag.signal()),
        Some(update),
        ControlledHost::context(),
    ));
    let result = result.unwrap();
    assert_eq!(result.content.len(), 2);
    assert_eq!(result.content[0], text("done \u{e9}", Some("sig-1")));
    assert_eq!(
        result.details.as_deref(),
        Some(r#"{"n":null,"z":0}"#),
        "present null and zero survive"
    );
    assert_eq!(
        result.terminate,
        Some(false),
        "an explicit false is not absence"
    );
    assert_eq!(partials.borrow().len(), 1);
    assert_eq!(partials.borrow()[0].content, [text("half", None)]);
    assert_eq!(partials.borrow()[0].details, None);
    let failure = block_on(execute(
        "call-2".into(),
        json!({ "fail": true }),
        None,
        None,
        ControlledHost::context(),
    ));
    assert_eq!(failure, Err("execution failed".to_owned()));
}

#[test]
fn maestro_tool_preparation_execution_and_progress_keep_values() {
    let (host, api, _) = host();
    let seen = Seen::default();
    api.register_tool(echo_tool(&seen)).unwrap();
    assert_eq!(host.tool_metadata("echo \u{e9}"), Some(full_metadata()));
    let (prepare, execute) = host.tool("echo \u{e9}").unwrap();
    assert_preparation(&host, &prepare.unwrap());
    assert_execution(&execute);
    assert_eq!(
        seen.borrow()[0],
        r#"call-1 {"wrapped":{"a":1}} signal=Some(false) update=true cwd=Ok("/work")"#
    );
    assert_eq!(
        seen.borrow()[1],
        r#"call-2 {"fail":true} signal=None update=false cwd=Ok("/work")"#
    );
}

/// The saved outputs of the reference's seven result guards for each input name.
const GUARDS: [[bool; 7]; 16] = [
    [true, false, false, false, false, false, false],
    [false, true, false, false, false, false, false],
    [false, false, true, false, false, false, false],
    [false, false, false, true, false, false, false],
    [false, false, false, false, true, false, false],
    [false, false, false, false, false, true, false],
    [false, false, false, false, false, false, true],
    [false; 7],
    [false; 7],
    [false; 7],
    [false; 7],
    [false; 7],
    [false; 7],
    [false; 7],
    [false; 7],
    [false; 7],
];

/// The saved outputs of the reference's call check: row `i` compares input `i` with each name.
const CALL_MATCHES: [&str; 16] = [
    "1000000000000000",
    "0100000000000000",
    "0010000000000000",
    "0001000000000000",
    "0000100000000000",
    "0000010000000000",
    "0000001000000000",
    "0000000100000000",
    "0000000010000000",
    "0000000001000000",
    "0000000000100000",
    "0000000000010000",
    "0000000000001000",
    "0000000000000100",
    "0000000000000010",
    "0000000000000001",
];

/// The names the reference was run with: the built-ins, a custom name, the empty name,
/// case and whitespace variants, a byte order mark, a next-line character, non-ASCII text
/// and a name that is special to the reference's object model.
const NAMES: [&str; 16] = [
    "bash",
    "read",
    "edit",
    "write",
    "grep",
    "find",
    "ls",
    "custom",
    "",
    "Bash",
    "bash ",
    "\u{feff}bash",
    "\u{85}bash",
    "\u{e9}",
    "\u{1f680}",
    "__proto__",
];

#[test]
fn maestro_tool_name_checks_are_exact() {
    let guards: [fn(&ToolResultEvent) -> bool; 7] = [
        is_bash_tool_result,
        is_read_tool_result,
        is_edit_tool_result,
        is_write_tool_result,
        is_grep_tool_result,
        is_find_tool_result,
        is_ls_tool_result,
    ];
    let mut comparisons = 0;
    for (row, name) in NAMES.iter().enumerate() {
        let result = ToolResultEvent::Custom(events::CustomToolResultEvent {
            base: events::ToolResultBase {
                tool_call_id: "c".into(),
                input: "{}".into(),
                content: Vec::new(),
                is_error: false,
            },
            tool_name: (*name).to_owned(),
            details: None,
        });
        let call = ToolCallEvent::Custom(events::CustomToolCallEvent {
            tool_call_id: "c".into(),
            tool_name: (*name).to_owned(),
            input: "{}".into(),
        });
        for (guard, expected) in guards.iter().zip(GUARDS[row]) {
            assert_eq!(guard(&result), expected, "result guard for {name:?}");
            comparisons += 1;
        }
        for (other, expected) in NAMES.iter().zip(CALL_MATCHES[row].chars()) {
            assert_eq!(
                is_tool_call_event_type(other, &call),
                expected == '1',
                "{other:?} vs {name:?}"
            );
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 16 * (7 + 16));
    let typed = ToolResultEvent::Write(events::ToolResultBase {
        tool_call_id: "c".into(),
        input: "{}".into(),
        content: Vec::new(),
        is_error: false,
    });
    assert!(is_write_tool_result(&typed) && !is_bash_tool_result(&typed));
}

/// A tool call handler that notes the command it saw, appends its label to it and, for the
/// first label, fails afterwards.
fn editing_handler(label: &'static str, order: &Seen) -> maestro_extensions_wasm::ExtensionHandler {
    let order = Rc::clone(order);
    Rc::new(move |event, _ctx| {
        let outcome = edit_command(event, label, &order);
        Box::pin(async move { outcome })
    })
}

/// Edits the bash command of a tool call event in place.
fn edit_command(
    event: &mut ExtensionEvent,
    label: &str,
    order: &Seen,
) -> ExtensionResult<Option<ExtensionEventResult>> {
    let ExtensionEvent::ToolCall(ToolCallEvent::Bash(call)) = event else {
        return Ok(None);
    };
    order
        .borrow_mut()
        .push(format!("{label} saw {}", call.input.command));
    let _ = write!(call.input.command, " +{label}");
    if label == "first" {
        return Err("first failed".to_owned());
    }
    let verdict = events::ToolCallEventResult {
        block: Some(false),
        reason: None,
    };
    Ok(Some(ExtensionEventResult::ToolCall(verdict)))
}

#[test]
fn maestro_handler_mutation_survives_returned_error() {
    let (host, api, _) = host();
    let order = Seen::default();
    for label in ["first", "second"] {
        api.on("tool_call", editing_handler(label, &order)).unwrap();
    }
    let mut event = ExtensionEvent::ToolCall(support::guest_family::bash_call("ls"));
    let handlers = host.handlers("tool_call");
    let first = block_on(handlers[0](&mut event, ControlledHost::context()));
    assert_eq!(first.unwrap_err(), "first failed");
    let second = block_on(handlers[1](&mut event, ControlledHost::context()));
    assert!(matches!(
        second,
        Ok(Some(ExtensionEventResult::ToolCall(_)))
    ));
    let ExtensionEvent::ToolCall(ToolCallEvent::Bash(call)) = event else {
        panic!("the event changed kind");
    };
    assert_eq!(
        call.input.command, "ls +first +second",
        "the edit of the failing handler is kept and seen"
    );
    assert_eq!(*order.borrow(), ["first saw ls", "second saw ls +first"]);
}

#[test]
fn maestro_bus_keeps_prefix_tail_and_unsubscribe_ownership() {
    let (host, api, log) = host();
    let bus = api.events();
    let ran: Rc<RefCell<Vec<String>>> = Rc::default();
    let listener = |label: &'static str, with_tail: bool| -> EventListener {
        let ran = Rc::clone(&ran);
        Rc::new(move |data: Value, emitter: CallbackEmitter<'_>| {
            ran.borrow_mut().push(format!("{label} prefix {data}"));
            emitter.emit("echo", json!(label))?;
            if !with_tail {
                return Ok(None);
            }
            let ran = Rc::clone(&ran);
            let tail: ExtensionFuture<'static, ()> = Box::pin(async move {
                ran.borrow_mut().push(format!("{label} tail"));
                Ok(())
            });
            Ok(Some(tail))
        })
    };
    let first = bus.on("topic", listener("a", true)).unwrap();
    let _dropped = bus.on("topic", listener("b", false)).unwrap();
    drop(bus.on("topic", listener("c", false)).unwrap());
    block_on(bus.emit("topic", json!({ "n": 1 }))).unwrap();
    assert_eq!(
        *ran.borrow(),
        [
            r#"a prefix {"n":1}"#,
            r#"b prefix {"n":1}"#,
            r#"c prefix {"n":1}"#,
            "a tail"
        ],
        "listeners run in subscription order and the tail follows the synchronous parts"
    );
    assert!(
        log.lines().iter().any(|line| line == r#"emit echo "a""#),
        "a callback emits through the borrowed emitter"
    );
    first.unsubscribe().unwrap();
    ran.borrow_mut().clear();
    block_on(host.deliver("topic", json!(2))).unwrap();
    assert_eq!(
        *ran.borrow(),
        ["b prefix 2", "c prefix 2"],
        "dropping a subscription keeps it; unsubscribe ends it"
    );
    assert!(
        log.lines()
            .iter()
            .any(|line| line.starts_with("unsubscribe "))
    );
}

/// A component that renders its message and counts invalidations.
struct Note {
    /// The rendered line.
    line: String,
    /// Invalidations seen.
    invalidated: Rc<Cell<u32>>,
    /// Reports the drop of the view.
    dropped: Rc<Cell<bool>>,
}

impl ComponentView for Note {
    fn render(&self, width: u32, emitter: CallbackEmitter<'_>) -> ExtensionResult<Vec<String>> {
        emitter.emit("rendered", json!(width))?;
        Ok(vec![format!("{width}|{}", self.line)])
    }

    fn handle_input(&self, data: &str, emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        emitter.emit("input", json!(data))
    }

    fn invalidate(&self, _emitter: CallbackEmitter<'_>) -> ExtensionResult<()> {
        self.invalidated.set(self.invalidated.get() + 1);
        Ok(())
    }
}

impl Drop for Note {
    fn drop(&mut self) {
        self.dropped.set(true);
    }
}

/// A theme with no operations.
struct Plain;

impl ThemePort for Plain {}

/// A renderer that draws its message with a [`Note`] and skips messages of type `skip`.
fn note_renderer(invalidated: &Rc<Cell<u32>>, dropped: &Rc<Cell<bool>>) -> MessageRenderer {
    let (count, gone) = (Rc::clone(invalidated), Rc::clone(dropped));
    Rc::new(
        move |message: CustomMessage, options: MessageRenderOptions, _theme: Theme, _emitter| {
            if message.custom_type == "skip" {
                return Ok(None);
            }
            let line = format!(
                "{}|{:?}|expanded={}",
                message.custom_type, message.content, options.expanded
            );
            Ok(Some(Component::new(Note {
                line,
                invalidated: Rc::clone(&count),
                dropped: Rc::clone(&gone),
            })))
        },
    )
}

/// A custom message of the given type.
fn custom_message(custom_type: &str) -> CustomMessage {
    CustomMessage {
        custom_type: custom_type.into(),
        content: maestro_extensions_wasm::UserContent::Text("body \u{e9}".into()),
        display: true,
        details: Some("null".into()),
        timestamp: 1.5,
    }
}

#[test]
fn maestro_renderer_invokes_and_retains_component() {
    let (host, api, log) = host();
    let invalidated = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(false));
    api.register_message_renderer("note", note_renderer(&invalidated, &dropped))
        .unwrap();
    let render = host.renderer("note").unwrap();
    let emitter = host.emitter();
    let theme = Theme::new(Rc::new(Plain));
    let collapsed = MessageRenderOptions { expanded: false };
    let skipped = render(
        custom_message("skip"),
        collapsed,
        theme.clone(),
        CallbackEmitter::new(&emitter),
    );
    assert!(
        matches!(skipped, Ok(None)),
        "no component is distinct from an empty one"
    );
    let expanded = MessageRenderOptions { expanded: true };
    let component = render(
        custom_message("note"),
        expanded,
        theme,
        CallbackEmitter::new(&emitter),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        component.render(20, CallbackEmitter::new(&emitter)),
        Ok(vec![
            r#"20|note|UserContent::Text("body é")|expanded=true"#.to_owned()
        ])
    );
    component
        .handle_input("k", CallbackEmitter::new(&emitter))
        .unwrap();
    assert!(
        !component.wants_key_release(),
        "the key release preference defaults to false"
    );
    component
        .invalidate(CallbackEmitter::new(&emitter))
        .unwrap();
    assert_eq!(invalidated.get(), 1);
    let retained = component.clone();
    drop(component);
    assert!(
        !dropped.get(),
        "the retained component keeps its captures after the callback returned"
    );
    drop(retained);
    assert!(dropped.get());
    assert!(log.lines().contains(&r#"emit input "k""#.to_owned()));
}

/// Registers a session start handler that reads the system prompt, a quiet agent start handler
/// and a bus subscription; returns how often the first one ran.
fn register_failing_extension(
    api: &ExtensionAPI,
) -> (Rc<Cell<u32>>, maestro_extensions_wasm::Subscription) {
    let calls: Rc<Cell<u32>> = Rc::default();
    let counter = Rc::clone(&calls);
    api.on(
        "session_start",
        Rc::new(move |_event, ctx| {
            counter.set(counter.get() + 1);
            Box::pin(async move { ctx.get_system_prompt().map(|_| None) })
        }),
    )
    .unwrap();
    api.on("agent_start", quiet()).unwrap();
    let subscription = api.events().on("topic", Rc::new(|_, _| Ok(None))).unwrap();
    (calls, subscription)
}

#[test]
fn maestro_trapped_adapter_preserves_registrations_and_reports_errors() {
    let (host, api, log) = host();
    let (calls, subscription) = register_failing_extension(&api);
    let flag = Flag::default();
    let signal: AbortSignal = flag.signal();
    let attributed = "extension 'author' trapped in session_start; later calls fail".to_owned();
    host.script
        .reply::<String>("get_system_prompt", Err(attributed.clone()));
    let handler = host.handler("session_start").unwrap();
    let context = ControlledHost::context_with(&host.script);
    for _ in 0..2 {
        let start = support::guest_family::session_start();
        let mut event =
            ExtensionEvent::Session(maestro_extensions_wasm::SessionEvent::Start(start));
        let outcome = block_on(handler(&mut event, context.clone()));
        assert_eq!(outcome.unwrap_err(), attributed);
    }
    assert_eq!(
        calls.get(),
        2,
        "every attempted call is reported, not swallowed"
    );
    assert_eq!(
        host.registered(),
        ["event session_start", "event agent_start"],
        "a failing instance stays registered"
    );
    assert!(
        subscription.unsubscribe().is_ok(),
        "its subscription is still live"
    );
    assert!(!signal.aborted());
    flag.abort();
    assert!(signal.aborted(), "an unrelated signal keeps working");
    let mut healthy = ExtensionEvent::AgentStart;
    assert!(block_on(host.handler("agent_start").unwrap()(&mut healthy, context)).is_ok());
    assert_eq!(
        log.lines()
            .iter()
            .filter(|line| line.starts_with("get_system_prompt"))
            .count(),
        2
    );
}
