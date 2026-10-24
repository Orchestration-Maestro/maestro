//! Strict mutation oracle replay through retained public handles.
use super::*;
use maestro_chat::{SessionFileOperations, TrashResult};
use maestro_tui::{Container, Input, interface::LocalFuture};
use std::io;

/// Exact callback transcript with closed operand shapes.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(untagged)]
enum Event {
    Confirmation((ConfirmationTag, Option<String>)),
    Deleted((DeleteTag, String)),
    DeleteVisible((DeleteTag, String, bool)),
    Renamed((RenameTag, String)),
    Error((ErrorTag, String)),
    Selected((SelectTag, String)),
    Cancelled((CancelTag,)),
    Exited((ExitTag,)),
}
/// Defines one admitted callback tag without permitting arbitrary strings.
macro_rules! event_tag {
    ($name:ident,$value:literal) => {
        #[derive(Debug, Deserialize, PartialEq)]
        enum $name {
            #[serde(rename=$value)]
            Tag,
        }
    };
}
event_tag!(ConfirmationTag, "confirmation");
event_tag!(DeleteTag, "delete");
event_tag!(RenameTag, "rename");
event_tag!(ErrorTag, "error");
event_tag!(SelectTag, "select");
event_tag!(CancelTag, "cancel");
event_tag!(ExitTag, "exit");
/// Complete expected mutation frame; every event operand has a closed type.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    lines: Vec<String>,
    selected: Option<String>,
    events: Vec<Event>,
}
impl PartialEq<Frame> for super::Frame {
    fn eq(&self, expected: &Frame) -> bool {
        let events: Vec<Event> = self
            .events
            .iter()
            .map(|event| serde_json::from_value(Value::Array(event.clone())).unwrap())
            .collect();
        self.lines == expected.lines
            && self.selected == expected.selected
            && events == expected.events
    }
}
/// Decodes this corpus using the shared strict envelope reader.
fn replay_cases<I: DeserializeOwned, E: DeserializeOwned>(name: &str, run: impl Fn(I, E)) {
    let _guard = registry::REGISTRY.lock().unwrap();
    let _restore = registry::Restore(get_keybindings());
    for (i, e) in decode_cases::<I, E>(include_str!("../../fixtures/session-mutations.json"), name)
    {
        set_keybindings(manager(Vec::new()));
        run(i, e);
    }
}
/// Two original default records with distinguishable identities.
fn records() -> Vec<maestro_app::SessionInfo> {
    vec![named("a", "Old"), named("b", "Other")]
}
/// Constructs mutation configuration without duplicating the selector host.
fn configured(configure: impl FnOnce(&mut SessionSelectorOptions)) -> Harness {
    Harness::configured(|o| {
        o.show_rename_hint = None;
        configure(o);
    })
}
/// Completes the ordinary population on the current discovery port.
fn ready(h: &Harness) {
    h.finish("current", records());
}
/// Supplies a no-op owned rename capability.
fn enable(o: &mut SessionSelectorOptions) {
    o.rename_session = Some(Rc::new(|_, _| Box::pin(async { Ok(()) })));
}
/// Captures each rename operand at factory invocation, before any polling.
fn capture(log: RenameLog) -> RenameCallback {
    Rc::new(move |path, value| {
        log.borrow_mut().push(vec![path.into(), value.into()]);
        Box::pin(async { Ok(()) })
    })
}
/// Captured save operands.
type RenameLog = Rc<RefCell<Vec<Vec<String>>>>;
/// Public save callable represented without nested option syntax.
type RenameCallback = Rc<dyn Fn(&str, &str) -> LocalFuture>;
/// Pending completion signals admitted in callback order.
type SaveGates = Rc<RefCell<std::collections::VecDeque<tokio::sync::oneshot::Receiver<()>>>>;
/// Captures operands immediately and waits on an independently controlled completion.
fn gated_save(log: RenameLog, gates: SaveGates, fail: bool) -> RenameCallback {
    Rc::new(move |path, value| {
        log.borrow_mut()
            .push(vec![path.to_owned(), value.to_owned()]);
        let gate = gates.borrow_mut().pop_front().unwrap();
        Box::pin(async move {
            gate.await.unwrap();
            if fail {
                return Err(io::Error::other("rename rejected").into());
            }
            Ok(())
        })
    })
}
/// Completes each currently pending refresh with a fresh default population.
fn finish_pending(h: &Harness) {
    for scope in ["current", "all"] {
        let loader = h.loader(scope);
        if loader
            .loads
            .borrow()
            .last()
            .is_some_and(|load| load.sender.is_some())
        {
            h.finish(scope, records());
        }
    }
}
/// Invokes the installed deletion callback and drives its admitted work.
fn delete(h: &Harness, path: &str) {
    let future = h.selector.get_session_list().on_delete_session().unwrap()(path);
    h.world.runtime.handle().spawn_local(future);
    h.drain();
}
/// Retains the actual editor through inherited child arrays.
fn rename_input(h: &Harness) -> Rc<dyn Component> {
    let children = h.selector.children();
    let panel = children.borrow()[3].clone();
    let panel = (panel.as_ref() as &dyn std::any::Any)
        .downcast_ref::<Container>()
        .unwrap();
    let input = panel.children().borrow()[2].clone();
    assert!((input.as_ref() as &dyn std::any::Any).is::<Input>());
    input
}
/// Observed effect call, retaining order and repeated operands.
#[derive(Debug, PartialEq)]
enum Call {
    Trash(Vec<String>),
    Unlink(String),
}
/// Controlled effects share only their actual call transcript.
struct Effects {
    calls: Rc<RefCell<Vec<Call>>>,
    success: bool,
    spawn_error: Option<String>,
    stderr: String,
    unlink_error: Option<String>,
    gate: RefCell<Option<tokio::sync::oneshot::Receiver<()>>>,
}
impl Default for Effects {
    fn default() -> Self {
        Self {
            calls: Rc::default(),
            success: true,
            spawn_error: None,
            stderr: String::new(),
            unlink_error: None,
            gate: RefCell::default(),
        }
    }
}
impl SessionFileOperations for Effects {
    fn trash(&self, args: &[String]) -> TrashResult {
        self.calls.borrow_mut().push(Call::Trash(args.to_vec()));
        self.spawn_error.as_ref().map_or_else(
            || TrashResult::Exited {
                success: self.success,
                stderr: self.stderr.clone(),
            },
            |error| TrashResult::Failed {
                error: io::Error::other(error.clone()).into(),
                stderr: self.stderr.clone(),
            },
        )
    }
    fn unlink(&self, path: String) -> LocalFuture {
        self.calls.borrow_mut().push(Call::Unlink(path));
        let error = self.unlink_error.clone();
        let gate = self.gate.take();
        Box::pin(async move {
            if let Some(gate) = gate {
                gate.await.unwrap();
            }
            if let Some(error) = error {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, error).into());
            }
            Ok(())
        })
    }
}
/// Installs the file-operation port before construction.
fn effects(o: &mut SessionSelectorOptions, port: &Rc<Effects>) {
    o.file_operations = Some(port.clone());
}
/// Checks exact deadlines on the controlled status owner.
fn delays(h: &Harness, expected: &[u64]) {
    assert_eq!(h.world.runtime.pending(), expected.len());
    for value in expected {
        assert_eq!(
            h.world.runtime.next_deadline(),
            Some(manual_runtime::ms(*value))
        );
    }
}
/// Defines closed per-group fixture records.
macro_rules! record {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields, rename_all = "camelCase")]
        struct $name { $( $field: $ty, )* }
    };
}
record!(Query {
    query: String,
    key: String
});
record!(Asked {
    asked: Frame,
    after: Frame
});
record!(Finish { finish: String });
record!(Confirmation {
    asked: Frame,
    blocked: Frame,
    after: Frame
});
record!(Text { value: String });
record!(Name { name: Option<String> });
record!(RenameExpected { opened: Frame, settled: Frame, renames: Vec<Vec<String>>, current_loads: usize });
record!(Saved { frame: Frame, renames: Vec<Vec<String>>, current_loads: usize });
record!(HintInput { callback: bool, show: Option<bool> });
record!(Pair {
    before: Frame,
    after: Frame
});
record!(Cancel { before: Frame, after: Frame, renames: Vec<Vec<String>> });
record!(EmptyView { empty: String });
record!(WinnerInput { winner: String });
record!(CollisionExpected {
    frame: Frame,
    all_loads: usize
});
record!(Manager {
    action: String,
    chosen: String
});
record!(ManagerFrames {
    wrong: Frame,
    after: Frame
});
record!(Width { width: usize });
record!(Layouts {
    opened: Frame,
    restored: Frame
});
record!(FocusState {
    parent: bool,
    list: bool
});
record!(FocusExpected { states: Vec<FocusState>, opened: Frame, unfocused: Frame, focused: Frame });

