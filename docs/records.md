# Model invocation records

Start with the [model invocation library guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/api.md)
for installation, offline executable examples and the delivered protocol/account surface.

`maestro-models` supplies typed conversations, model descriptors and invocation
options through replaceable registered protocol callbacks. An invocable model
need not belong to a catalog. Registration wraps both callbacks with a protocol
check. Replacing a protocol preserves its position but replaces its source owner;
source removal affects only registrations still owned by that source. Retained
handles remain usable after replacement, removal or clear.

`TextContent::new(text)` moves an owned string into a text block unchanged and
leaves its model-specific signature absent. It preserves empty text, whitespace,
Unicode and control characters.

Protocol registration, account membership, session cleanup and cloud callback
selection retain separate owners on each target.

The registry already holds the bundled protocols; see [bundled protocol linkage](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/module-linkage.md)
for registration, reset, the root entry points and the raw extras they accept.

`stream` and `stream_simple` invoke immediately and return a registered custom
callback's setup errors directly; a bundled callback settles its own as error streams.
`complete` and `complete_simple` also invoke immediately, but return setup errors
inside their futures. Options, hooks and cancellation signals reach the adapter
unchanged; adapters own payload policy, credentials, I/O and provider defaults.
A payload hook receives `PayloadHookInput` and returns the selected record. Its `payload`
field holds JSON parameters to keep, edit or replace; its optional `signal` field carries
request cancellation for [content generation](models/content-generation.md#public-invocation).
Other bundled adapters supply no hook signal and ignore the returned signal.
Response hooks receive status and headers under the owning adapter's policy:
[content generation](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/content-generation.md)
does not use the hook; [cloud conversations](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/cloud-conversations.md)
wait for the first known service event or successful end of stream, including
empty or unknown-only streams. A failure before either prevents the hook from
running. Other adapter policies are documented on their focused pages. Both hooks receive the invocation's model, shared and read-only.

Dynamic catalog callbacks accept `StreamOptionsInput::Raw(ProviderStreamOptions)`
or `StreamOptionsInput::Simple(SimpleStreamOptions)`. The carrier retains the
original form, including open raw fields, typed objects and hooks; an absent
outer options operand stays absent. See [dynamic providers](catalog.md#dynamic-providers)
for registration and replay.

The [offline model catalog](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/catalog.md) provides owned embedded descriptors,
exact lookup, thinking-level selection, identity comparison and flat-rate costs.

The [request authentication guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/request-authentication.md)
explains explicit environment-key discovery and ambient configuration.

The [model option helpers](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/model-options.md)
copy common settings, adjust thinking budgets and classify context overflow.

The [assistant-output guide](models/assistant-output.md) describes shared initialization
and failure publication for provider and proxy streams.

The [chat-completion guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/chat-completions.md)
covers direct streamed requests, header and cache policy, retries and the shared
Cloudflare and Copilot helpers.

The [response-event guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/responses.md)
describes the internal history/tool converter and response-event reducer.

The [response-session guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/response-sessions.md)
covers dedicated raw/simple invocation, automatic transport dispatch and session controls.

The [OAuth authorization guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/oauth.md)
covers data records, secure proof keys, escaped callback pages, device, subscription
and response-account login and refresh.

The [message-protocol guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/messages.md)
covers direct streamed message requests with key authentication, their payload, cache and
header policy, the event framing and repair, and replaceable clients.

The [content-generation guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/content-generation.md)
covers the shared conversation, tool and finish-reason conversion of the Gemini-style protocols.

See [Tool arguments](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/arguments.md) for shared JSON completion and repair.

The [conversation projection](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/conversation-projection.md)
produces replay-safe request histories without editing stored messages. Providers
supply ID normalization; projection owns content conversion and missing results.

## Controlled registered invocation

A retained owner implements `Invoke` generically for both execution families.
The same handle can populate raw and simple fields; direct hook closures stay unchanged.

```rust
use maestro_models::{
    SharedHandle, ApiProvider, AssistantMessage, AssistantMessageEvent, Context, DoneReason,
    Model, clear_api_providers, complete, create_assistant_message_event_stream,
    register_api_provider,
};
use std::{future::Future, sync::{Arc, RwLock}, task::{Context as PollContext, Poll, Waker}};

let model: Model = serde_json::from_value(serde_json::json!({
    "id": "controlled", "name": "Controlled model", "api": "fixture",
    "provider": "fixture", "baseUrl": "https://fixture.invalid",
    "reasoning": false, "input": ["text"], "contextWindow": 100,
    "maxTokens": 10,
    "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}
}))?;
let message: AssistantMessage = serde_json::from_value(serde_json::json!({
    "role": "assistant", "content": [{"type": "text", "text": "ready"}],
    "api": "fixture", "provider": "fixture", "model": "controlled",
    "usage": {"input": 0, "output": 1, "cacheRead": 0, "cacheWrite": 0,
        "totalTokens": 1,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}},
    "stopReason": "stop", "timestamp": 1
}))?;
let message = Arc::new(RwLock::new(message));
let producer = create_assistant_message_event_stream();
producer.push(AssistantMessageEvent::Done {
    reason: DoneReason::Stop, message: Arc::clone(&message),
});
struct ControlledProvider(maestro_models::AssistantMessageEventStream);
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for ControlledProvider
{
    type Return = Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, _: maestro_models::ProviderCall<E, O>) -> Self::Return {
        Ok(self.0.clone())
    }
}
let owner = SharedHandle::new(ControlledProvider(producer.clone()));
register_api_provider(ApiProvider {
    api: "fixture".into(),
    stream: owner.clone(),
    stream_simple: owner,
}, Some("controlled-example".into()));
let completion = complete(model, Context {
    system_prompt: None, messages: vec![], tools: None,
}, None);
clear_api_providers(); // The admitted invocation retains its producer.
let mut context = PollContext::from_waker(Waker::noop());
let mut completion = std::pin::pin!(completion);
assert!(matches!(completion.as_mut().poll(&mut context), Poll::Ready(Ok(_))));
let mut event = std::pin::pin!(producer.next());
assert!(matches!(event.as_mut().poll(&mut context),
    Poll::Ready(Some(AssistantMessageEvent::Done { .. }))));
# Ok::<(), serde_json::Error>(())
```

### Asynchronous registered producers

Keep the worker generic; only its future boxes name concrete execution families.
The caller's supplied admission retains the selected future without moving local hooks.

```rust
use maestro_models::*;
use std::{cell::{Cell, RefCell}, rc::Rc, sync::{Arc, RwLock}};

trait Producer: ProviderExecution {
    fn task(call: ProviderCall<Self, StreamOptionsInput<Self>>,
        stream: AssistantMessageEventStream) -> Self::Future<'static, ()>;
}
impl Producer for Local {
    fn task(call: ProviderCall<Self, StreamOptionsInput<Self>>,
        stream: AssistantMessageEventStream) -> Self::Future<'static, ()> {
        Box::pin(produce(call, stream))
    }
}
impl Producer for Threaded {
    fn task(call: ProviderCall<Self, StreamOptionsInput<Self>>,
        stream: AssistantMessageEventStream) -> Self::Future<'static, ()> {
        Box::pin(produce(call, stream))
    }
}
async fn answer<E: ProviderExecution>(call: ProviderCall<E, StreamOptionsInput<E>>)
    -> Result<AssistantMessage, DiagnosticErrorInfo> {
    let common = match call.options {
        Some(StreamOptionsInput::Raw(options)) => options.common,
        Some(StreamOptionsInput::Simple(options)) => options.common,
        None => StreamOptions::default(),
    };
    let mut input = PayloadHookInput { payload: serde_json::json!({"answer":"ready"}), signal: None };
    if let Some(hook) = common.on_payload { input = hook(input, Arc::new(call.model)).await?; }
    Ok(faux_assistant_message(input.payload["answer"].as_str().unwrap_or("ready"),
        FauxAssistantMessageOptions::default()))
}
async fn produce<E: ProviderExecution>(call: ProviderCall<E, StreamOptionsInput<E>>,
    stream: AssistantMessageEventStream) {
    let event = match answer(call).await {
        Ok(message) => AssistantMessageEvent::Done { reason: DoneReason::Stop,
            message: Arc::new(RwLock::new(message)) },
        Err(error) => AssistantMessageEvent::Error { reason: ErrorReason::Error,
            error: Arc::new(RwLock::new(faux_assistant_message("", FauxAssistantMessageOptions {
                stop_reason: Some(StopReason::Error), error_message: Some(error.message),
                ..Default::default()
            }))) },
    };
    stream.push(event);
}
struct Custom;
impl<E: Producer, O: Into<StreamOptionsInput<E>>> Invoke<E, ProviderCall<E, O>> for Custom {
    type Return = Result<AssistantMessageEventStream, DiagnosticErrorInfo>;
    fn invoke(&self, call: ProviderCall<E, O>) -> Self::Return {
        let submit = call.submit.clone();
        let stream = AssistantMessageEventStream::new();
        let input = ProviderCall { model: call.model, context: call.context,
            options: call.options.map(Into::into), submit: call.submit };
        E::submit(E::task(input, stream.clone()), submit, |task| { tokio::spawn(task); Ok(()) })?;
        Ok(stream)
    }
}
fn one_generic_owner_registers_both_fields() -> Result<(), Box<dyn std::error::Error>> {
    let owner = Arc::new(Custom);
    register_api_provider(ApiProvider { api: "custom-async".into(),
        stream: owner.clone(), stream_simple: owner }, Some("custom-async-example".into()));
    let model: Model = serde_json::from_value(serde_json::json!({
        "id":"controlled", "name":"Controlled", "api":"custom-async", "provider":"custom",
        "baseUrl":"https://fixture.invalid", "reasoning":false, "input":["text"],
        "contextWindow":100, "maxTokens":10,
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0}
    }))?;
    let tasks = Rc::new(RefCell::new(Vec::new()));
    let submitted = tasks.clone();
    let spawn: ProviderSpawner<Local> = Rc::new(move |task| { submitted.borrow_mut().push(task); true });
    let calls = Rc::new(Cell::new(0));
    let observed = calls.clone();
    let options = ProviderStreamOptions::<Local> { common: StreamOptions {
        on_payload: Some(Rc::new(move |mut input, _| {
            let observed = observed.clone();
            Box::pin(async move {
                tokio::task::yield_now().await;
                observed.set(observed.get() + 1);
                input.payload["answer"] = serde_json::json!("selected");
                Ok(input)
            })
        })), ..Default::default()
    }, ..Default::default() };
    let stream = stream_for::<Local>(model, Context { system_prompt: None, messages: vec![], tools: None },
        Some(options), spawn)?;
    assert_eq!(calls.get(), 0);
    let runtime = tokio::runtime::Builder::new_current_thread().build()?;
    let task = tasks.borrow_mut().pop().unwrap();
    runtime.block_on(task);
    assert_eq!(calls.get(), 1);
    let message = runtime.block_on(stream.result());
    assert_eq!(message.read().unwrap().content, [AssistantContent::Text(faux_text("selected"))]);
    unregister_api_providers("custom-async-example");
    Ok(())
}
one_generic_owner_registers_both_fields()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Stream ownership

`Execution`, `Local` and `Threaded` select erased futures and shared/weak pointer
representations for callable owners. They do not create tasks. Local uses `Rc`;
`Threaded` uses `Arc` and `Send` futures natively, and `Rc` with local futures
on browser targets. `SharedHandle<T>` selects the same native/browser pointer
family for registered stream callbacks and account-provider handles.
`Execution::downgrade`, `upgrade` and `ptr_eq` operate on a family's shared and
weak pointers, and its weak pointer is `Clone`. Extension registration uses them
for weak facade back-edges and allocation-identity subscription removal in
either family.
`EventStream::new_local_items` admits local retained items through the same FIFO;
`new` keeps its existing native thread-safety bounds. Classifier and extractor
callbacks retain their existing target-specific bounds.

`EventStream` is a producer-owned FIFO. Readers compete for events; the first
terminal result is independently and repeatedly observable. Assistant updates
and terminal observations retain the same live message handle, not snapshots.
Dropping a reader or result observer does not cancel production. A terminal
event reaches one pending reader and wakes all remaining readers to EOF. End
retains queued events; an absent result stays pending until an explicit result
arrives. The first supplied result is never replaced.

Classification and extraction run outside stream locks. Terminal admission is
reserved before extraction, so an extractor's reentrant pushes are ignored.
Readers and result observers wait for publication. Caller wakers, retained-result
clones and callback destruction also execute outside ownership locks.

`Cancellation` is re-exported from the [shared cancellation owner](../crates/maestro-cancellation/README.md)
through both models public paths. The caller decides which
producer observes a signal; unrelated streams are not automatically stopped.

## Cleanup and diagnostics

Session cleanup registrations deduplicate callback identity and preserve order.
Removal is explicit and idempotent; dropping a removal handle does not unsubscribe.
Traversal is live: added callbacks run in that pass, while unvisited removed
callbacks are skipped. Nested cleanup is permitted. Every callback is attempted;
failures are returned in encounter order with `Failed to cleanup session resources`.

Diagnostics retain supplied name, message, stack and textual or numeric code;
no cause chain is inferred. Message-only construction stores name and message
unchanged, without a stack or code; formatting an empty message falls back to
the supplied name.
Already-formatted thrown text is retained unchanged. Created diagnostics use
current epoch milliseconds and append without removing prior entries.

## Wire records and helpers

Messages and content serialize with their fixed role/type tags, even when used
alone. Tagged unions decode by their role/type. Optional absent fields are omitted;
explicit null is retained for supported thinking/routing fields and open tool
details. Signature
strings remain opaque; the separate signature metadata record accepts version 1.

`Usage::total_tokens` keeps the presence of the saved count. An omitted total
decodes, is never computed from the categories and is omitted again when the record
is written; an explicit null stays null, and a supplied number, including its sign
and fractional part, survives unchanged. `Usage::default` reports an explicit zero.
Readers keep their own rules: current context falls back to the category counts when
the total is missing or falsy, session statistics always sum the categories, and
file reports add a missing total as zero.

Model compatibility is one ordered open object, independent of the protocol identifier.
`ModelCompat` retains mixed-family and custom members and allows ordinary map edits.
The typed completion, response and message compatibility records convert into it with
`From`, omitting absent optional fields. Request decisions belong to the
[provider adapters](models/chat-completions.md), not shared-record decoding.

`short_hash` mixes supplied UTF-16 units with wrapping unsigned arithmetic and
emits two concatenated lower-case base-36 words. It is not a cryptographic hash.
`headers_to_record` copies an already-normalized name/value iterator; an exact
repeated key replaces its previous value, without parsing or HTTP normalization.
`string_enum` retains supplied order and nonempty description/default strings
without trimming, list validation or default-membership validation.

## Scripted models

The opt-in faux provider supplies offline queued responses, async factories and
ordered cancellable events through the ordinary invocation interface. See the
[scripted model guide](models/faux.md) for a two-turn tool flow and defaults.

## Resolved request authentication

`maestro-request::model_registry::ResolvedRequestAuth` owns the resolved key,
optional endpoint, ordered headers or denial string shared by the [native catalog](catalog.md#request-authentication)
and [guest authoring library](extensions/guest.md). Both consumers re-export the
same record; resolution policy remains with the producer.

## Provider registration records

`maestro-request::provider_config` owns `ProviderConfig<C>` and
`ProviderModelConfig`, the data a provider registration carries. `C` holds the
caller's callable operations and never enters the JSON form: the native catalog
uses `ProviderConfigInput`, which is `ProviderConfig<ProviderCallbacks>`, and the
guest authoring library uses `ProviderConfig` with no operations. The catalog
re-exports `ProviderModelConfig` as `ProviderModelInput` and the guest under its
own name. JSON field names are camel case, absent options are omitted, and empty
strings, lists, maps and `false` stay as written.

## Credential records

`maestro-request::oauth::OAuthCredentials` and the `maestro-request::auth_storage`
records (`ApiKeyCredential`, `AuthCredential`, `AuthStorageData`, `AuthSource` and
`AuthStatus`) are the [credential owner's](credentials.md) typed records, defined
once here and re-exported by models, credentials and the guest.
`maestro-request::oauth` also owns `OAuthPrompt`, `OAuthAuthInfo`,
`OAuthSelectOption` and `OAuthSelectPrompt`, re-exported by models and the guest.
These are plain interaction data; callable login operations stay with their
native or guest owners.

## Raw user-block text

[`UserBlock::text_output`](../crates/maestro-request/src/types/messages.rs)
joins text blocks in order with one newline, keeping empty blocks and repeated
text while ignoring images. It leaves text unformatted; consuming renderers and
protocol adapters own normalization, placeholders and presentation.

## Application message records

`maestro-request::messages` owns `BashExecutionMessage`, generic
`CustomMessage<T>`, `BranchSummaryMessage` and `CompactionSummaryMessage`.
`maestro-request::presence::Presence<T>` distinguishes missing, null and present
optional fields. Its `is_missing` predicate supports skipping omitted fields
in record serializers; serializing Missing on its own returns an error.

The guest re-exports these records, with `CustomMessage` selecting string details.
The native [conversation projection](sessions/messages.md) re-exports the shared
records and converts their model-visible content.

## Stored session entries

[Session interchange records](entry.md) are declared by `maestro-entry` and
re-exported by `maestro_session::session_manager`.
[Stored-entry context](sessions/session_manager.md) owns history and projection. Raw ordered JSON remains the
file-entry representation; typed views are explicit consuming reads. Shared
message records remain in `maestro-request`, and the existing
[conversation projection](sessions/messages.md) converts typed messages into
model input. `Presence<T>` is copyable when its payload is copyable.

`SessionManager` retains raw message values without decoding their payloads;
borrowed reads and explicit caller-owned copies follow the
[history interface](sessions/session_manager.md#native-history). Native files retain
raw records; header admission, historical conversion and index construction are
separate from per-operation payload decoding.
`NewSessionOptions` carries an optional explicit id and `Presence<String>` parent
metadata; it is a construction option, not a wire decoder.
Remaining appends, names, labels and tree reads follow the same raw-record rule:
see [appends](sessions/session_manager.md#appends-names-and-branch-summaries) and
the [tree guide](tree.md). Branch extraction and forking are described in
[the extraction guide](sessions/session_manager.md#branch-extraction-and-forking).

Tool metadata has one generic record owner, `maestro-request::tool_info::ToolInfo<P>`.
Native callers use its default parameter-schema owner through models; guest authoring
keeps a `ToolInfo` alias over the same owner. Live callback storage
belongs to `maestro-listeners::Listeners<P>`, with one admission per identity by
default or one per registration as an event-emitter listener list keeps them.
Agent subscriptions, session listeners, extension error delivery, terminal input
listeners and native output-failure handlers reuse it. For shared `Rc` callbacks it
also owns subscribing, which returns the removal
operation, and the live walk in registration order. What each listener receives
and when it runs remain with the caller. The collection returns removed ownership
for release outside guards, and it reclaims retired registrations while no
traversal runs, so one removal and a complete clear both stay cheap.


## Extension event records

`maestro-request::extension_events` owns generic event/result schemas and
`maestro-request::system_prompt` owns assembled prompt inputs. Models re-exports
both; the extension root also exports `BuildSystemPromptOptions`. Native and
guest specializations retain their existing domain operands;
stream snapshots and capability encoding remain adapter operations. The shared
Presence adapter delegates object-list admission to `maestro-json`.
