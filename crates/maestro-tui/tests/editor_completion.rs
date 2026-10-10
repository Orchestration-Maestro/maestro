#![cfg(test)]
//! Completion requests, menus and key routing through the public editor.
#[allow(dead_code, reason = "Each test crate uses part of the shared rig.")]
#[path = "fixtures/completion_rig.rs"]
mod rig;
#[path = "fixtures/editor_support/host.rs"]
mod support;
use maestro_cancellation::Cancellation;
use maestro_tui::autocomplete::{
    ArgumentCompletions, AutocompleteOperations, Command, DirectoryEntry,
};
use maestro_tui::{
    AutocompleteItem, AutocompleteSuggestions, Component, Editor, EditorOptions, tui::InputHandler,
};
use maestro_tui::{
    CURSOR_MARKER, CombinedAutocompleteProvider, KeybindingKeys, KeybindingsManager, SlashCommand,
    TUI_KEYBINDINGS, set_keybindings, visible_width,
};
use rig::{Rig, offer};
use std::{cell::RefCell, future::Future, io, pin::Pin, rc::Rc};

#[allow(dead_code, reason = "Only the one-step suspension is used here.")]
#[path = "fixtures/futures.rs"]
mod futures;
use futures::YieldOnce;

const TAB: &str = "\t";
const ENTER: &str = "\r";
const ESC: &str = "\x1b";
const UP: &str = "\x1b[A";
const DOWN: &str = "\x1b[B";
const COPY: &str = "\x03";
const UNDO: &str = "\x1b[45;5u";

/// Rows of the menu with trailing padding removed.
fn rows(rig: &Rig) -> Vec<String> {
    rig.menu(80)
        .iter()
        .map(|row| row.trim_end().to_owned())
        .collect()
}

/// The selected row of the menu without its marker.
fn selected(rig: &Rig) -> Option<String> {
    rows(rig)
        .iter()
        .find_map(|row| row.strip_prefix("→ ").map(str::to_owned))
}

/// Opens a menu by typing `text` and answering its request with `reply`.
fn open(rig: &Rig, text: &str, reply: impl rig::IntoReply) {
    rig.typed(text);
    rig.wait(20);
    rig.provider.resolve_last(reply);
    rig.run();
}

/// Types `/` into a fresh slash menu of the given values.
fn slash_menu(rig: &Rig, values: &[&str]) {
    open(rig, "/", offer("/", values));
    assert!(rig.editor.is_showing_autocomplete());
}

#[test]
fn completion_absent_and_empty_results_clear_menu() {
    let rig = Rig::new();
    let (tui, _, runtime) = support::host(24);
    let bare = Editor::new(&tui, support::theme(), EditorOptions::default());
    bare.handle_input("/");
    bare.handle_input(TAB);
    assert_eq!(
        (runtime.futures(), bare.is_showing_autocomplete()),
        (0, false)
    );

    rig.provider.answer(|_| offer("/", &["/one", "/two"]));
    rig.input("/");
    rig.run();
    assert_eq!(rows(&rig), ["→ /one", "  /two"]);

    rig.provider.answer(|_| None);
    rig.runtime.settle().unwrap();
    rig.input("o");
    assert!(
        rig.editor.is_showing_autocomplete(),
        "refresh waits for its provider"
    );
    rig.run();
    assert!(!rig.editor.is_showing_autocomplete());
    assert!(
        rig.runtime.pending() > 0,
        "an emptied menu requests a frame"
    );

    rig.provider.answer(|_| offer("/on", &[]));
    rig.runtime.settle().unwrap();
    rig.input("n");
    rig.run();
    assert!(!rig.editor.is_showing_autocomplete());
    assert!(rig.runtime.pending() > 0, "an empty list requests a frame");
    assert_eq!(rig.editor.get_text(), "/on");
}

#[test]
fn completion_force_predicate_precedes_cancellation() {
    let rig = Rig::new();
    rig.typed("@");
    rig.wait(20);
    assert_eq!(rig.provider.count(), 1, "automatic @ request is running");
    rig.provider.trigger.set(Some(false));
    rig.input(TAB);
    rig.run();
    assert_eq!(rig.provider.trigger_asks.get(), 1);
    assert!(
        !rig.provider.aborted(0),
        "a vetoed force request cancels nothing"
    );
    assert_eq!(rig.provider.count(), 1);
    drop(rig);

    for opinion in [None, Some(true)] {
        let rig = Rig::new();
        rig.provider.trigger.set(opinion);
        rig.typed("x");
        rig.input(TAB);
        rig.run();
        assert_eq!(rig.provider.count(), 1, "{opinion:?} admits the request");
    }

    let rig = Rig::new();
    rig.typed("x");
    rig.input(TAB);
    rig.run();
    rig.provider.resolve_last(offer("x", &["xa", "xb"]));
    rig.run();
    assert!(rig.editor.is_showing_autocomplete());
    rig.provider.trigger.set(Some(false));
    rig.input("y");
    rig.run();
    assert_eq!(rig.provider.count(), 1);
    assert!(
        rig.editor.is_showing_autocomplete(),
        "a vetoed refresh keeps its menu"
    );
}