/// Each named witness dispatches only to its closed fixture decoder.
pub fn run(name: &str) {
    match name {
        "session_delete_keys_respect_query_ownership" => query(name),
        "session_confirmation_freezes_target_and_intercepts_controls" => confirmation(name),
        "session_empty_views_do_not_mutate" => empty_view(name),
        "session_mutation_collisions_keep_dispatch_precedence" => collision(name),
        "session_mutation_bindings_use_global_manager" => managers_replay(name),
        "session_rename_hint_defaults_to_callback_capability" => hint(name),
        "session_rename_prefills_and_saves_selected_path" => save(name),
        "session_rename_trims_ecmascript_whitespace" => trim(name),
        "session_rename_cancel_restores_query_without_saving" => cancel(name),
        "session_rename_reopening_preserves_edit_cursor" => cursor(name),
        "session_rename_layout_replaces_child_array" => layout_replay(name),
        "session_rename_focus_reaches_both_retained_inputs" => focus_replay(name),
        "session_rename_panel_keeps_prepared_text_and_live_borders" => palette_replay(name),
        "session_stale_rename_input_submission_does_not_save" => stale_input(name),
        "session_file_delete_passes_literal_trash_arguments" => literal(name),
        #[cfg(not(target_arch = "wasm32"))]
        "session_file_delete_selects_trash_or_unlink" => policy(name),
        #[cfg(not(target_arch = "wasm32"))]
        "session_file_delete_failure_combines_bounded_first_line_hint" => diagnostic(name),
        #[cfg(not(target_arch = "wasm32"))]
        "session_delete_success_updates_both_caches_and_status" => deleted(name),
        #[cfg(not(target_arch = "wasm32"))]
        "session_delete_failure_keeps_records_without_refresh" => delete_failed(name),
        "session_delete_refresh_failure_retains_local_removal" => refresh_failed(name),
        "session_rename_settles_callback_and_refresh_before_exit" => settled(name),
        #[cfg(not(target_arch = "wasm32"))]
        "session_mutation_refresh_uses_scope_at_effect_completion" => effect_scope(name),
        "session_mutation_refresh_rejects_obsolete_writers" => obsolete(name),
        "session_retained_list_keeps_mutation_callbacks" => retained_mutation(name),
        "session_rename_submissions_keep_independent_completions" => independent(name),
        "session_active_identity_refuses_deletion" => active(name),
        "session_mutation_slots_replace_clear_and_reenter" => slots(name),
        "session_rename_waits_for_visible_scope_load" => visible_loading(name),
        _ => panic!("unknown witness"),
    }
}
fn query(name: &str) {
    replay_cases(name, |i: Query, e: Asked| {
        let h = configured(|_| {});
        let mut record = session("a");
        record.first_message = "alpha".into();
        record.all_messages_text = "alpha".into();
        h.finish("current", vec![record]);
        let list = h.selector.get_session_list();
        let log = h.events.clone();
        list.set_on_delete_confirmation_change(Some(Rc::new(move |path| {
            log.borrow_mut()
                .push(vec!["confirmation".into(), path.into()]);
        })));
        let log = h.events.clone();
        list.set_on_delete_session(Some(Rc::new(move |path| {
            log.borrow_mut().push(vec!["delete".into(), path.into()]);
            Box::pin(async { Ok(()) })
        })));
        if !i.query.is_empty() {
            list.handle_input(&i.query);
        }
        list.handle_input(&i.key);
        assert_eq!(h.frame(120), e.asked);
        if i.query.is_empty() || i.key == "\x04" {
            list.handle_input("\r");
            h.drain();
        }
        assert_eq!(h.frame(120), e.after);
    });
}
fn confirmation(name: &str) {
    replay_cases(name, |i: Finish, e: Confirmation| {
        let h = configured(|_| {});
        ready(&h);
        let list = h.selector.get_session_list();
        let old = list.on_delete_confirmation_change().unwrap();
        let log = h.events.clone();
        list.set_on_delete_confirmation_change(Some(Rc::new(move |path| {
            old(path);
            log.borrow_mut()
                .push(vec!["confirmation".into(), path.into()]);
        })));
        let weak = Rc::downgrade(&h.selector);
        let log = h.events.clone();
        list.set_on_delete_session(Some(Rc::new(move |path| {
            let confirming = weak
                .upgrade()
                .unwrap()
                .render(120)
                .iter()
                .any(|line| line.contains("Delete session?"));
            log.borrow_mut()
                .push(vec!["delete".into(), path.into(), confirming.into()]);
            Box::pin(async { Ok(()) })
        })));
        list.handle_input("\x04");
        assert_eq!(h.frame(120), e.asked);
        for key in ["z", "\t", "\x13", "\x0e", "\x10", "\x12", "\x1b[B", "\x04"] {
            list.handle_input(key);
        }
        assert_eq!(h.frame(120), e.blocked);
        list.handle_input(if i.finish == "confirm" {
            "\r"
        } else {
            assert_eq!(i.finish, "cancel");
            "\x1b"
        });
        h.drain();
        assert_eq!(h.frame(120), e.after);
    });
}
fn empty_view(name: &str) {
    replay_cases(name, |i: EmptyView, e: Single| {
        let h = configured(|_| {});
        h.finish(
            "current",
            if i.empty == "records" {
                vec![]
            } else {
                assert_eq!(i.empty, "query");
                vec![session("a")]
            },
        );
        let list = h.selector.get_session_list();
        let log = h.events.clone();
        list.set_on_delete_confirmation_change(Some(Rc::new(move |p| {
            log.borrow_mut().push(vec!["confirmation".into(), p.into()]);
        })));
        let log = h.events.clone();
        list.set_on_rename_session(Some(Rc::new(move |p| {
            log.borrow_mut().push(vec!["rename".into(), p.into()]);
        })));
        if i.empty == "query" {
            list.handle_input("zzzz");
        }
        for key in ["\x04", "\x12", "\x1b[127;5u"] {
            list.handle_input(key);
        }
        assert_eq!(h.frame(120), e.frame);
    });
}
/// Installs observable mutation notifications without retaining the list itself.
fn log_slots(h: &Harness) {
    let list = h.selector.get_session_list();
    let log = h.events.clone();
    list.set_on_delete_confirmation_change(Some(Rc::new(move |p| {
        log.borrow_mut().push(vec!["confirmation".into(), p.into()]);
    })));
    let log = h.events.clone();
    list.set_on_rename_session(Some(Rc::new(move |p| {
        log.borrow_mut().push(vec!["rename".into(), p.into()]);
    })));
    let log = h.events.clone();
    list.set_on_delete_session(Some(Rc::new(move |p| {
        log.borrow_mut().push(vec!["delete".into(), p.into()]);
        Box::pin(async { Ok(()) })
    })));
}
fn collision(name: &str) {
    replay_cases(name, |i: WinnerInput, e: CollisionExpected| {
        let h = configured(|_| {});
        ready(&h);
        log_slots(&h);
        let config = [
            (
                "tui.input.tab",
                if i.winner == "scope" { "ctrl+x" } else { "tab" },
            ),
            (
                "app.session.delete",
                if matches!(i.winner.as_str(), "delete" | "scope") {
                    "ctrl+x"
                } else {
                    "ctrl+d"
                },
            ),
            (
                "app.session.rename",
                if i.winner == "confirm" {
                    "ctrl+r"
                } else {
                    "ctrl+x"
                },
            ),
            ("app.session.deleteNoninvasive", "ctrl+x"),
            (
                "tui.select.confirm",
                if i.winner == "confirm" {
                    "ctrl+x"
                } else {
                    "enter"
                },
            ),
            (
                "tui.select.cancel",
                if i.winner == "confirm" {
                    "ctrl+x"
                } else {
                    "escape"
                },
            ),
        ]
        .into_iter()
        .flat_map(|(id, key)| binding(id, Some(key)))
        .collect();
        set_keybindings(manager(config));
        if i.winner == "confirm" {
            h.selector.handle_input("\x04");
        }
        h.selector.handle_input("\x18");
        h.drain();
        finish_pending(&h);
        assert_eq!(h.frame(120), e.frame);
        assert_eq!(h.all.loads.borrow().len(), e.all_loads);
    });
}
fn managers_replay(name: &str) {
    replay_cases(name, |i: Manager, e: ManagerFrames| {
        set_keybindings(manager(binding(
            &i.action,
            (i.chosen == "global").then_some("ctrl+x"),
        )));
        let h = configured(|o| {
            o.keybindings = Some(manager(binding(
                &i.action,
                (i.chosen == "supplied").then_some("ctrl+x"),
            )));
        });
        ready(&h);
        log_slots(&h);
        h.selector.handle_input("\x1b[120;7u");
        assert_eq!(h.frame(120), e.wrong);
        h.selector.handle_input("\x18");
        assert_eq!(h.frame(120), e.after);
    });
}
fn hint(name: &str) {
    replay_cases(name, |i: HintInput, e: Pair| {
        let h = configured(|o| {
            o.show_rename_hint = i.show;
            if i.callback {
                enable(o);
            }
        });
        ready(&h);
        assert_eq!(h.frame(120), e.before);
        h.selector.handle_input("\x12");
        assert_eq!(h.frame(120), e.after);
    });
}
fn save(name: &str) {
    replay_cases(name, |i: Name, e: RenameExpected| {
        let log = Rc::new(RefCell::new(Vec::new()));
        let h = configured(|o| o.rename_session = Some(capture(log.clone())));
        let mut record = session("a");
        record.name = i.name.clone();
        h.finish("current", vec![record]);
        h.selector.handle_input("\x12");
        assert_eq!(h.frame(120), e.opened);
        h.selector.handle_input("X");
        h.selector.handle_input("\r");
        h.drain();
        let mut record = session("a");
        record.name = i.name;
        // The recording's loader returns its original snapshot, not the saved name.
        h.finish("current", vec![record]);
        assert_eq!(*log.borrow(), e.renames);
        assert_eq!(h.current.loads.borrow().len(), e.current_loads);
        assert_eq!(h.frame(120), e.settled);
    });
}
fn trim(name: &str) {
    replay_cases(name, |i: Text, e: Saved| {
        let log = Rc::new(RefCell::new(Vec::new()));
        let h = configured(|o| o.rename_session = Some(capture(log.clone())));
        let mut record = session("a");
        record.name = Some(i.value.clone());
        h.finish("current", vec![record]);
        h.selector.handle_input("\x12");
        h.selector.handle_input("\r");
        h.drain();
        if !log.borrow().is_empty() {
            let mut record = session("a");
            record.name = Some(i.value);
            h.finish("current", vec![record]);
        }
        assert_eq!(*log.borrow(), e.renames);
        assert_eq!(h.current.loads.borrow().len(), e.current_loads);
        assert_eq!(h.frame(120), e.frame);
    });
}
fn cancel(name: &str) {
    replay_cases(name, |_: Empty, e: Cancel| {
        let log = Rc::new(RefCell::new(Vec::new()));
        let h = configured(|o| o.rename_session = Some(capture(log.clone())));
        let mut record = named("a", "Old");
        record.first_message = "needle".into();
        record.all_messages_text = "needle".into();
        h.finish("current", vec![record]);
        h.selector.handle_input("needle");
        assert_eq!(h.frame(120), e.before);
        h.selector.handle_input("\x12");
        h.selector.handle_input("X");
        h.selector.handle_input("\x1b");
        assert_eq!(h.frame(120), e.after);
        assert_eq!(*log.borrow(), e.renames);
    });
}
fn cursor(name: &str) {
    replay_cases(name, |i: Name, e: Single| {
        let h = configured(enable);
        let mut record = session("a");
        record.name = i.name;
        h.finish("current", vec![record]);
        for key in ["\x12", "\x1b[C", "\x1b", "\x12", "X"] {
            h.selector.handle_input(key);
        }
        assert_eq!(h.frame(120), e.frame);
    });
}
fn layout_replay(name: &str) {
    replay_cases(name, |i: Width, e: Layouts| {
        let h = configured(enable);
        ready(&h);
        let old = h.selector.children();
        h.selector.handle_input("\x12");
        old.borrow_mut().push(Rc::new(maestro_tui::Text::new(
            "old alias".into(),
            0,
            0,
            None,
        )));
        assert_eq!(h.frame(i.width), e.opened);
        let editing = h.selector.children();
        h.selector.handle_input("\x1b");
        editing.borrow_mut().push(Rc::new(maestro_tui::Text::new(
            "stale rename alias".into(),
            0,
            0,
            None,
        )));
        assert_eq!(h.frame(i.width), e.restored);
    });
}
fn focus_replay(name: &str) {
    replay_cases(name, |_: Empty, e: FocusExpected| {
        let h = configured(enable);
        ready(&h);
        assert_eq!(e.states.len(), 3);
        for (value, state) in [false, true, false].into_iter().zip(e.states) {
            h.selector.focus_flag().set(value);
            assert_eq!(h.selector.focus_flag().get(), state.parent);
            assert_eq!(h.selector.get_session_list().focus_flag().get(), state.list);
        }
        h.selector.handle_input("\x12");
        assert_eq!(h.frame(120), e.opened);
        h.selector.focus_flag().set(false);
        assert_eq!(h.frame(120), e.unfocused);
        h.selector.focus_flag().set(true);
        assert_eq!(h.frame(120), e.focused);
    });
}
fn palette_replay(name: &str) {
    replay_cases(name, |_: Empty, e: Pair| {
        let h = configured(enable);
        ready(&h);
        h.selector.handle_input("\x12");
        assert_eq!(h.frame(120), e.before);
        h.world.state.set_theme_instance(palette(10)).unwrap();
        assert_eq!(h.frame(120), e.after);
    });
}
fn stale_input(name: &str) {
    replay_cases(name, |_: Empty, e: Saved| {
        let log = Rc::new(RefCell::new(Vec::new()));
        let h = configured(|o| o.rename_session = Some(capture(log.clone())));
        ready(&h);
        h.selector.handle_input("\x12");
        let input = rename_input(&h);
        h.selector.handle_input("\x1b");
        input.input_handler().unwrap().handle_input("\r");
        h.drain();
        assert_eq!(h.frame(120), e.frame);
        assert_eq!(*log.borrow(), e.renames);
        assert_eq!(h.current.loads.borrow().len(), e.current_loads);
    });
}

