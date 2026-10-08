//! Contexts, session access and operation continuations through the author facade.
#![cfg(test)]
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

mod support;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::task::Poll;

use maestro_extensions_wasm::{
    AbortSignal, CompactOptions, ContentBlock, ExtensionAPI, ExtensionCommandContext,
    ExtensionUIContext, ExtensionUIContextPort, ForkOptions, MessageDelivery, Model, ModelRegistry,
    NewSessionCommandData, NewSessionCommandOptions, ReadonlySessionManager,
    ReplacedSessionContext, SendMessageOptions, SendUserMessageOptions, SessionChangeResult,
    SessionManager, SessionTreeNode, SwitchSessionOptions, UserContent, UserMessageDelivery,
};
use serde_json::{Value, json};
use support::controlled::{Catalog, ControlledHost, Flag, Hold, Log, Node, STALE, Script, Writer};
use support::guest_family::{self as typed, models, session};
use support::{block_on, poll_once};

/// Lines a callback wrote about the calls it saw.
type Seen = Rc<RefCell<Vec<String>>>;

/// The error the host reports while an extension is still loading.
const LOADING: &str =
    "Extension runtime not initialized. Action methods cannot be called during extension loading.";
/// The error the host reports before a context is bound.
const UNBOUND: &str = "Extension runtime not initialized";
/// The error the host reports for a context of a replaced session.
const STALE_CONTEXT: &str = "This extension ctx is stale after session replacement or reload. Do not use a captured maestro or command ctx after ctx.newSession(), ctx.fork(), ctx.switchSession(), or ctx.reload(). For newSession, fork, and switchSession, move post-replacement work into withSession and use the ctx passed to withSession. For reload, do not use the old ctx after await ctx.reload().";

/// An interface with no operations.
struct Screen;

impl ExtensionUIContextPort for Screen {}

/// A host and a script for contexts that answer from it.
fn scripted() -> (ControlledHost, Script) {
    let host = ControlledHost::new(Log::default());
    let script = host.script.clone();
    (host, script)
}

/// Scripts the first answers of every context operation that has one.
fn script_context(script: &Script) {
    script.reply("has_ui", Ok(false));
    script.reply("is_idle", Ok(true));
    script.reply("has_pending_messages", Ok(false));
    script.reply("get_system_prompt", Ok("first prompt".to_owned()));
    script.reply("model", Ok(None::<Model>));
    script.reply("get_context_usage", Ok(None::<session::ContextUsage>));
    script.reply("abort", Ok(()));
    script.reply("shutdown", Ok(()));
    script.reply("ui", Ok(ExtensionUIContext::new(Rc::new(Screen))));
}

/// Checks that getters read the host's state when they are called.
fn assert_live(context: &maestro_extensions_wasm::ExtensionContext, script: &Script) {
    assert_eq!(context.cwd(), Ok("/work".to_owned()));
    assert_eq!(context.has_ui(), Ok(false));
    assert_eq!(context.is_idle(), Ok(true));
    assert_eq!(context.get_system_prompt(), Ok("first prompt".to_owned()));
    script.reply("is_idle", Ok(false));
    script.reply("get_system_prompt", Ok("second prompt".to_owned()));
    let usage = session::ContextUsage {
        tokens: None,
        context_window: 1.0e5,
        percent: None,
    };
    script.reply("get_context_usage", Ok(Some(usage)));
    assert_eq!(
        context.is_idle(),
        Ok(false),
        "a getter reads the host's current state, not a cached value"
    );
    assert_eq!(context.get_system_prompt(), Ok("second prompt".to_owned()));
    let usage = context.get_context_usage().unwrap().unwrap();
    assert!(
        usage.tokens.is_none() && usage.percent.is_none(),
        "unknown usage stays unknown"
    );
    assert_eq!(context.model().unwrap().map(|model| model.id), None);
    assert!(
        context.has_pending_messages().is_ok()
            && context.abort().is_ok()
            && context.shutdown().is_ok()
    );
    assert!(context.ui().is_ok());
}