#[test]
fn completion_autoapply_requires_explicit_force_single() {
    let rig = Rig::new();
    rig.typed("x");
    rig.input(TAB);
    rig.run();
    rig.provider.resolve_last(offer("x", &["xyz"]));
    rig.run();
    assert_eq!(rig.editor.get_text(), "xyz");
    assert!(!rig.editor.is_showing_autocomplete());
    assert_eq!(*rig.changes.borrow(), ["x", "xyz"], "one committed change");
    rig.input(UNDO);
    assert_eq!(rig.editor.get_text(), "x", "one undo snapshot");
    drop(rig);

    let rig = Rig::new();
    slash_menu(&rig, &["/help"]);
    assert_eq!(
        rig.editor.get_text(),
        "/",
        "a regular single item uses the menu"
    );
    assert_eq!(rows(&rig), ["→ /help"]);
    drop(rig);

    let rig = Rig::new();
    rig.typed("x");
    rig.input(TAB);
    rig.run();
    rig.provider.resolve_last(offer("x", &["xa", "xb"]));
    rig.run();
    assert_eq!(rows(&rig), ["→ xa", "  xb"]);
    assert_eq!(rig.editor.get_text(), "x");
}

/// Records submissions of the editor.
fn submissions(rig: &Rig) -> Rc<RefCell<Vec<String>>> {
    let log = Rc::new(RefCell::new(Vec::new()));
    let sink = Rc::clone(&log);
    rig.editor.set_on_submit(Some(Rc::new(move |text| {
        sink.borrow_mut().push(text.to_owned());
    })));
    log
}

/// Copy and undo keep priority over the open menu.
fn assert_copy_and_undo_precede_menu() {
    let rig = Rig::new();
    slash_menu(&rig, &["/a", "/b", "/c"]);
    rig.input(COPY);
    assert!(
        rig.editor.is_showing_autocomplete(),
        "copy precedes the menu"
    );
    rig.input(UNDO);
    assert_eq!(rig.editor.get_text(), "", "undo precedes the menu");
    assert!(
        rig.editor.is_showing_autocomplete(),
        "undo does not cancel completion"
    );
}

/// Up and down wrap through the list and cancel consumes its key.
fn assert_navigation_and_cancel() {
    let rig = Rig::new();
    let submitted = submissions(&rig);
    slash_menu(&rig, &["/a", "/b", "/c"]);
    rig.input(UP);
    assert_eq!(
        selected(&rig).as_deref(),
        Some("/c"),
        "up wraps to the last row"
    );
    assert_eq!(
        rig.editor.get_cursor().col,
        1,
        "the editor never moves its own cursor for menu navigation"
    );
    rig.input(DOWN);
    assert_eq!(
        selected(&rig).as_deref(),
        Some("/a"),
        "down wraps to the first row"
    );
    rig.input(ESC);
    assert!(!rig.editor.is_showing_autocomplete());
    assert_eq!(
        (rig.editor.get_text().as_str(), submitted.borrow().len()),
        ("/", 0)
    );
}

/// Tab and a non-slash confirm apply as edits; a slash confirm continues to submit.
fn assert_apply_keys() {
    let rig = Rig::new();
    let submitted = submissions(&rig);
    open(&rig, "@", offer("@", &["@one", "@two"]));
    rig.changes.borrow_mut().clear();
    rig.input(TAB);
    assert_eq!(rig.editor.get_text(), "@one");
    assert_eq!(*rig.changes.borrow(), ["@one"]);
    assert!(submitted.borrow().is_empty() && !rig.editor.is_showing_autocomplete());

    rig.editor.set_text("");
    open(&rig, "@", offer("@", &["@one", "@two"]));
    rig.input(DOWN);
    rig.changes.borrow_mut().clear();
    rig.input(ENTER);
    assert_eq!(rig.editor.get_text(), "@two");
    assert_eq!(*rig.changes.borrow(), ["@two"]);
    assert!(submitted.borrow().is_empty(), "non-slash confirm returns");

    rig.editor.set_text("");
    slash_menu(&rig, &["/alpha"]);
    rig.changes.borrow_mut().clear();
    rig.input(ENTER);
    assert_eq!(
        *submitted.borrow(),
        ["/alpha"],
        "slash confirm falls through to submit"
    );
    assert_eq!(
        *rig.changes.borrow(),
        [""],
        "no intermediate change after the slash apply"
    );
}

#[test]
fn completion_menu_key_precedence() {
    assert_copy_and_undo_precede_menu();
    assert_navigation_and_cancel();
    assert_apply_keys();
}

#[test]
fn completion_confirm_respects_submit_gate() {
    let rig = Rig::new();
    let submitted = submissions(&rig);
    rig.editor.set_disable_submit(true);
    slash_menu(&rig, &["/alpha"]);
    rig.input(ENTER);
    assert_eq!(
        rig.editor.get_text(),
        "/alpha",
        "the choice survives a disabled submit"
    );
    assert!(submitted.borrow().is_empty() && !rig.editor.is_showing_autocomplete());

    rig.editor.set_text("");
    open(&rig, "@", offer("@", &["@one"]));
    rig.input(ENTER);
    assert_eq!(
        rig.editor.get_text(),
        "@one",
        "a non-slash choice stays an edit"
    );

    rig.editor.set_text("");
    let rebound = vec![(
        "tui.select.confirm".to_owned(),
        Some(maestro_tui::KeybindingKeys::Single("alt+enter".to_owned())),
    )];
    maestro_tui::set_keybindings(maestro_tui::KeybindingsManager::new(
        maestro_tui::TUI_KEYBINDINGS.clone(),
        rebound,
    ));
    slash_menu(&rig, &["/alpha"]);
    rig.input("\x1b\r");
    assert_eq!(
        rig.editor.get_text(),
        "/alpha\n",
        "a slash confirm that is not submit continues as ordinary input"
    );
    assert!(submitted.borrow().is_empty());
}