record!(DeletePath { path: String });
record!(EffectInput { status: Option<i32>, error: Option<String>, stderr: Option<String>, exists: Option<bool> });
record!(Encoding { encoding: String });
/// Closed recorded process/file call forms; tuple arity is checked by serde.
#[derive(Deserialize)]
#[serde(untagged)]
enum RecordedCall {
    Spawn((String, String, Vec<String>, Encoding)),
    Path((String, String)),
}
record!(DeleteResult { ok: bool, method: String, error: Option<String> });
record!(DeleteExpected { calls: Vec<RecordedCall>, result: DeleteResult });
record!(Method { method: String });
record!(Deleted { calls: Vec<RecordedCall>, removed: Frame, other: Frame, unexpired: Frame, expired: Frame, delays: Vec<u64>, current_loads: usize, all_loads: usize });
record!(DeleteFailed { calls: Vec<RecordedCall>, failed: Frame, unexpired: Frame, expired: Frame, delays: Vec<u64>, current_loads: usize });
record!(RefreshFailed { failed: Frame, unexpired: Frame, expired: Frame, delays: Vec<u64> });
/// Validates recorded command identity/options and preserves every ordered operand. The
/// recorded session path `from` names the real file `to`; the existence check reads the
/// file system, so the port no longer observes it.
fn calls(port: &Effects, expected: Vec<RecordedCall>, (from, to): (&str, &str)) {
    let real = |path: String| if path == from { to.to_owned() } else { path };
    let expected: Vec<_> = expected
        .into_iter()
        .filter_map(|call| match call {
            RecordedCall::Spawn((tag, program, args, encoding)) => {
                assert_eq!(tag, "spawn");
                assert_eq!(program, "trash");
                assert_eq!(encoding.encoding, "utf-8");
                Some(Call::Trash(args.into_iter().map(real).collect()))
            }
            RecordedCall::Path((tag, path)) => match tag.as_str() {
                "exists" => None,
                "unlink" => Some(Call::Unlink(real(path))),
                _ => panic!("invalid effect"),
            },
        })
        .collect();
    assert_eq!(*port.calls.borrow(), expected);
}
/// A real session file named like the recorded `name`, created unless it should be
/// absent, with the tree that owns it.
#[cfg(not(target_arch = "wasm32"))]
fn session_file(name: &str, present: bool) -> (maestro_test_directory::Scratch, String) {
    let dir = maestro_test_directory::Scratch::new().unwrap();
    let path = dir.path().join(name);
    if present {
        std::fs::write(&path, "").unwrap();
    }
    (dir, path.to_str().unwrap().to_owned())
}
/// The recorded frame, with the recorded selection `from` naming the real file `to`.
fn moved(mut frame: Frame, (from, to): (&str, &str)) -> Frame {
    if frame.selected.as_deref() == Some(from) {
        frame.selected = Some(to.into());
    }
    frame
}
/// The default records, with session `a` stored at `path`.
fn records_at(path: &str) -> Vec<maestro_app::SessionInfo> {
    let mut records = records();
    records[0].path = path.into();
    records
}
/// Uses status, membership and refresh choice to observe the private deletion result.
fn check_delete(h: &Harness, result: DeleteResult) {
    let frame = h.selector.render(2000).join("\n");
    assert_eq!(
        result.method,
        if frame.contains("Session moved to trash") {
            "trash"
        } else {
            "unlink"
        }
    );
    if result.ok {
        assert!(result.error.is_none());
        assert!(frame.contains("Session moved to trash") || frame.contains("Session deleted"));
        assert_eq!(h.current.loads.borrow().len(), 2);
    } else {
        assert!(frame.contains(&format!("Failed to delete: {}", result.error.unwrap())));
        assert_eq!(h.current.loads.borrow().len(), 1);
    }
}
fn literal(name: &str) {
    replay_cases(name, |i: DeletePath, e: DeleteExpected| {
        let port = Rc::new(Effects::default());
        let h = configured(|o| effects(o, &port));
        ready(&h);
        delete(&h, &i.path);
        calls(&port, e.calls, ("", ""));
        check_delete(&h, e.result);
        finish_pending(&h);
    });
}
#[cfg(not(target_arch = "wasm32"))]
fn policy(name: &str) {
    replay_cases(name, |i: EffectInput, e: DeleteExpected| {
        let port = Rc::new(Effects {
            success: i.status == Some(0),
            spawn_error: i.error,
            stderr: i.stderr.unwrap_or_default(),
            ..Effects::default()
        });
        let (_dir, path) = session_file("session.jsonl", i.exists.unwrap_or(true));
        let h = configured(|o| effects(o, &port));
        ready(&h);
        delete(&h, &path);
        calls(&port, e.calls, ("/session.jsonl", &path));
        check_delete(&h, e.result);
        finish_pending(&h);
    });
}
#[cfg(not(target_arch = "wasm32"))]
fn diagnostic(name: &str) {
    replay_cases(name, |i: EffectInput, e: DeleteExpected| {
        assert!(i.status.is_none());
        assert!(i.exists.is_none());
        let port = Rc::new(Effects {
            success: false,
            spawn_error: i.error,
            stderr: i.stderr.unwrap_or_default(),
            unlink_error: Some("unlink denied".into()),
            ..Effects::default()
        });
        let (_dir, path) = session_file("session.jsonl", true);
        let h = configured(|o| effects(o, &port));
        ready(&h);
        delete(&h, &path);
        calls(&port, e.calls, ("/session.jsonl", &path));
        check_delete(&h, e.result);
    });
}
#[cfg(not(target_arch = "wasm32"))]
fn deleted(name: &str) {
    replay_cases(name, |i: Method, e: Deleted| {
        let port = Rc::new(Effects {
            success: i.method == "trash",
            ..Effects::default()
        });
        assert!(matches!(i.method.as_str(), "trash" | "unlink"));
        let (_dir, path) = session_file("a.jsonl", true);
        let h = configured(|o| effects(o, &port));
        h.finish("current", records_at(&path));
        h.selector.handle_input("\t");
        h.finish("all", records_at(&path));
        h.selector.handle_input("\t");
        h.selector.handle_input("hello");
        delete(&h, &path);
        calls(&port, e.calls, ("/missing/a.jsonl", &path));
        assert_eq!(h.frame(120), e.removed);
        delays(&h, &e.delays);
        h.finish("current", vec![named("b", "Other")]);
        h.selector.handle_input("\t");
        assert_eq!(h.frame(120), e.other);
        advance(&h.world.runtime, 1999);
        assert_eq!(h.frame(120), e.unexpired);
        advance(&h.world.runtime, 2000);
        assert_eq!(h.frame(120), e.expired);
        assert_eq!(h.current.loads.borrow().len(), e.current_loads);
        assert_eq!(h.all.loads.borrow().len(), e.all_loads);
    });
}
#[cfg(not(target_arch = "wasm32"))]
fn delete_failed(name: &str) {
    replay_cases(name, |_: Empty, e: DeleteFailed| {
        let port = Rc::new(Effects {
            success: false,
            stderr: "trash denied\nsecond".into(),
            unlink_error: Some("unlink denied".into()),
            ..Effects::default()
        });
        let (_dir, path) = session_file("a.jsonl", true);
        let recorded = ("/missing/a.jsonl", path.as_str());
        let h = configured(|o| effects(o, &port));
        h.finish("current", records_at(&path));
        h.selector.handle_input("hello");
        delete(&h, &path);
        calls(&port, e.calls, recorded);
        assert_eq!(h.frame(120), moved(e.failed, recorded));
        delays(&h, &e.delays);
        advance(&h.world.runtime, 2999);
        assert_eq!(h.frame(120), moved(e.unexpired, recorded));
        advance(&h.world.runtime, 3000);
        assert_eq!(h.frame(120), moved(e.expired, recorded));
        assert_eq!(h.current.loads.borrow().len(), e.current_loads);
    });
}
fn refresh_failed(name: &str) {
    replay_cases(name, |_: Empty, e: RefreshFailed| {
        let port = Rc::new(Effects::default());
        let h = configured(|o| effects(o, &port));
        ready(&h);
        delete(&h, "/missing/a.jsonl");
        fail(&h, "current", "refresh rejected");
        assert_eq!(h.frame(120), e.failed);
        delays(&h, &e.delays);
        advance(&h.world.runtime, 3999);
        assert_eq!(h.frame(120), e.unexpired);
        advance(&h.world.runtime, 4000);
        assert_eq!(h.frame(120), e.expired);
    });
}
record!(Outcome { outcome: String });
record!(SaveCompletion { saving:Frame, refreshing:Frame, settled:Frame, renames:Vec<Vec<String>>, error:Option<String>, current_loads:usize, delays:Vec<u64> });
fn settled(name: &str) {
    replay_cases(name, |i: Outcome, e: SaveCompletion| {
        let (sender, receiver) = tokio::sync::oneshot::channel::<()>();
        let gates = Rc::new(RefCell::new(std::collections::VecDeque::from([receiver])));
        let log = Rc::new(RefCell::new(Vec::new()));
        let h = configured(|o| {
            o.rename_session = Some(gated_save(log.clone(), gates, i.outcome == "rename-error"));
        });
        ready(&h);
        h.selector.handle_input("\x12");
        let input = rename_input(&h);
        let input = (input.as_ref() as &dyn std::any::Any)
            .downcast_ref::<Input>()
            .unwrap();
        (input.on_submit().unwrap())(" New ");
        h.drain();
        assert_eq!(h.frame(120), e.saving);
        sender.send(()).unwrap();
        let errors = h.world.runtime.drain_futures();
        assert_eq!(h.frame(120), e.refreshing);
        if i.outcome != "rename-error" {
            if i.outcome == "refresh-error" {
                fail(&h, "current", "refresh rejected");
            } else {
                assert_eq!(i.outcome, "success");
                h.finish("current", vec![named("a", "New")]);
            }
        }
        assert_eq!(errors.first().map(ToString::to_string), e.error);
        assert_eq!(errors.len(), usize::from(e.error.is_some()));
        assert_eq!(*log.borrow(), e.renames);
        assert_eq!(h.frame(120), e.settled);
        assert_eq!(h.current.loads.borrow().len(), e.current_loads);
        delays(&h, &e.delays);
    });
}
record!(Mutation { mutation: String });
record!(ScopeCompletion {
    frame: Frame,
    current_loads: usize,
    all_loads: usize
});
/// The default records, with session `a` stored at `path` when it is deleted.
fn scope_records(mutation: &str, path: &str) -> Vec<maestro_app::SessionInfo> {
    if mutation == "delete" {
        records_at(path)
    } else {
        records()
    }
}
#[cfg(not(target_arch = "wasm32"))]
fn effect_scope(name: &str) {
    replay_cases(name, |i: Mutation, e: ScopeCompletion| {
        let (sender, receiver) = tokio::sync::oneshot::channel::<()>();
        let port = Effects {
            success: false,
            ..Effects::default()
        };
        let gate = Rc::new(RefCell::new(None));
        if i.mutation == "delete" {
            port.gate.replace(Some(receiver));
        } else {
            assert_eq!(i.mutation, "rename");
            gate.replace(Some(receiver));
        }
        let port = Rc::new(port);
        let save: RenameCallback = Rc::new(move |_, _| {
            let gate = gate.take().unwrap();
            Box::pin(async move {
                gate.await.unwrap();
                Ok(())
            })
        });
        let (_dir, path) = session_file("a.jsonl", true);
        let h = configured(|o| {
            effects(o, &port);
            o.rename_session = Some(save);
        });
        h.finish("current", scope_records(&i.mutation, &path));
        if i.mutation == "delete" {
            delete(&h, &path);
        } else {
            h.selector.handle_input("\x12");
            let input = rename_input(&h);
            (input.as_ref() as &dyn std::any::Any)
                .downcast_ref::<Input>()
                .unwrap()
                .set_value("New".into());
            h.selector.handle_input("\r");
            h.drain();
            h.selector.handle_input("\x1b");
        }
        h.selector.handle_input("\t");
        h.finish("all", scope_records(&i.mutation, &path));
        sender.send(()).unwrap();
        h.drain();
        assert_eq!(h.all.loads.borrow().len(), 2);
        h.finish(
            "all",
            if i.mutation == "delete" {
                vec![named("b", "Other")]
            } else {
                records()
            },
        );
        assert_eq!(h.frame(120), e.frame);
        assert_eq!(h.current.loads.borrow().len(), e.current_loads);
        assert_eq!(h.all.loads.borrow().len(), e.all_loads);
    });
}
record!(OldOutcome {
    scope: String,
    old_outcome: String
});
record!(RequestFrames { before:Frame,pending_progress:Frame,newest:Frame,obsolete:Frame,attempt:Option<Frame>,progress:Frame,roundtrip:Frame,delays:Vec<u64> });
/// Settles a chosen admitted request, preserving the original producer error.
fn finish_at(h: &Harness, scope: &str, index: usize, result: DiscoveryResult) {
    let sender = h.loader(scope).loads.borrow_mut()[index]
        .sender
        .take()
        .unwrap();
    assert!(sender.send(result).is_ok());
    h.drain();
}
fn obsolete(name: &str) {
    replay_cases(name, |i: OldOutcome, e: RequestFrames| {
        let port = Rc::new(Effects::default());
        let h = configured(|o| {
            enable(o);
            effects(o, &port);
        });
        h.finish(
            "current",
            vec![session("a"), session("b"), named("c", "Kept")],
        );
        if i.scope == "all" {
            h.selector.handle_input("\t");
            h.finish("all", vec![session("a"), session("b"), named("c", "Kept")]);
        } else {
            assert_eq!(i.scope, "current");
        }
        delete(&h, "/missing/a.jsonl");
        let old = h.loader(&i.scope).loads.borrow().len() - 1;
        delete(&h, "/missing/b.jsonl");
        let latest = h.loader(&i.scope).loads.borrow().len() - 1;
        assert_ne!(old, latest);
        assert_eq!(h.frame(120), e.before);
        let progress = h.loader(&i.scope).loads.borrow()[old].progress.clone();
        progress(9, 99);
        assert_eq!(h.frame(120), e.pending_progress);
        settle_obsolete(&h, &i, &e, [old, latest]);
        progress(9, 99);
        assert_eq!(h.frame(120), e.progress);
        h.selector.handle_input("\t");
        if i.scope == "current" {
            h.finish("all", vec![session("other")]);
        }
        h.selector.handle_input("\t");
        assert_eq!(h.frame(120), e.roundtrip);
        delays(&h, &e.delays);
    });
}
/// Checks both completion orders while retaining the newest request's loading state.
fn settle_obsolete(h: &Harness, i: &OldOutcome, e: &RequestFrames, [old, latest]: [usize; 2]) {
    if i.old_outcome == "before-new" {
        finish_at(h, &i.scope, old, Ok(vec![named("stale", "Stale")]));
        assert_eq!(h.frame(120), e.obsolete);
        h.selector.handle_input("\x12");
        assert_eq!(h.frame(120), *e.attempt.as_ref().unwrap());
        finish_at(h, &i.scope, latest, Ok(vec![named("fresh", "Fresh")]));
        assert_eq!(h.frame(120), e.newest);
    } else {
        assert!(e.attempt.is_none());
        finish_at(h, &i.scope, latest, Ok(vec![named("fresh", "Fresh")]));
        assert_eq!(h.frame(120), e.newest);
        let result = if i.old_outcome == "error" {
            Err(io::Error::other("obsolete failed").into())
        } else {
            assert_eq!(i.old_outcome, "success");
            Ok(vec![named("stale", "Stale")])
        };
        finish_at(h, &i.scope, old, result);
        assert_eq!(h.frame(120), e.obsolete);
    }
}
record!(RetainedMutation { lines:Vec<String>,selected:Option<String>,events:Vec<Vec<Value>>,current_loads:usize });
fn retained_mutation(name: &str) {
    replay_cases(name, |_: Empty, e: RetainedMutation| {
        let port = Rc::new(Effects::default());
        let capture = Rc::new(Cell::new(0));
        let weak = Rc::downgrade(&capture);
        let h = configured(|o| {
            effects(o, &port);
            o.request_render = Rc::new(move || capture.set(capture.get() + 1));
        });
        ready(&h);
        let Harness {
            selector,
            current,
            all: _,
            events,
            world,
        } = h;
        let list = selector.get_session_list();
        let weak_list = Rc::downgrade(&list);
        drop(selector);
        world
            .runtime
            .handle()
            .spawn_local(list.on_delete_session().unwrap()("/missing/a.jsonl"));
        assert!(world.runtime.drain_futures().is_empty());
        current.finish(Ok(records()));
        assert!(world.runtime.drain_futures().is_empty());
        list.handle_input("\r");
        assert_eq!(list.render(120), e.lines);
        assert_eq!(list.get_selected_session_path(), e.selected);
        assert_eq!(*events.borrow(), e.events);
        assert_eq!(current.loads.borrow().len(), e.current_loads);
        assert!(weak.upgrade().is_some());
        drop(list);
        assert!(weak_list.upgrade().is_none());
        assert!(weak.upgrade().is_none());
        assert_eq!(world.runtime.futures(), 0);
    });
}
fn independent(name: &str) {
    replay_cases(name, |_: Empty, e: Saved| {
        let log = Rc::new(RefCell::new(Vec::new()));
        let (s0, r0) = tokio::sync::oneshot::channel::<()>();
        let (s1, r1) = tokio::sync::oneshot::channel::<()>();
        let gates = Rc::new(RefCell::new(std::collections::VecDeque::from([r0, r1])));
        let h = configured(|o| {
            o.rename_session = Some(gated_save(log.clone(), gates, false));
        });
        ready(&h);
        h.selector.handle_input("\x12");
        h.selector.handle_input("\r");
        h.selector.handle_input("\r");
        h.drain();
        assert_eq!(log.borrow().len(), 2);
        s1.send(()).unwrap();
        h.drain();
        h.finish("current", records());
        s0.send(()).unwrap();
        h.drain();
        h.finish("current", records());
        assert_eq!(*log.borrow(), e.renames);
        assert_eq!(h.frame(120), e.frame);
        assert_eq!(h.current.loads.borrow().len(), e.current_loads);
    });
}