#[test]
fn maestro_context_getters_remain_live_and_errors_are_catchable() {
    let (_host, script) = scripted();
    let context = ControlledHost::context_with(&script);
    script_context(&script);
    assert_live(&context, &script);
    let options = CompactOptions {
        custom_instructions: Some("focus".into()),
        on_complete: None,
        on_error: None,
    };
    context.compact(Some(options)).unwrap_err();
    assert!(script.peek::<()>("compact").is_none());
    for message in [
        LOADING,
        UNBOUND,
        STALE_CONTEXT,
        "supplied by the host \u{1f4a5}",
    ] {
        script.reply::<bool>("is_idle", Err(message.to_owned()));
        assert_eq!(
            context.is_idle().unwrap_err(),
            message,
            "the host's text reaches the author unchanged"
        );
        script.reply("is_idle", Ok(true));
        assert_eq!(
            context.is_idle(),
            Ok(true),
            "after catching an error, a healthy call works"
        );
    }
}

#[test]
fn maestro_signal_outlives_callback_return() {
    let (_host, script) = scripted();
    let flag = Flag::default();
    let retained: Rc<RefCell<Option<AbortSignal>>> = Rc::default();
    let context = ControlledHost::context_with(&script);
    script.reply("signal", Ok(None::<AbortSignal>));
    assert!(
        context.signal().unwrap().is_none(),
        "no signal while the agent is not streaming"
    );
    script.reply("signal", Ok(Some(flag.signal())));
    {
        let callback_scope = context.signal().unwrap();
        *retained.borrow_mut() = callback_scope;
    }
    let held = retained.borrow().clone().unwrap();
    assert!(!held.aborted());
    flag.abort();
    assert!(
        held.aborted(),
        "a signal kept after its callback returned shows the later abort"
    );
    drop(held);
    let again = retained.borrow().clone().unwrap();
    assert!(
        again.aborted(),
        "dropping a clone neither aborts nor resets the host's flag"
    );
    let other = Flag::default();
    let facade = other.signal();
    drop(facade.clone());
    assert!(
        !other.signal().aborted(),
        "dropping a facade does not abort the operation"
    );
}

/// Scripts every read-only session operation and a two-level tree.
fn script_reader(script: &Script, entries: &[session::SessionEntry]) {
    script.reply("get_cwd", Ok("/c".to_owned()));
    script.reply("get_session_dir", Ok("/d".to_owned()));
    script.reply("get_session_id", Ok("id-\u{e9}".to_owned()));
    script.reply("get_session_file", Ok(None::<String>));
    script.reply("get_leaf_id", Ok(Some("9".to_owned())));
    script.reply("get_leaf_entry", Ok(Some(entries[8].clone())));
    script.reply("get_entry", Ok(None::<session::SessionEntry>));
    script.reply("get_label", Ok(Some("bookmark".to_owned())));
    script.reply("get_branch", Ok(entries.to_vec()));
    let header = session::SessionHeader {
        version: Some(3),
        id: "h".into(),
        timestamp: "t".into(),
        cwd: "/c".into(),
        parent_session: None,
    };
    script.reply("get_header", Ok(Some(header)));
    script.reply("get_entries", Ok(entries.to_vec()));
    script.reply("get_session_name", Ok(Some(String::new())));
    let child = SessionTreeNode::new(Rc::new(Node(script.clone(), "child")));
    let root = SessionTreeNode::new(Rc::new(Node(script.clone(), "root")));
    script.reply("root.entry", Ok(entries[0].clone()));
    script.reply("root.children", Ok(vec![child]));
    script.reply("root.label", Ok(Some("top".to_owned())));
    script.reply("root.label_timestamp", Ok(None::<String>));
    script.reply("child.entry", Ok(entries[1].clone()));
    script.reply("child.children", Ok(Vec::<SessionTreeNode>::new()));
    script.reply("child.label", Ok(None::<String>));
    script.reply(
        "child.label_timestamp",
        Ok(Some("2026-10-08T00:00:00Z".to_owned())),
    );
    script.reply("get_tree", Ok(vec![root]));
}