/// Items with distinct labels for matching tests.
fn items(pairs: &[(&str, &str)], prefix: &str) -> AutocompleteSuggestions {
    AutocompleteSuggestions {
        items: pairs
            .iter()
            .map(|(value, label)| AutocompleteItem {
                value: (*value).to_owned(),
                label: (*label).to_owned(),
                description: None,
            })
            .collect(),
        prefix: prefix.to_owned(),
    }
}

/// Candidates as `(value, label)` pairs, the typed prefix and the value Enter applies.
struct Matching {
    /// What the case shows.
    name: &'static str,
    /// Candidates in provider order.
    pairs: &'static [(&'static str, &'static str)],
    /// Prefix the provider reports.
    prefix: &'static str,
    /// Value Enter applies.
    applied: &'static str,
}

/// Value-matching cases in decreasing order of precedence.
const MATCHING: [Matching; 7] = [
    Matching {
        name: "empty prefix keeps the default",
        pairs: &[("x", "x"), ("y", "y")],
        prefix: "",
        applied: "x",
    },
    Matching {
        name: "exact beats an earlier prefix",
        pairs: &[("bz", "bz"), ("b", "b")],
        prefix: "b",
        applied: "b",
    },
    Matching {
        name: "first prefix wins",
        pairs: &[("xb", "xb"), ("bz", "bz"), ("ba", "ba")],
        prefix: "b",
        applied: "bz",
    },
    Matching {
        name: "case matters",
        pairs: &[("b", "b"), ("Ba", "Ba")],
        prefix: "B",
        applied: "Ba",
    },
    Matching {
        name: "labels never match",
        pairs: &[("q", "b"), ("r", "r")],
        prefix: "b",
        applied: "q",
    },
    Matching {
        name: "duplicates keep their order",
        pairs: &[("d", "first"), ("d", "second")],
        prefix: "d",
        applied: "d",
    },
    Matching {
        name: "no match keeps the default",
        pairs: &[("m", "m"), ("n", "n")],
        prefix: "z",
        applied: "m",
    },
];

/// Opens a forced menu of `pairs` after `b`.
fn forced_menu(rig: &Rig, suggestions: AutocompleteSuggestions) {
    rig.typed("b");
    rig.input(TAB);
    rig.run();
    rig.provider.resolve_last(suggestions);
    rig.run();
}

#[test]
fn completion_value_matching_retains_order() {
    for case in MATCHING {
        let rig = Rig::new();
        forced_menu(&rig, items(case.pairs, case.prefix));
        assert!(selected(&rig).is_some(), "{}", case.name);
        rig.input(ENTER);
        assert_eq!(
            rig.provider.applied.borrow().as_slice(),
            [case.applied],
            "{}",
            case.name
        );
    }
    let rig = Rig::new();
    forced_menu(
        &rig,
        items(&[("d", "first"), ("d", "second"), ("e", "e")], "b"),
    );
    assert_eq!(
        rows(&rig),
        ["→ first", "  second", "  e"],
        "candidates are not sorted or merged"
    );
}

/// Requests a fresh editor makes after `setup` text and then each input, answered with nothing.
fn requests(setup: &str, inputs: &[&str]) -> usize {
    let rig = Rig::new();
    rig.provider.answer(|_| None);
    if !setup.is_empty() {
        rig.editor.set_text(setup);
    }
    for input in inputs {
        rig.input(input);
        rig.wait(20);
    }
    rig.provider.count()
}

#[test]
fn completion_contexts_have_distinct_boundaries() {
    let cases: [(&str, &[&str], usize); 21] = [
        ("", &["/"], 1),
        (" ", &["/"], 1),
        ("a", &["/"], 0),
        ("a\n", &["/"], 0),
        ("\u{feff}", &["/"], 1),
        ("\u{85}", &["/"], 0),
        ("", &["/", "a"], 2),
        ("", &["/", "a", " ", "b"], 3),
        ("", &["a", "b"], 0),
        ("", &["@"], 1),
        ("a", &["@"], 0),
        ("a ", &["@"], 1),
        ("a\t", &["@"], 1),
        ("é", &["@"], 0),
        ("\u{feff}", &["@"], 0),
        ("", &["@", "é"], 1),
        ("", &["#", "x"], 2),
        ("", &["x@y"], 0),
        ("", &["a @y"], 1),
        ("", &["@\"ab\"c"], 1),
        ("", &["@", "\"", "a", "b", "\"", "c"], 4),
    ];
    for (setup, inputs, expected) in cases {
        assert_eq!(
            requests(setup, inputs),
            expected,
            "{setup:?} then {inputs:?}"
        );
    }
}

/// A menu opened by `@` that refreshes on every later edit.
fn symbol_menu() -> Rig {
    let rig = Rig::new();
    rig.provider.answer(|_| offer("@", &["@a", "@b"]));
    rig.typed("@");
    rig.wait(20);
    assert_eq!(rig.provider.count(), 1);
    rig
}