record!(Alias { alias: bool });
record!(ActiveFrames { refused:Frame,unexpired:Frame,expired:Frame,delays:Vec<u64> });
/// Expands only disposable authored root fields, never rendered text normalization.
fn expand_frame(mut frame: Frame, root: &str) -> Frame {
    frame.selected = frame.selected.map(|path| path.replace("$FS", root));
    frame
}
fn active(name: &str) {
    replay_cases(name, |i: Alias, e: ActiveFrames| {
        #[cfg(not(unix))]
        if i.alias {
            return;
        }
        let fs = Filesystem::new();
        let root = fs.root.clone();
        std::fs::create_dir_all(format!("{root}/real")).unwrap();
        std::fs::write(format!("{root}/real/a.jsonl"), b"x").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(format!("{root}/real"), format!("{root}/alias")).unwrap();
        let path = format!("{root}/{}/a.jsonl", if i.alias { "alias" } else { "real" });
        let h = configured(|o| o.current_session_file_path = Some(format!("{root}/real/a.jsonl")));
        let mut record = session("a");
        record.path = path;
        h.finish("current", vec![record]);
        let list = h.selector.get_session_list();
        let original = list.on_error().unwrap();
        let events = h.events.clone();
        list.set_on_error(Some(Rc::new(move |message| {
            events
                .borrow_mut()
                .push(vec!["error".into(), message.into()]);
            original(message);
        })));
        h.selector.handle_input("\x04");
        assert_eq!(h.frame(120), expand_frame(e.refused, &root));
        delays(&h, &e.delays);
        advance(&h.world.runtime, 2999);
        assert_eq!(h.frame(120), expand_frame(e.unexpired, &root));
        advance(&h.world.runtime, 3000);
        assert_eq!(h.frame(120), expand_frame(e.expired, &root));
    });
}
record!(Slot { slot: String });
record!(SlotResult { initial_absent:bool,events:Vec<Value>,absent:bool });
fn slots(name: &str) {
    replay_cases(name, |i: Slot, e: SlotResult| {
        let world = World::new();
        let list = world.list(
            vec![session("a")],
            (false, SortMode::Threaded, NameFilter::All),
            None,
        );
        let events = Rc::new(RefCell::new(Vec::<Value>::new()));
        match i.slot.as_str() {
            "onDeleteConfirmationChange" => confirmation_slot(&list, &e, events.clone()),
            "onDeleteSession" => delete_slot(&list, &e, events.clone(), &world.runtime),
            "onRenameSession" | "onError" => {
                notification_slot(&list, &e, events.clone(), i.slot == "onError");
            }
            _ => panic!("invalid slot"),
        }
        assert_eq!(*events.borrow(), e.events);
    });
}
/// Borrowed confirmation notification.
type ConfirmCallback = Rc<dyn Fn(Option<&str>)>;
/// Synchronous operand notification.
type NotifyCallback = Rc<dyn Fn(&str)>;
/// Independently replaceable notification accessors.
type NotifyAccess = (
    fn(&maestro_chat::SessionList) -> Option<NotifyCallback>,
    fn(&maestro_chat::SessionList, Option<NotifyCallback>),
);
/// Checks replacement, callable identity and callback-driven clearing.
fn confirmation_slot(
    list: &Rc<maestro_chat::SessionList>,
    expected: &SlotResult,
    log: Rc<RefCell<Vec<Value>>>,
) {
    let weak = Rc::downgrade(list);

    assert_eq!(
        list.on_delete_confirmation_change().is_none(),
        expected.initial_absent
    );
    let callback: ConfirmCallback = Rc::new(move |value| {
        log.borrow_mut().push(value.into());
        weak.upgrade()
            .unwrap()
            .set_on_delete_confirmation_change(None);
    });
    list.set_on_delete_confirmation_change(Some(Rc::new(|_| panic!("replaced"))));
    list.set_on_delete_confirmation_change(Some(callback.clone()));
    assert!(Rc::ptr_eq(
        &callback,
        &list.on_delete_confirmation_change().unwrap()
    ));
    (list.on_delete_confirmation_change().unwrap())(None);
    assert_eq!(
        list.on_delete_confirmation_change().is_none(),
        expected.absent
    );
}
/// Checks replacement, callable identity and callback-driven clearing.
fn delete_slot(
    list: &Rc<maestro_chat::SessionList>,
    expected: &SlotResult,
    log: Rc<RefCell<Vec<Value>>>,
    runtime: &manual_runtime::ManualRuntime,
) {
    let weak = Rc::downgrade(list);

    assert_eq!(list.on_delete_session().is_none(), expected.initial_absent);
    let callback: Rc<dyn Fn(&str) -> LocalFuture> = Rc::new(move |value| {
        log.borrow_mut().push(value.into());
        weak.upgrade().unwrap().set_on_delete_session(None);
        Box::pin(async { Ok(()) })
    });
    list.set_on_delete_session(Some(Rc::new(|_| panic!("replaced"))));
    list.set_on_delete_session(Some(callback.clone()));
    assert!(Rc::ptr_eq(&callback, &list.on_delete_session().unwrap()));
    runtime
        .handle()
        .spawn_local(list.on_delete_session().unwrap()("observed"));
    assert!(runtime.drain_futures().is_empty());
    assert_eq!(list.on_delete_session().is_none(), expected.absent);
}
/// Checks replacement, callable identity and callback-driven clearing.
fn notification_slot(
    list: &Rc<maestro_chat::SessionList>,
    expected: &SlotResult,
    log: Rc<RefCell<Vec<Value>>>,
    error: bool,
) {
    let weak = Rc::downgrade(list);

    let (get, set): NotifyAccess = if error {
        (
            maestro_chat::SessionList::on_error,
            maestro_chat::SessionList::set_on_error,
        )
    } else {
        (
            maestro_chat::SessionList::on_rename_session,
            maestro_chat::SessionList::set_on_rename_session,
        )
    };
    assert_eq!(get(list).is_none(), expected.initial_absent);
    let callback: Rc<dyn Fn(&str)> = Rc::new(move |value| {
        log.borrow_mut().push(value.into());
        set(&weak.upgrade().unwrap(), None);
    });
    set(list, Some(Rc::new(|_| panic!("replaced"))));
    set(list, Some(callback.clone()));
    assert!(Rc::ptr_eq(&callback, &get(list).unwrap()));
    get(list).unwrap()("observed");
    assert_eq!(get(list).is_none(), expected.absent);
}
record!(VisibleScope { scope: String });
record!(VisibleLoad { blocked:Frame,opened:Frame,renames:Vec<Vec<String>> });
fn visible_loading(name: &str) {
    replay_cases(name, |i: VisibleScope, e: VisibleLoad| {
        let port = Rc::new(Effects::default());
        let log = Rc::new(RefCell::new(Vec::new()));
        let h = configured(|o| {
            effects(o, &port);
            o.rename_session = Some(capture(log.clone()));
        });
        ready(&h);
        if i.scope == "all" {
            h.selector.handle_input("\t");
            h.finish("all", records());
        } else {
            assert_eq!(i.scope, "current");
        }
        delete(&h, "/missing/a.jsonl");
        h.selector.handle_input("\x12");
        assert!(
            !h.selector
                .render(120)
                .iter()
                .any(|line| line.contains("Rename Session"))
        );
        assert_eq!(h.frame(120), e.blocked);
        h.finish(&i.scope, records());
        h.selector.handle_input("\x12");
        assert!(
            h.selector
                .render(120)
                .iter()
                .any(|line| line.contains("Rename Session"))
        );
        assert_eq!(h.frame(120), e.opened);
        assert_eq!(*log.borrow(), e.renames);
    });
}