/// Checks the recursive tree the reader returns.
fn assert_tree(reader: &ReadonlySessionManager, entries: &[session::SessionEntry]) {
    let tree = reader.get_tree().unwrap();
    assert_eq!(tree[0].entry(), Ok(entries[0].clone()));
    assert_eq!(
        (tree[0].label(), tree[0].label_timestamp()),
        (Ok(Some("top".to_owned())), Ok(None))
    );
    let children = tree[0].children().unwrap();
    assert_eq!(children[0].entry(), Ok(entries[1].clone()));
    assert_eq!(
        children[0].label_timestamp(),
        Ok(Some("2026-10-08T00:00:00Z".to_owned()))
    );
}

/// Checks the queries that read one value of the session.
fn assert_scalars(reader: &ReadonlySessionManager, entries: &[session::SessionEntry]) {
    assert_eq!(reader.get_cwd(), Ok("/c".to_owned()));
    assert_eq!(reader.get_session_dir(), Ok("/d".to_owned()));
    assert_eq!(reader.get_session_id(), Ok("id-\u{e9}".to_owned()));
    assert_eq!(
        reader.get_session_file(),
        Ok(None),
        "an unpersisted session has no file"
    );
    assert_eq!(reader.get_leaf_id(), Ok(Some("9".to_owned())));
    assert_eq!(reader.get_leaf_entry(), Ok(Some(entries[8].clone())));
    assert_eq!(reader.get_entry("missing"), Ok(None));
    assert_eq!(reader.get_label("1"), Ok(Some("bookmark".to_owned())));
    assert_eq!(reader.get_header().unwrap().unwrap().version, Some(3));
    assert_eq!(
        reader.get_session_name(),
        Ok(Some(String::new())),
        "an empty name is a name"
    );
}