/// A quoted `@` keeps waiting through whitespace; an unquoted token stops at it.
fn assert_quoted_and_plain_tokens() {
    let rig = symbol_menu();
    for typed in ["\"", "x", " ", "y"] {
        rig.typed(typed);
        rig.run();
        assert_eq!(
            rig.provider.count(),
            1,
            "quoted @ keeps waiting after {typed:?}"
        );
    }
    rig.wait(20);
    assert_eq!(
        rig.provider.count(),
        2,
        "one restarted request after the quiet period"
    );
    drop(rig);

    let rig = symbol_menu();
    rig.typed("x");
    rig.typed(" ");
    rig.run();
    assert_eq!(
        rig.provider.count(),
        2,
        "a plain token stops waiting once whitespace ends it"
    );
}

#[test]
fn completion_deadline_restarts_at_twenty_milliseconds() {
    let rig = Rig::new();
    rig.typed("@");
    rig.run();
    assert_eq!(rig.provider.count(), 0, "nothing starts before the wait");
    rig.wait(19);
    assert_eq!(rig.provider.count(), 0, "19 ms is too early");
    rig.wait(1);
    assert_eq!(rig.provider.count(), 1, "the request starts at 20 ms");
    drop(rig);

    let rig = Rig::new();
    rig.typed("@");
    rig.wait(15);
    rig.typed("a");
    rig.wait(15);
    assert_eq!(rig.provider.count(), 0, "another edit restarts the wait");
    rig.wait(5);
    assert_eq!(rig.provider.count(), 1);
    drop(rig);

    let rig = Rig::new();
    rig.typed("x");
    rig.input(TAB);
    rig.run();
    assert_eq!(rig.provider.count(), 1, "explicit Tab starts at once");
    drop(rig);
    assert_quoted_and_plain_tokens();
}

/// Starts a held request for `/` and returns the rig.
fn pending() -> Rig {
    let rig = Rig::new();
    rig.typed("/");
    rig.run();
    assert_eq!(rig.provider.count(), 1);
    rig
}

/// An edit applied to a rig with one running request.
type Edit = fn(&Rig);

/// Edits that abort the running request.
const CANCELLING: [(&str, Edit); 6] = [
    ("set text", |rig| rig.editor.set_text("/")),
    ("insertion", |rig| rig.editor.insert_text_at_cursor("a")),
    ("paste", |rig| rig.input("\x1b[200~ab\x1b[201~")),
    ("newline", |rig| rig.input("\x1b[13;2~")),
    ("submit", |rig| rig.input(ENTER)),
    ("provider", |rig| {
        rig.editor.set_autocomplete_provider(rig::Scripted::new());
    }),
];

/// Edits that leave the running request alone.
const KEEPING: [(&str, Edit); 4] = [
    ("empty insertion", |rig| {
        rig.editor.insert_text_at_cursor("");
    }),
    ("empty paste", |rig| rig.input("\x1b[200~\x1b[201~")),
    ("undo", |rig| rig.input(UNDO)),
    ("cursor movement", |rig| rig.input("\x1b[D")),
];

/// A menu edit refreshes in the mode the menu was requested in.
fn assert_refresh_mode(force: bool) {
    let rig = Rig::new();
    if force {
        rig.typed("x");
        rig.input(TAB);
        rig.run();
        rig.provider.resolve_last(offer("x", &["xa", "xb"]));
    } else {
        rig.provider.answer(|_| offer("/", &["/a", "/b"]));
        rig.typed("/");
    }
    rig.run();
    assert!(rig.editor.is_showing_autocomplete());
    rig.typed("y");
    rig.run();
    let last = rig.provider.calls.borrow().last().map(|call| call.force);
    assert_eq!(last, Some(Some(force)), "force {force}");
}

#[test]
fn completion_edits_cancel_refresh_or_keep_pending() {
    for (name, edit) in CANCELLING {
        let rig = pending();
        edit(&rig);
        assert!(rig.provider.aborted(0), "{name} cancels");
    }
    for (name, edit) in KEEPING {
        let rig = pending();
        edit(&rig);
        assert!(!rig.provider.aborted(0), "{name} keeps the request");
    }
    let rig = Rig::new();
    rig.editor.set_text("ab");
    rig.input(TAB);
    rig.run();
    rig.input("\x7f");
    assert!(
        !rig.provider.aborted(0),
        "a deletion outside any context keeps the request"
    );
    drop(rig);
    assert_refresh_mode(false);
    assert_refresh_mode(true);
}

#[test]
fn completion_serial_queue_skips_superseded_starts() {
    let rig = pending();
    rig.typed("a");
    rig.run();
    assert!(rig.provider.aborted(0), "abort is signalled at once");
    assert_eq!(
        rig.provider.count(),
        1,
        "the replacement waits for its predecessor"
    );
    rig.typed("b");
    rig.typed("c");
    rig.run();
    assert_eq!(rig.provider.count(), 1);

    let replacement = rig::Scripted::new();
    rig.editor.set_autocomplete_provider(replacement.clone());
    rig.typed("d");
    rig.provider.resolve(0, offer("/", &["/stale"]));
    rig.run();
    assert!(
        !rig.editor.is_showing_autocomplete(),
        "a cancelled result is never applied"
    );
    assert_eq!(rig.provider.count(), 1);
    assert_eq!(replacement.count(), 1, "one start, using the live provider");
    assert_eq!(
        replacement.calls.borrow()[0].lines,
        ["/abcd"],
        "the live buffer at start"
    );
}

