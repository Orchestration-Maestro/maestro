#![cfg(test)]
//! Callback reentry, weak ownership, the shared component interface and local futures.
#[allow(dead_code, reason = "Each test crate uses part of the shared rig.")]
#[path = "fixtures/completion_rig.rs"]
mod rig;
#[path = "fixtures/editor_support/host.rs"]
mod support;
use maestro_cancellation::Cancellation;
use maestro_tui::EditorComponent;
use maestro_tui::editor_component::TextCallback;
use maestro_tui::tui::TuiRuntime;
use rig::{Rig, offer};
use std::{
    cell::{Cell, RefCell},
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
};

const TAB: &str = "\t";
const ENTER: &str = "\r";

/// Wires an applying hook and a change callback that both reenter the editor weakly.
fn reenter_on_apply_and_change(rig: &Rig) -> Rc<RefCell<Vec<(String, String)>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let weak = Rc::downgrade(&rig.editor);
    let (log, reentrant, nested) = (Rc::clone(&seen), weak.clone(), Cell::new(false));
    rig.editor.set_on_change(Some(Rc::new(move |text| {
        let Some(editor) = reentrant.upgrade() else {
            return;
        };
        log.borrow_mut().push((text.to_owned(), editor.get_text()));
        if text == "applied" && !nested.replace(true) {
            editor.set_text("final");
        }
    })));
    let hook_target = weak;
    rig.provider.on_apply.replace(Some(Box::new(move || {
        if let Some(editor) = hook_target.upgrade() {
            editor.set_text("hooked");
        }
    })));
    seen
}

/// Each way an application is reached, ending with a candidate that replaces the prefix.
fn apply_by(path: &str, rig: &Rig) {
    rig.editor.set_text("");
    rig.typed(if path == "confirm" { "@" } else { "x" });
    match path {
        "tab-menu" | "forced-single" => rig.input(TAB),
        _ => {}
    }
    rig.wait(20);
    let (prefix, values): (&str, &[&str]) = match path {
        "forced-single" => ("x", &["applied"]),
        "tab-menu" => ("x", &["applied", "other"]),
        _ => ("@", &["applied", "other"]),
    };
    rig.provider.resolve_last(offer(prefix, values));
    rig.run();
    match path {
        "tab-menu" => rig.input(TAB),
        "confirm" => rig.input(ENTER),
        _ => {}
    }
}

#[test]
fn completion_callbacks_observe_commit_and_allow_reentry() {
    for path in ["forced-single", "tab-menu", "confirm"] {
        let rig = Rig::new();
        let seen = reenter_on_apply_and_change(&rig);
        apply_by(path, &rig);
        let seen = seen.take();
        assert!(
            seen.iter().all(|(argument, live)| argument == live),
            "{path}: callbacks observe the committed value"
        );
        let texts: Vec<_> = seen.iter().map(|(text, _)| text.as_str()).collect();
        assert!(
            texts.ends_with(&["hooked", "applied", "final"]),
            "{path}: {texts:?}"
        );
        assert_eq!(
            rig.editor.get_text(),
            "final",
            "{path}: the callback may change the text again"
        );
        assert_eq!(rig.provider.applied.borrow().len(), 1, "{path}");

        let witness = Rc::downgrade(&rig.editor);
        let rig::Rig {
            editor, provider, ..
        } = rig;
        editor.set_on_change(None);
        provider.on_apply.replace(None);
        drop(editor);
        assert!(
            witness.upgrade().is_none(),
            "{path}: nothing keeps the editor alive"
        );
    }
}

#[test]
fn completion_pending_request_releases_editor() {
    let rig = Rig::new();
    let held_by_editor = Rc::new(());
    let state_witness = Rc::downgrade(&held_by_editor);
    rig.editor.set_on_change(Some(Rc::new(move |_| {
        drop(Rc::clone(&held_by_editor));
    })));
    rig.typed("/");
    rig.run();
    assert_eq!(
        rig.provider.slot_owners(0),
        2,
        "the provider and the pending future"
    );
    let alias = Rc::clone(&rig.editor);
    let rig::Rig {
        editor,
        provider,
        runtime,
        ..
    } = rig;
    let witness = Rc::downgrade(&editor);
    drop(editor);
    assert!(
        witness.upgrade().is_some(),
        "a retained alias keeps its state"
    );
    assert_eq!(alias.get_text(), "/");
    assert!(state_witness.upgrade().is_some());
    drop(alias);
    assert!(
        witness.upgrade().is_none() && state_witness.upgrade().is_none(),
        "a pending provider owns neither the editor nor its retained state"
    );

    runtime.settle().unwrap();
    provider.resolve(0, offer("/", &["/a", "/b"]));
    assert!(runtime.drain_futures().is_empty());
    assert_eq!(provider.slot_owners(0), 1, "the settled future is released");
    assert_eq!(runtime.futures(), 0);
    assert_eq!(
        runtime.pending(),
        0,
        "no frame is requested for a dropped editor"
    );
}