#[test]
fn maestro_session_reader_forwards_every_query() {
    let script = Script::new(Log::default());
    let reader = ReadonlySessionManager::new(Rc::new(Writer(script.clone())));
    let entries = typed::entries();
    script_reader(&script, &entries);
    assert_scalars(&reader, &entries);
    assert_eq!(reader.get_branch(Some("9")).unwrap().len(), 9);
    assert_eq!(reader.get_branch(None).unwrap().len(), 9);
    assert_eq!(reader.get_entries(), Ok(entries.clone()));
    assert_tree(&reader, &entries);
    script
        .reply::<Vec<session::SessionEntry>>("get_entries", Err("session unavailable".to_owned()));
    assert_eq!(reader.get_entries().unwrap_err(), "session unavailable");
    let calls = script.calls();
    assert!(calls.iter().any(|call| call == r#"get_entry("missing")"#));
    assert!(calls.iter().any(|call| call == r#"get_branch(Some("9"))"#));
    assert!(calls.iter().any(|call| call == "get_branch(None)"));
}

/// Scripts every write operation of the setup session.
fn script_writer(script: &Script, context: &session::SessionContext) {
    script.reply("set_session_file", Ok(()));
    script.reply("new_session", Ok(Some("/new.jsonl".to_owned())));
    script.reply("is_persisted", Ok(true));
    script.reply("get_children", Ok(Vec::<session::SessionEntry>::new()));
    script.reply("build_session_context", Ok(context.clone()));
    for operation in [
        "append_message",
        "append_thinking_level_change",
        "append_model_change",
        "append_compaction",
        "append_custom_entry",
        "append_session_info",
        "append_custom_message_entry",
        "append_label_change",
        "branch_with_summary",
    ] {
        script.reply(operation, Ok(format!("id-of-{operation}")));
    }
    script.reply("branch", Ok(()));
    script.reply("reset_leaf", Ok(()));
    script.reply("create_branched_session", Ok(None::<String>));
    script.reply("get_cwd", Ok("/reader".to_owned()));
}

/// Checks the operations that return the identifier of a new entry.
fn assert_appends(writer: &SessionManager) {
    let message = models::AgentMessage::Assistant(typed::assistant());
    assert_eq!(
        writer.append_message(message).unwrap(),
        "id-of-append_message"
    );
    assert_eq!(
        writer.append_thinking_level_change("high").unwrap(),
        "id-of-append_thinking_level_change"
    );
    assert_eq!(
        writer.append_model_change("p", "m").unwrap(),
        "id-of-append_model_change"
    );
    let compaction = session::CompactionResult {
        summary: "s".into(),
        first_kept_entry_id: "k".into(),
        tokens_before: 1.5,
        details: Some("0".into()),
    };
    assert_eq!(
        writer.append_compaction(compaction, Some(false)).unwrap(),
        "id-of-append_compaction"
    );
    assert_eq!(
        writer
            .append_custom_entry("kind", Some(json!({ "z": null })))
            .unwrap(),
        "id-of-append_custom_entry"
    );
    assert_eq!(
        writer.append_custom_entry("kind", None).unwrap(),
        "id-of-append_custom_entry"
    );
    assert_eq!(
        writer.append_session_info("name").unwrap(),
        "id-of-append_session_info"
    );
    let note = models::CustomMessageInput {
        custom_type: "c".into(),
        content: UserContent::Text("t".into()),
        display: false,
        details: None,
    };
    assert_eq!(
        writer.append_custom_message_entry(note).unwrap(),
        "id-of-append_custom_message_entry"
    );
    assert_eq!(
        writer.append_label_change("1", None).unwrap(),
        "id-of-append_label_change"
    );
    assert_eq!(
        writer
            .branch_with_summary(None, "sum", Some(json!(false)), None)
            .unwrap(),
        "id-of-branch_with_summary"
    );
}

#[test]
fn maestro_setup_writer_forwards_every_operation() {
    let script = Script::new(Log::default());
    let writer = SessionManager::new(Rc::new(Writer(script.clone())));
    let context = typed::empty_session_context();
    script_writer(&script, &context);
    assert_eq!(writer.set_session_file("/s.jsonl"), Ok(()));
    let options = session::NewSessionOptions {
        id: Some("fixed".into()),
        parent_session: None,
    };
    assert_eq!(
        writer.new_session(Some(options)),
        Ok(Some("/new.jsonl".to_owned()))
    );
    assert_eq!(writer.is_persisted(), Ok(true));
    assert_eq!(writer.get_children("p"), Ok(Vec::new()));
    assert_eq!(writer.build_session_context(), Ok(context));
    assert_appends(&writer);
    assert_eq!(writer.branch("1"), Ok(()));
    assert_eq!(writer.reset_leaf(), Ok(()));
    assert_eq!(writer.create_branched_session("leaf"), Ok(None));
    assert_eq!(
        writer.get_cwd(),
        Ok("/reader".to_owned()),
        "the writer also reads the session"
    );
    script.reply::<()>("branch", Err("cannot branch".to_owned()));
    assert_eq!(writer.branch("2").unwrap_err(), "cannot branch");
    let calls = script.calls();
    assert!(
        calls.contains(&r#"append_custom_entry("kind", Some(Object {"z": Null}))"#.to_owned()),
        "{calls:?}"
    );
    assert!(
        calls.contains(&r#"append_custom_entry("kind", None)"#.to_owned()),
        "present JSON and absence differ"
    );
    assert!(
        calls.contains(&r#"append_label_change("1", None)"#.to_owned()),
        "a label can be cleared"
    );
    assert!(calls.iter().any(|call| {
        call.starts_with("branch_with_summary(None, \"sum\", Some(Bool(false)), None)")
    }));
}

#[test]
fn maestro_catalog_queries_preserve_auth_outcomes() {
    let script = Script::new(Log::default());
    let catalog = ModelRegistry::new(Rc::new(Catalog(script.clone())));
    let model = typed::model();
    script.reply("get_all", Ok(vec![model.clone()]));
    script.reply("get_available", Ok(Vec::<Model>::new()));
    script.reply("find", Ok(None::<Model>));
    assert_eq!(catalog.get_all(), Ok(vec![model.clone()]));
    assert_eq!(catalog.get_available(), Ok(Vec::new()));
    assert_eq!(
        catalog.find("custom-provider", "unknown"),
        Ok(None),
        "a missing model is not an error"
    );
    script.reply("find", Ok(Some(model.clone())));
    assert_eq!(
        catalog
            .find("custom-provider", "m-1")
            .unwrap()
            .unwrap()
            .provider,
        "custom-provider"
    );
    let credentials = |key: Option<&str>, headers: Option<Vec<(String, String)>>| {
        session::ResolvedRequestAuth::Ok(session::RequestCredentials {
            api_key: key.map(Into::into),
            headers,
        })
    };
    for (reply, expected) in [
        (Ok(credentials(None, None)), "no key, no headers"),
        (
            Ok(credentials(None, Some(Vec::new()))),
            "no key, empty headers",
        ),
        (
            Ok(credentials(Some("k"), Some(vec![("h".into(), "v".into())]))),
            "key and headers",
        ),
        (
            Ok(session::ResolvedRequestAuth::Error(
                "auth failed \u{1f510}".into(),
            )),
            "structured failure",
        ),
    ] {
        script.reply("get_api_key_and_headers", reply.clone());
        assert_eq!(
            block_on(catalog.get_api_key_and_headers(model.clone())),
            reply,
            "{expected}"
        );
    }
    script.reply(
        "get_api_key_and_headers",
        Err::<session::ResolvedRequestAuth, _>("capability failed".to_owned()),
    );
    assert_eq!(
        block_on(catalog.get_api_key_and_headers(model)).unwrap_err(),
        "capability failed"
    );
}

/// Scripts the answers of the ordinary actions.
fn script_actions(script: &Script) {
    for operation in [
        "send_message",
        "send_user_message",
        "set_session_name",
        "set_label",
        "set_active_tools",
        "set_thinking_level",
    ] {
        script.reply(operation, Ok(()));
    }
    script.reply("get_session_name", Ok(None::<String>));
    script.reply("get_active_tools", Ok(vec!["read".to_owned()]));
    script.reply("get_all_tools", Ok(Vec::<session::ToolInfo>::new()));
    script.reply("get_commands", Ok(Vec::<session::SlashCommandInfo>::new()));
    script.reply("get_thinking_level", Ok(models::ThinkingLevel::Xhigh));
    script.reply("set_model", Ok(false));
}

/// Sends messages with every delivery variant and with none.
fn send_messages(api: &ExtensionAPI) {
    let note = |text: &str| models::CustomMessageInput {
        custom_type: "note".into(),
        content: UserContent::Text(text.into()),
        display: true,
        details: Some("null".into()),
    };
    for delivery in [
        MessageDelivery::Steer,
        MessageDelivery::FollowUp,
        MessageDelivery::NextTurn,
    ] {
        let options = SendMessageOptions {
            trigger_turn: Some(false),
            deliver_as: Some(delivery),
        };
        api.send_message(note("x"), Some(options)).unwrap();
    }
    api.send_message(note("y"), None).unwrap();
    let block = ContentBlock::Text(models::TextContent {
        text: "t".into(),
        text_signature: None,
    });
    let follow_up = SendUserMessageOptions {
        deliver_as: Some(UserMessageDelivery::FollowUp),
    };
    api.send_user_message(UserContent::Blocks(vec![block]), Some(follow_up))
        .unwrap();
    api.send_user_message(UserContent::Text(String::new()), None)
        .unwrap();
}

/// Checks the recorded sends and entries.
fn assert_recorded(calls: &[String]) {
    let sent: Vec<&String> = calls
        .iter()
        .filter(|call| call.starts_with("send_message("))
        .collect();
    assert_eq!(sent.len(), 4);
    assert!(
        sent[0].contains("MessageDelivery::Steer") && sent[1].contains("MessageDelivery::FollowUp")
    );
    assert!(sent[2].contains("MessageDelivery::NextTurn"));
    assert!(
        sent[0].contains("trigger-turn: Some(false)") && sent[3].ends_with(", None)"),
        "{sent:?}"
    );
    let entries: Vec<&String> = calls
        .iter()
        .filter(|call| call.starts_with("entry custom"))
        .collect();
    assert_eq!(
        entries,
        [
            "entry custom none",
            "entry custom null",
            "entry custom false",
            "entry custom 0",
            "entry custom \"\""
        ]
    );
    assert!(
        calls.contains(&r#"set_label("e1", None)"#.to_owned())
            && calls.contains(&r#"set_label("e1", Some(""))"#.to_owned())
    );
    assert!(
        calls.contains(&r#"set_active_tools(["a", "b"])"#.to_owned()),
        "{calls:?}"
    );
}

#[test]
fn maestro_session_actions_keep_delivery_and_payload_presence() {
    let (host, script) = scripted();
    let api: ExtensionAPI = host.api();
    script_actions(&script);
    send_messages(&api);
    for data in [
        None,
        Some(Value::Null),
        Some(json!(false)),
        Some(json!(0)),
        Some(json!("")),
    ] {
        api.append_entry("custom", data).unwrap();
    }
    api.set_session_name("\u{e9}").unwrap();
    assert_eq!(api.get_session_name(), Ok(None));
    api.set_label("e1", None).unwrap();
    api.set_label("e1", Some("")).unwrap();
    assert_eq!(api.get_active_tools(), Ok(vec!["read".to_owned()]));
    assert_eq!(api.get_all_tools(), Ok(Vec::new()));
    api.set_active_tools(&["a".to_owned(), "b".to_owned()])
        .unwrap();
    assert_eq!(api.get_commands(), Ok(Vec::new()));
    assert_eq!(
        block_on(api.set_model(typed::model())),
        Ok(false),
        "the host's boolean answer is returned as is"
    );
    assert_eq!(api.get_thinking_level(), Ok(models::ThinkingLevel::Xhigh));
    api.set_thinking_level(models::ThinkingLevel::Off).unwrap();
    assert_recorded(&host.log_lines());
}

/// Records which contexts a continuation saw and drops with a witness.
fn witness(
    seen: &Rc<RefCell<Vec<String>>>,
    label: &'static str,
    drops: &Rc<Cell<u32>>,
) -> maestro_extensions_wasm::WithSession {
    struct Dropped(Rc<Cell<u32>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let (seen, guard) = (Rc::clone(seen), Dropped(Rc::clone(drops)));
    Box::new(move |replaced: ReplacedSessionContext| {
        Box::pin(async move {
            let _guard = guard;
            seen.borrow_mut().push(format!(
                "{label} cwd={:?} parent-stale={:?}",
                replaced.cwd(),
                replaced.is_idle().is_ok()
            ));
            replaced
                .send_user_message(UserContent::Text(format!("from {label}")), None)
                .await
        })
    })
}

/// A setup closure that records that it ran before any continuation.
fn recording_setup(seen: &Seen) -> maestro_extensions_wasm::SetupSession {
    let seen = Rc::clone(seen);
    Box::new(move |manager: SessionManager| {
        Box::pin(async move {
            seen.borrow_mut().push(format!(
                "setup before continuation: {:?}",
                manager.is_persisted().is_err()
            ));
            Ok(())
        })
    })
}

/// Starts a new session and a fork, both held pending, then releases the fork first.
fn run_overlapping(
    host: &ControlledHost,
    ctx: &ExtensionCommandContext,
    seen: &Seen,
    drops: &Rc<Cell<u32>>,
) {
    let options = NewSessionCommandOptions {
        data: NewSessionCommandData {
            parent_session: Some("p".into()),
        },
        setup: Some(recording_setup(seen)),
        with_session: Some(witness(seen, "new", drops)),
    };
    let (hold_new, started_new, open_new) = hold();
    host.hold_next_session(hold_new);
    let mut first = Box::pin(ctx.new_session(Some(options)));
    assert!(matches!(poll_once(&mut first), Poll::Pending));
    let (hold_fork, started_fork, open_fork) = hold();
    host.hold_next_session(hold_fork);
    let fork = ForkOptions {
        data: session::ForkData { position: None },
        with_session: Some(witness(seen, "fork", drops)),
    };
    let mut second = Box::pin(ctx.fork("e1", Some(fork)));
    assert!(matches!(poll_once(&mut second), Poll::Pending));
    assert!(
        started_new.try_recv_ok() && started_fork.try_recv_ok(),
        "both operations are pending at once"
    );
    open_fork.send(()).unwrap();
    assert!(matches!(
        poll_once(&mut second),
        Poll::Ready(Ok(SessionChangeResult { cancelled: false }))
    ));
    assert_eq!(
        drops.get(),
        1,
        "the finished operation released its own continuation once"
    );
    open_new.send(()).unwrap();
    assert!(matches!(
        poll_once(&mut first),
        Poll::Ready(Ok(SessionChangeResult { cancelled: false }))
    ));
    assert_eq!(drops.get(), 2);
}

#[test]
fn maestro_replacement_continuations_keep_their_target_and_closure() {
    let (host, script) = scripted();
    let ctx = host.command_context();
    script.reply("new_session_target", Ok("/target-new".to_owned()));
    script.reply("fork_target", Ok("/target-fork".to_owned()));
    script.reply("switch_session_target", Ok("/target-switch".to_owned()));
    script.reply("wait_for_idle", Ok(()));
    script.reply("reload", Ok(()));
    let seen = Seen::default();
    let drops: Rc<Cell<u32>> = Rc::default();
    run_overlapping(&host, &ctx, &seen, &drops);
    assert_eq!(
        *seen.borrow(),
        [
            "fork cwd=Ok(\"/target-fork\") parent-stale=false",
            "setup before continuation: true",
            "new cwd=Ok(\"/target-new\") parent-stale=false",
        ],
        "each continuation received its own operation's target, and the setup ran before its continuation"
    );
    assert_eq!(
        ctx.cwd().unwrap_err(),
        STALE,
        "the old context reports the stale message"
    );
    script.reply(
        "switch_session",
        Ok(SessionChangeResult { cancelled: true }),
    );
    let fresh = host.command_context();
    let options = SwitchSessionOptions {
        with_session: Some(witness(&seen, "switch", &drops)),
    };
    let cancelled = block_on(fresh.switch_session("/other", Some(options)));
    assert_eq!(
        cancelled,
        Ok(SessionChangeResult { cancelled: true }),
        "a cancelled result is forwarded"
    );
    assert_eq!(
        seen.borrow().len(),
        3,
        "a cancelled operation runs no continuation"
    );
    assert_eq!(drops.get(), 3, "its continuation is still released once");
    assert_eq!(block_on(fresh.wait_for_idle()), Ok(()));
    assert_eq!(
        block_on(fresh.reload()),
        Ok(()),
        "a reload has no continuation to run"
    );
}

/// A hold and the channels that observe and release it.
fn hold() -> (Hold, Started, tokio::sync::oneshot::Sender<()>) {
    let (started_tx, started) = tokio::sync::oneshot::channel();
    let (open, opened) = tokio::sync::oneshot::channel();
    (
        Hold {
            started: started_tx,
            open: opened,
        },
        Started(started),
        open,
    )
}

/// Observes that an operation became pending.
struct Started(tokio::sync::oneshot::Receiver<()>);

impl Started {
    /// Whether the operation already signalled that it is pending.
    fn try_recv_ok(mut self) -> bool {
        self.0.try_recv().is_ok()
    }
}

/// Registers a shortcut that reads its context's directory; returns the flag it sets.
fn register_shortcut(api: &ExtensionAPI) -> Rc<Cell<bool>> {
    let ran = Rc::new(Cell::new(false));
    let flag = Rc::clone(&ran);
    let handler = Rc::new(move |ctx: maestro_extensions_wasm::ExtensionContext| {
        flag.set(ctx.cwd().is_ok());
        Box::pin(async { Ok(()) }) as maestro_extensions_wasm::ExtensionFuture<'static, ()>
    });
    let options = maestro_extensions_wasm::ShortcutOptions {
        description: None,
        handler,
    };
    api.register_shortcut("ctrl+k", options).unwrap();
    ran
}

/// Registers a command that waits for the agent to be idle and notes its directory.
fn register_idle_command(api: &ExtensionAPI) -> Seen {
    let saw = Seen::default();
    let record = Rc::clone(&saw);
    let handler: maestro_extensions_wasm::CommandHandler = Rc::new(move |args, ctx| {
        let record = Rc::clone(&record);
        Box::pin(async move {
            ctx.wait_for_idle().await?;
            record.borrow_mut().push(format!("{args} {}", ctx.cwd()?));
            Ok(())
        })
    });
    let options = maestro_extensions_wasm::CommandOptions {
        description: None,
        get_argument_completions: None,
        handler,
    };
    api.register_command("idle", options).unwrap();
    saw
}

/// Starts a session whose continuation starts another one; returns what the nested call did.
fn nest_new_session(ctx: &ExtensionCommandContext) -> Rc<RefCell<Vec<String>>> {
    let nested_result: Rc<RefCell<Vec<String>>> = Rc::default();
    let record = Rc::clone(&nested_result);
    let empty = || NewSessionCommandOptions {
        data: NewSessionCommandData {
            parent_session: None,
        },
        setup: None,
        with_session: None,
    };
    let continuation: maestro_extensions_wasm::WithSession =
        Box::new(move |replaced: ReplacedSessionContext| {
            Box::pin(async move {
                let before = replaced.cwd();
                let nested = replaced.new_session(Some(empty())).await;
                record
                    .borrow_mut()
                    .push(format!("{before:?} {nested:?} after={:?}", replaced.cwd()));
                Ok(())
            })
        });
    let options = NewSessionCommandOptions {
        with_session: Some(continuation),
        ..empty()
    };
    block_on(ctx.new_session(Some(options))).unwrap();
    nested_result
}

#[test]
fn maestro_command_capabilities_are_not_available_to_tools() {
    let (host, script) = scripted();
    let shortcut_ran = register_shortcut(&host.api());
    script.reply("wait_for_idle", Ok(()));
    let command_saw = register_idle_command(&host.api());
    let shortcut = host.shortcut("ctrl+k").unwrap();
    block_on((shortcut.handler)(ControlledHost::context())).unwrap();
    assert!(
        shortcut_ran.get(),
        "a shortcut runs against an ordinary context"
    );
    let command = host.command_handler("idle").unwrap();
    block_on(command("now".into(), host.command_context())).unwrap();
    assert_eq!(
        *command_saw.borrow(),
        ["now /work"],
        "a command runs against a command context"
    );
    script.reply("new_session_target", Ok("/inner".to_owned()));
    let nested = nest_new_session(&host.command_context());
    assert_eq!(
        *nested.borrow(),
        [format!(
            r#"Ok("/inner") Ok(SessionChangeResult {{ cancelled: false }}) after=Err({STALE:?})"#
        )],
        "a replacement context is command-capable and goes stale when it replaces the session"
    );
}

#[test]
fn maestro_retained_raw_resources_keep_their_own_lifetime() {
    let (host, script) = scripted();
    let reader = ReadonlySessionManager::new(Rc::new(Writer(script.clone())));
    let catalog = ModelRegistry::new(Rc::new(Catalog(script.clone())));
    let screen = ExtensionUIContext::new(Rc::new(Screen));
    let flag = Flag::default();
    script.reply("session_manager", Ok(reader));
    script.reply("model_registry", Ok(catalog));
    script.reply("ui", Ok(screen));
    script.reply("signal", Ok(Some(flag.signal())));
    script.reply("get_cwd", Ok("/retained".to_owned()));
    script.reply("find", Ok(None::<Model>));
    script.reply("new_session_target", Ok("/next".to_owned()));
    let ctx = host.command_context();
    let retained_reader = ctx.session_manager().unwrap();
    let retained_catalog = ctx.model_registry().unwrap();
    let retained_ui = ctx.ui().unwrap();
    let retained_signal = ctx.signal().unwrap().unwrap();
    let later_context = host.command_context();
    let options = NewSessionCommandOptions {
        data: NewSessionCommandData {
            parent_session: None,
        },
        setup: None,
        with_session: None,
    };
    block_on(ctx.new_session(Some(options))).unwrap();
    assert_eq!(
        ctx.cwd().unwrap_err(),
        STALE,
        "the replaced context is stale"
    );
    assert_eq!(
        retained_reader.get_cwd(),
        Ok("/retained".to_owned()),
        "a retained reader is not guarded by the context's staleness"
    );
    assert_eq!(retained_catalog.find("p", "m"), Ok(None));
    drop(retained_ui);
    assert!(!retained_signal.aborted());
    flag.abort();
    assert!(retained_signal.aborted());
    assert_eq!(
        later_context.cwd(),
        Ok("/work".to_owned()),
        "an older operation's staleness does not reach another context"
    );
    script.reply::<String>("get_cwd", Err(STALE_CONTEXT.to_owned()));
    assert_eq!(
        retained_reader.get_cwd().unwrap_err(),
        STALE_CONTEXT,
        "a host error stays a recoverable error value"
    );
    script.reply("get_cwd", Ok("/recovered".to_owned()));
    assert_eq!(retained_reader.get_cwd(), Ok("/recovered".to_owned()));
}