#[test]
fn completion_snapshot_can_return_to_same_position() {
    let rig = pending();
    rig.input("\x1b[D");
    rig.input("\x1b[C");
    rig.provider.resolve(0, offer("/", &["/a", "/b"]));
    rig.run();
    assert!(
        rig.editor.is_showing_autocomplete(),
        "the same text and cursor stay eligible"
    );
    drop(rig);

    let rig = Rig::new();
    rig.editor.set_text("ab");
    rig.input(TAB);
    rig.run();
    rig.typed("c");
    rig.input(UNDO);
    assert_eq!(rig.editor.get_text(), "ab");
    rig.provider.resolve(0, offer("ab", &["abc", "abd"]));
    rig.run();
    assert!(
        rig.editor.is_showing_autocomplete(),
        "undo back to the request's text is eligible"
    );
    drop(rig);

    let rig = Rig::new();
    rig.editor.set_text("ab");
    rig.input(TAB);
    rig.run();
    rig.typed("c");
    rig.provider.resolve(0, offer("ab", &["abc", "abd"]));
    rig.run();
    assert!(
        !rig.editor.is_showing_autocomplete(),
        "changed text rejects the result"
    );
}

#[test]
fn completion_tab_context_uses_literal_space() {
    let cases: [(&str, bool); 7] = [
        ("/he", false),
        ("  /he", false),
        ("\u{feff}/he", false),
        ("/he llo", true),
        ("x\n/he", true),
        ("hello", true),
        ("/a\tb", false),
    ];
    for (text, forced) in cases {
        let rig = Rig::new();
        if text.contains('\t') {
            rig.input(text);
        } else {
            rig.editor.set_text(text);
        }
        rig.input(TAB);
        rig.run();
        let last = rig.provider.calls.borrow().last().map(|call| call.force);
        assert_eq!(last, Some(Some(forced)), "{text:?}");
    }
}

#[test]
fn completion_cancel_removes_delayed_start() {
    type Cancel = fn(&Rig);
    let cancels: [(&str, Cancel); 4] = [
        ("provider", |rig| {
            rig.editor.set_autocomplete_provider(rig::Scripted::new());
        }),
        ("text", |rig| rig.editor.set_text("")),
        ("newline", |rig| rig.input("\x1b[13;2~")),
        ("insertion", |rig| rig.editor.insert_text_at_cursor("z")),
    ];
    for (name, cancel) in cancels {
        let rig = Rig::new();
        rig.typed("@");
        cancel(&rig);
        rig.wait(100);
        assert_eq!(
            rig.provider.count(),
            0,
            "{name} removes the scheduled start"
        );
        assert_eq!(rig.runtime.futures(), 0);
    }
    let rig = Rig::new();
    open(&rig, "@", offer("@", &["@a", "@b"]));
    assert!(rig.editor.is_showing_autocomplete());
    let replacement = rig::Scripted::new();
    rig.editor.set_autocomplete_provider(replacement.clone());
    assert!(
        !rig.editor.is_showing_autocomplete(),
        "the menu clears before later work"
    );
    rig.typed("c");
    rig.wait(20);
    assert_eq!((rig.provider.count(), replacement.count()), (1, 1));
}

/// Requests made by one deletion sequence on a provider that offers nothing.
fn deletion_requests(text: &str, inputs: &[&str]) -> usize {
    let rig = Rig::new();
    rig.provider.answer(|_| None);
    rig.editor.set_text(text);
    for input in inputs {
        rig.input(input);
    }
    rig.wait(20);
    rig.provider.count()
}

/// Backspace on a forced menu keeps its mode, and on a slash menu emptied by it hides the menu.
fn assert_menu_deletions() {
    let rig = Rig::new();
    rig.provider.answer(|_| offer("/", &["/a", "/b"]));
    rig.input("/");
    rig.run();
    assert!(rig.editor.is_showing_autocomplete());
    rig.provider.answer(|_| None);
    rig.input("\x7f");
    rig.run();
    assert_eq!(rig.editor.get_text(), "");
    assert!(
        !rig.editor.is_showing_autocomplete(),
        "backspacing a slash to empty hides the menu"
    );
    drop(rig);

    let rig = Rig::new();
    rig.typed("x");
    rig.input(TAB);
    rig.run();
    rig.provider.resolve_last(offer("x", &["xa", "xb"]));
    rig.run();
    rig.input("\x7f");
    rig.run();
    let last = rig.provider.calls.borrow().last().map(|call| call.force);
    assert_eq!(last, Some(Some(true)), "deletion keeps the forced mode");
}

#[test]
fn completion_deletions_retrigger_from_their_context() {
    assert_eq!(
        deletion_requests("/ab", &["\x7f"]),
        1,
        "backspace retriggers a slash context"
    );
    assert_eq!(
        deletion_requests("/ab", &["\x1b[D", "\x1b[3~"]),
        1,
        "forward delete retriggers its own context"
    );
    assert_eq!(
        deletion_requests("", &["\x7f", "\x1b[3~"]),
        0,
        "a no-op deletion requests nothing"
    );
    assert_eq!(
        deletion_requests("@a b", &["\x1b[D", "\x7f"]),
        1,
        "deleting the space returns to the token context"
    );
    assert_menu_deletions();
}