/// Exercises the change and submit callback accessors, `set_text` and Enter through the trait.
/// Callbacks are shared slots: an earlier alias stays callable after replacement.
fn exercise_callbacks<E: EditorComponent<Signal = Cancellation>>(editor: &E) {
    let log = Rc::new(RefCell::new(Vec::new()));
    let recorder = |name: &'static str| -> TextCallback {
        let log = Rc::clone(&log);
        Rc::new(move |text| log.borrow_mut().push(format!("{name}:{text}")))
    };
    assert!(editor.on_change().is_none() && editor.on_submit().is_none());
    editor.set_on_change(Some(recorder("first")));
    let alias = editor.on_change().unwrap();
    editor.set_text("hello");
    editor.set_on_change(Some(recorder("second")));
    editor.set_text("again");
    alias("direct");
    editor.set_on_change(None);
    assert!(editor.on_change().is_none());
    editor.set_on_submit(Some(recorder("submit")));
    editor.set_text("go");
    editor.handle_input(ENTER);
    assert_eq!(
        *log.borrow(),
        ["first:hello", "second:again", "first:direct", "submit:go"],
        "an earlier alias stays callable after replacement"
    );
}

/// Border, history, insertion and marker expansion operate on the retained editor.
fn exercise_text<E: EditorComponent<Signal = Cancellation>>(editor: &E) {
    assert_eq!(
        editor.set_border_color(Rc::new(|text| format!("[{text}]"))),
        Some(())
    );
    assert!(
        editor
            .border_color()
            .is_some_and(|paint| paint("-") == "[-]")
    );
    assert!(editor.render(6)[0].starts_with("[─]"));

    assert_eq!(editor.add_to_history("older"), Some(()));
    editor.handle_input("\x1b[A");
    assert_eq!(editor.get_text(), "older");
    editor.set_text("a");
    assert_eq!(editor.insert_text_at_cursor("!"), Some(()));
    assert_eq!(editor.get_text(), "a!");

    let big = "x\n".repeat(12);
    editor.set_text("");
    editor.handle_input(&format!("\x1b[200~{big}\x1b[201~"));
    assert_eq!(editor.get_text(), "[paste #1 +13 lines]");
    assert_eq!(editor.get_expanded_text().as_deref(), Some(big.as_str()));
}

/// Padding, provider installation and the item maximum take effect.
fn exercise_completion<E: EditorComponent<Signal = Cancellation>>(
    editor: &E,
    provider: &Rc<rig::Scripted>,
    run: &dyn Fn(),
) {
    editor.set_text("ab");
    assert_eq!(editor.set_padding_x(2.0), Some(()));
    assert!(editor.render(10)[1].starts_with("  ab"));

    let five = ["/a", "/b", "/c", "/d", "/e"];
    provider.answer(move |_| offer("/", &five));
    assert_eq!(editor.set_autocomplete_provider(provider.clone()), Some(()));
    assert_eq!(editor.set_autocomplete_max_visible(3.0), Some(()));
    editor.set_text("");
    editor.handle_input("/");
    run();
    assert_eq!(
        editor.render(20).len(),
        3 + 3 + 1,
        "three items and a position row"
    );
}

#[test]
fn editor_component_facade_shares_retained_operations() {
    let rig = Rig::new();
    rig.editor.set_on_change(None);
    let runtime = rig.runtime.clone();
    exercise_callbacks(&*rig.editor);
    exercise_text(&*rig.editor);
    exercise_completion(&*rig.editor, &rig.provider, &|| {
        runtime.drain_futures();
    });
}

/// Becomes ready once its flag is set.
struct Gate(Rc<Cell<bool>>, Rc<Cell<u32>>, Rc<()>);

impl Future for Gate {
    type Output = Result<(), Box<dyn std::error::Error>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        self.1.set(self.1.get() + 1);
        if self.0.get() {
            Poll::Ready(Err(Box::new(rig::Marked(Rc::clone(&self.2)))))
        } else {
            Poll::Pending
        }
    }
}

#[test]
fn completion_host_defers_and_drives_local_futures() {
    let runtime = support::manual_runtime::ManualRuntime::new();
    let open = Rc::new(Cell::new(false));
    let polls = Rc::new(Cell::new(0));
    let witness = Rc::new(());
    let owned = Rc::clone(&witness);
    let token = Rc::new(());
    let (gate_open, gate_polls, gate_token) =
        (Rc::clone(&open), Rc::clone(&polls), Rc::clone(&token));
    runtime.spawn_local(Box::pin(async move {
        let result = Gate(gate_open, gate_polls, gate_token).await;
        drop(owned);
        result
    }));
    assert_eq!(polls.get(), 0, "spawning never polls");
    assert!(runtime.drain_futures().is_empty());
    assert_eq!(
        (polls.get(), runtime.futures()),
        (1, 1),
        "the future suspends"
    );
    open.set(true);
    let errors = runtime.drain_futures();
    assert_eq!(errors.len(), 1);
    assert!(
        errors[0]
            .downcast_ref::<rig::Marked>()
            .is_some_and(|error| Rc::ptr_eq(&error.0, &token)),
        "the very error the future returned reaches the host"
    );
    assert_eq!(runtime.futures(), 0);
    assert_eq!(
        Rc::strong_count(&witness),
        1,
        "a finished future is dropped"
    );
}