#[cfg(target_os = "linux")]
use super::super::native_child;
/// Qualifies actual command capture and unlink in isolated child environments,
/// re-running `test` (the caller's qualified test name) for each scenario.
#[cfg(target_os = "linux")]
pub fn native(test: &str) {
    if maestro_test_process::selection().is_child() {
        let mode = [
            "unavailable",
            "nonzero",
            "zero",
            "removed-nonzero",
            "leading-dash",
            "unlink-error",
        ]
        .into_iter()
        .find(|mode| maestro_test_process::selection().runs_case(mode))
        .expect("selected native mode");
        native_run(mode);
    } else {
        native_launch(test);
    }
}
/// Drives one child-only native scenario.
#[cfg(target_os = "linux")]
fn native_run(mode: &str) {
    let _guard = registry::REGISTRY.lock().unwrap();
    let _restore = registry::Restore(get_keybindings());
    set_keybindings(manager(Vec::new()));
    let root = std::env::current_dir().unwrap();
    let path = if mode == "leading-dash" {
        "-session.jsonl".into()
    } else {
        root.join("session.jsonl").to_str().unwrap().to_owned()
    };
    let h = configured(|_| {});
    let mut record = session("a");
    record.path = path.clone();
    h.finish("current", vec![record]);
    delete(&h, &path);
    let frame = h.selector.render(2000).join("\n");
    if mode == "unlink-error" {
        native_failure(&h, &path, &frame);
    } else {
        let trash = matches!(mode, "zero" | "removed-nonzero" | "leading-dash");
        assert!(frame.contains(if trash {
            "Session moved to trash"
        } else {
            "Session deleted"
        }));
        assert_eq!(
            std::path::Path::new(&path).exists(),
            matches!(mode, "zero" | "leading-dash")
        );
        assert_eq!(h.current.loads.borrow().len(), 2);
        h.finish("current", vec![]);
        if mode == "leading-dash" {
            assert_eq!(
                std::fs::read_to_string(std::env::var("MAESTRO_TRASH_LOG").unwrap()).unwrap(),
                "--\n-session.jsonl\n"
            );
        }
    }
    println!("\nPROBE native qualified");
}
/// Installs controlled executables through a child before launching each scenario.
#[cfg(target_os = "linux")]
fn native_launch(test: &str) {
    for mode in [
        "unavailable",
        "nonzero",
        "zero",
        "removed-nonzero",
        "leading-dash",
        "unlink-error",
    ] {
        let dir = native_child::Scratch::new();
        std::fs::create_dir(dir.0.path().join("bin")).unwrap();
        let name = if mode == "leading-dash" {
            "-session.jsonl"
        } else {
            "session.jsonl"
        };
        if mode == "unlink-error" {
            std::fs::create_dir(dir.0.path().join(name)).unwrap();
        } else {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dir.0.path().join(name))
                .unwrap();
        }
        if mode != "unavailable" {
            let status = std::process::Command::new("/usr/bin/install")
                .args(["-m", "0700"])
                .arg(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/session-trash.sh"),
                )
                .arg(dir.0.path().join("bin/trash"))
                .status()
                .unwrap();
            assert!(status.success());
        }
        let path = dir.join("bin");
        let log = dir.join("argv");
        let lines = native_child::probe(
            maestro_test_process::TestProcess::named(test)
                .unwrap()
                .case(mode),
            dir.0.path(),
            &[
                ("SESSION_TRASH_OUTCOME", mode),
                ("PATH", &path),
                ("MAESTRO_TRASH_LOG", &log),
            ],
            &[],
        );
        assert_eq!(lines, ["native qualified"]);
    }
}

/// Observes the native directory failure category and composed first-line diagnostic.
#[cfg(target_os = "linux")]
fn native_failure(h: &Harness, path: &str, frame: &str) {
    assert!(frame.contains("Failed to delete:"));
    assert!(frame.contains("(trash: trash native failure)"));
    assert!(!frame.contains("second line"));
    assert_eq!(h.current.loads.borrow().len(), 1);
    assert!(std::path::Path::new(path).is_dir());
    let error = std::fs::remove_file(path).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::IsADirectory);
    assert!(frame.contains(&error.to_string()));
}