#[test]
fn completion_cursor_line_staleness_is_checked() {
    for (name, key, accepted) in [
        ("unchanged", "", true),
        ("line", UP, false),
        ("column", "\x1b[D", false),
    ] {
        let rig = Rig::new();
        rig.editor.set_text("ab\ncd");
        rig.input(TAB);
        rig.run();
        assert_eq!(rig.provider.count(), 1);
        if !key.is_empty() {
            rig.input(key);
        }
        rig.provider.resolve(0, offer("cd", &["cda", "cdb"]));
        rig.run();
        assert_eq!(rig.editor.is_showing_autocomplete(), accepted, "{name}");
    }
}

#[test]
fn completion_rejection_settles_before_next_request() {
    let rig = Rig::new();
    slash_menu(&rig, &["/a", "/b"]);
    rig.typed("a");
    rig.run();
    rig.typed("b");
    rig.provider.resolve(1, rig::failure());
    let errors = rig.run();
    assert_eq!(errors.len(), 1, "the provider error reaches the host");
    assert_eq!(errors[0].to_string(), "controlled failure");
    assert!(
        errors[0]
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.kind() == std::io::ErrorKind::Other),
        "the error is returned unchanged"
    );
    assert!(rig.editor.is_showing_autocomplete(), "the old menu stays");
    assert_eq!(
        rig.provider.count(),
        3,
        "the queued request started after the failure"
    );
    rig.provider.resolve(2, offer("/ab", &["/abc", "/abd"]));
    rig.run();
    assert_eq!(rows(&rig), ["→ /abc", "  /abd"]);
    rig.typed("c");
    rig.run();
    assert_eq!(rig.provider.count(), 4, "a later request works");
    drop(rig);

    let rig = Rig::new();
    rig.typed("/");
    rig.run();
    let replacement = rig::Scripted::new();
    rig.editor.set_autocomplete_provider(replacement.clone());
    rig.editor.set_text("");
    rig.typed("/");
    rig.run();
    assert_eq!(replacement.count(), 0, "waits for the failing predecessor");
    rig.provider.resolve(0, rig::failure());
    assert_eq!(rig.run().len(), 1);
    assert_eq!(
        replacement.count(),
        1,
        "the replacement provider starts after a failure"
    );
    replacement.resolve(0, offer("/", &["/x", "/y"]));
    rig.run();
    assert!(rig.editor.is_showing_autocomplete());
}

/// Five described slash candidates.
fn described() -> AutocompleteSuggestions {
    let item = |value: &str| AutocompleteItem {
        value: value.to_owned(),
        label: value.to_owned(),
        description: Some(format!("about {value}")),
    };
    AutocompleteSuggestions {
        items: ["/aa", "/bb", "/cc", "/dd", "/ee"].map(item).to_vec(),
        prefix: "/".to_owned(),
    }
}

/// A row padded with spaces to the full width.
fn padded(text: &str, width: usize) -> String {
    format!("{text:<width$}")
}

/// Rendering stays within the width, hides only the hardware cursor marker and edits nothing.
fn assert_render_bounds(rig: &Rig) {
    for width in [0, 1, 8, 20, 60, 80] {
        let lines = rig.editor.render(width);
        assert!(
            lines.iter().all(|line| visible_width(line) <= width),
            "width {width}"
        );
        let joined = lines.join("\n");
        assert!(!joined.contains(CURSOR_MARKER), "width {width}");
        assert_eq!(joined.contains("\x1b[7m"), width > 0, "width {width}");
    }
    assert_eq!(
        rig.editor.get_cursor().col,
        1,
        "rendering never edits the stored cursor"
    );
}

#[test]
fn completion_render_uses_list_layout_and_suppresses_cursor() {
    let rig = Rig::new();
    rig.editor.set_autocomplete_max_visible(3.0);
    rig.provider.answer(|_| Some(described()));
    let closed = rig.editor.render(20).join("\n");
    assert!(
        closed.contains(CURSOR_MARKER),
        "a focused editor emits the cursor marker"
    );
    rig.input("/");
    rig.run();
    assert_eq!(
        rig.menu(60),
        [
            padded("→ /aa         about /aa", 60),
            padded("  /bb         about /bb", 60),
            padded("  /cc         about /cc", 60),
            padded("  (1/5)", 60),
        ],
        "slash columns are 12 to 32 wide and a long list reports its position"
    );
    assert_eq!(
        rig.menu(20),
        [
            padded("→ /aa", 20),
            padded("  /bb", 20),
            padded("  /cc", 20),
            padded("  (1/5)", 20)
        ]
    );
    assert_eq!(rig.menu(0), ["", "", "", ""], "zero cells hold nothing");
    assert_eq!(
        rig.menu(1),
        ["→\x1b[0m", " \x1b[0m", " \x1b[0m", " "],
        "one cell holds the marker only"
    );
    assert_render_bounds(&rig);
    rig.input(DOWN);
    assert_eq!(rig.menu(60)[1], padded("→ /bb         about /bb", 60));
    rig.editor.set_padding_x(2.0);
    assert_eq!(
        rig.menu(30),
        [
            padded("    /aa", 30),
            padded("  → /bb", 30),
            padded("    /cc", 30),
            padded("    (2/5)", 30)
        ],
        "menu rows share the editor's side padding"
    );
    rig.input(ESC);
    rig.provider.answer(|_| offer("@", &["@one", "@two"]));
    rig.editor.set_text("");
    open_plain(&rig);
    assert_eq!(
        rig.menu(30),
        [padded("  → @one", 30), padded("    @two", 30)],
        "default non-slash layout"
    );
}

/// Types `@` and lets its request finish.
fn open_plain(rig: &Rig) {
    rig.typed("@");
    rig.wait(20);
}

/// Candidates `{argument}1` and `{argument}2`.
fn numbered(argument: &str) -> Vec<AutocompleteItem> {
    vec![
        rig::item(&format!("{argument}1")),
        rig::item(&format!("{argument}2")),
    ]
}

/// Host operations that never touch the file system.
struct NoFiles;

impl AutocompleteOperations for NoFiles {
    type Signal = Cancellation;

    fn is_aborted(&self, signal: &Cancellation) -> bool {
        signal.is_aborted()
    }

    fn run_fd<'a>(
        &'a self,
        _: &'a str,
        _: &'a [String],
        _: &'a Cancellation,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<u8>>> + 'a>> {
        Box::pin(async { Err(io::Error::other("no search")) })
    }

    fn home_dir(&self) -> io::Result<String> {
        Err(io::Error::other("no home"))
    }

    fn read_dir(&self, _: &str) -> io::Result<Vec<DirectoryEntry>> {
        Err(io::Error::other("no directory"))
    }

    fn is_directory(&self, _: &str) -> io::Result<bool> {
        Err(io::Error::other("no metadata"))
    }

    fn compare(&self, left: &str, right: &str) -> io::Result<std::cmp::Ordering> {
        Ok(left.cmp(right))
    }
}

/// A slash command whose argument callback records its argument and answers `numbered`.
fn command(name: &str, suspend: bool, seen: &Rc<RefCell<Vec<String>>>) -> Command {
    let seen = Rc::clone(seen);
    let completions: ArgumentCompletions = Rc::new(move |argument| {
        seen.borrow_mut().push(argument.to_owned());
        Box::pin(async move {
            if suspend {
                YieldOnce(false).await;
            }
            Ok(Some(numbered(argument)))
        })
    });
    Command::SlashCommand(SlashCommand {
        name: name.to_owned(),
        description: None,
        argument_hint: None,
        get_argument_completions: Some(completions),
    })
}

/// An editor whose provider is the bundled one over three commands.
fn bundled_rig(seen: &Rc<RefCell<Vec<String>>>) -> Rig {
    let rig = Rig::new();
    let plain = Command::SlashCommand(SlashCommand {
        name: "plain".to_owned(),
        description: None,
        argument_hint: None,
        get_argument_completions: None,
    });
    let commands = vec![
        command("model", false, seen),
        command("later", true, seen),
        plain,
    ];
    rig.editor
        .set_autocomplete_provider(Rc::new(CombinedAutocompleteProvider::new(
            commands,
            "/work".to_owned(),
            None,
            NoFiles,
        )));
    rig
}

/// A suspended argument callback leaves its request pending; later edits supersede its result.
fn assert_suspended_argument(rig: &Rig, seen: &Rc<RefCell<Vec<String>>>) {
    rig.editor.set_text("");
    seen.borrow_mut().clear();
    rig.typed("/later x");
    rig.run();
    assert!(
        !rig.editor.is_showing_autocomplete(),
        "the callback is still suspended"
    );
    assert_eq!(rig.runtime.futures(), 1);
    rig.typed("y");
    rig.run();
    assert!(
        !rig.editor.is_showing_autocomplete(),
        "the superseded result is not applied"
    );
    rig.run();
    assert_eq!(*seen.borrow(), ["x", "xy"]);
    assert_eq!(rows(rig), ["→ xy1", "  xy2"]);
}

#[test]
fn completion_bundled_provider_argument_paths() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let rig = bundled_rig(&seen);
    rig.typed("/model é");
    rig.run();
    assert_eq!(
        *seen.borrow(),
        ["é"],
        "the callback receives the argument at its byte cursor"
    );
    assert_eq!(rows(&rig), ["→ é1", "  é2"]);
    rig.input(ENTER);
    assert_eq!(rig.editor.get_text(), "/model é1");
    assert_eq!(rig.editor.get_cursor().col, "/model é1".len());
    assert_suspended_argument(&rig, &seen);
    rig.editor.set_text("");
    rig.typed("/plain x");
    rig.run();
    assert!(
        !rig.editor.is_showing_autocomplete(),
        "a command without a callback offers nothing"
    );
}

#[test]
fn completion_force_mode_keeps_single_item_menu() {
    let rig = Rig::new();
    rig.typed("x");
    rig.input(TAB);
    rig.run();
    rig.provider.resolve_last(offer("x", &["xa", "xb"]));
    rig.run();
    assert_eq!(rows(&rig).len(), 2);
    rig.typed("y");
    rig.run();
    rig.provider.resolve_last(offer("xy", &["xya"]));
    rig.run();
    assert_eq!(rows(&rig), ["→ xya"], "a forced menu keeps its single item");
    assert_eq!(rig.editor.get_text(), "xy", "nothing was applied silently");
    rig.typed("z");
    rig.run();
    assert_eq!(
        rig.provider.calls.borrow().last().map(|call| call.force),
        Some(Some(true))
    );
    rig.provider.resolve_last(offer("xyz", &["xyza"]));
    rig.run();
    rig.input(ENTER);
    assert_eq!(rig.editor.get_text(), "xyza");
}

#[test]
fn completion_undo_restores_before_applied_value() {
    let rig = Rig::new();
    rig.typed("x");
    rig.input(TAB);
    rig.run();
    rig.provider.resolve_last(offer("x", &["xa", "xb"]));
    rig.run();
    rig.input(ENTER);
    assert_eq!(rig.editor.get_text(), "xa");
    rig.typed("q");
    rig.input(UNDO);
    assert_eq!(
        rig.editor.get_text(),
        "xa",
        "later typing is its own undo group"
    );
    rig.input(UNDO);
    assert_eq!(rig.editor.get_text(), "x");
    assert_eq!(rig.editor.get_cursor().col, 1);
    drop(rig);

    let rig = Rig::new();
    rig.typed("x");
    rig.input(TAB);
    rig.run();
    rig.provider.resolve_last(offer("x", &["xyz"]));
    rig.run();
    assert_eq!(rig.editor.get_text(), "xyz");
    rig.input(UNDO);
    assert_eq!(
        (rig.editor.get_text().as_str(), rig.editor.get_cursor().col),
        ("x", 1)
    );
}

#[test]
fn completion_paste_does_not_request_suggestions() {
    let rig = Rig::new();
    rig.typed("/");
    rig.run();
    assert_eq!(rig.provider.count(), 1);
    rig.input("\x1b[200~look at @node_modules/react/index.js please\x1b[201~");
    assert!(
        rig.provider.aborted(0),
        "a paste cancels the admitted request"
    );
    rig.wait(100);
    assert_eq!(rig.provider.count(), 1, "the paste starts no request");
    assert_eq!(
        rig.editor.get_text(),
        "/look at @node_modules/react/index.js please"
    );
    assert!(!rig.editor.is_showing_autocomplete());
    rig.provider.resolve(0, offer("/", &["/a", "/b"]));
    rig.run();
    assert!(
        !rig.editor.is_showing_autocomplete(),
        "the cancelled result is dropped"
    );
    assert_eq!(rig.runtime.futures(), 0);
}

/// Replaces the defaults of the completion keys with Ctrl letters.
fn rebind_completion_keys() {
    let bind = |action: &str, key: &str| {
        (
            action.to_owned(),
            Some(KeybindingKeys::Single(key.to_owned())),
        )
    };
    set_keybindings(KeybindingsManager::new(
        TUI_KEYBINDINGS.clone(),
        vec![
            bind("tui.input.tab", "ctrl+t"),
            bind("tui.select.up", "ctrl+p"),
            bind("tui.select.down", "ctrl+n"),
            bind("tui.select.confirm", "ctrl+g"),
            bind("tui.select.cancel", "ctrl+x"),
        ],
    ));
}

/// The new keys drive the menu and the former defaults no longer do.
fn assert_new_keys_drive_menu() {
    let rig = Rig::new();
    rebind_completion_keys();
    rig.editor.set_text("x");
    rig.input("\x14");
    rig.run();
    assert_eq!(
        rig.provider.count(),
        1,
        "the new Tab key requests completion"
    );
    rig.provider.resolve_last(offer("x", &["xa", "xb", "xc"]));
    rig.run();
    rig.input("\x0e");
    assert_eq!(
        selected(&rig).as_deref(),
        Some("xb"),
        "the new down key moves"
    );
    rig.input("\x10");
    assert_eq!(
        selected(&rig).as_deref(),
        Some("xa"),
        "the new up key moves"
    );
    rig.input(DOWN);
    rig.input(UP);
    assert_eq!(
        selected(&rig).as_deref(),
        Some("xa"),
        "former arrows no longer drive the menu"
    );
    rig.input(COPY);
    assert!(
        rig.editor.is_showing_autocomplete(),
        "copy keeps priority over an overlapping cancel"
    );
    rig.input(ESC);
    assert!(
        rig.editor.is_showing_autocomplete(),
        "the former cancel key does nothing"
    );
    rig.input("\x18");
    assert!(
        !rig.editor.is_showing_autocomplete(),
        "the new cancel key cancels"
    );
    rig.editor.set_text("x");
    rig.input("\x14");
    rig.run();
    rig.provider.resolve_last(offer("x", &["xa", "xb"]));
    rig.run();
    rig.input("\x07");
    assert_eq!(rig.editor.get_text(), "xa", "the new confirm key applies");
}

/// The former Tab and confirm keys keep only their ordinary meaning.
fn assert_former_keys_are_ordinary() {
    let rig = Rig::new();
    rebind_completion_keys();
    let submitted = submissions(&rig);
    rig.editor.set_text("x");
    rig.input(TAB);
    rig.run();
    assert_eq!(
        rig.provider.count(),
        0,
        "the former Tab key requests nothing"
    );
    rig.input("\x14");
    rig.run();
    rig.provider.resolve_last(offer("x", &["xa", "xb"]));
    rig.run();
    rig.input(TAB);
    assert!(
        rig.editor.is_showing_autocomplete() && rig.editor.get_text() == "x",
        "former Tab does not apply"
    );
    rig.input(ENTER);
    assert_eq!(
        *submitted.borrow(),
        ["x"],
        "the former confirm key now only submits"
    );
}

#[test]
fn completion_registered_keys_replace_defaults() {
    assert_new_keys_drive_menu();
    assert_former_keys_are_ordinary();
}
