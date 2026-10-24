//! Case observation helper ownership checks.

use std::path::Path;

use ra_ap_rustc_lexer::{FrontmatterAllowed, TokenKind};

use crate::source::{Member, Source};

/// The harness, relative to its crate directory, that owns the observation helpers.
const HARNESS: &str = "tests/support/session_harness.rs";

/// A token of a source file: its text and the byte offset it starts at.
type Token<'a> = (&'a str, usize);

/// A token fragment that recursions call.
const STRING_ARM: &[&str] = &["Value", ":", ":", "String"];
/// A token fragment that collection recursions call.
const ARRAY_ARM: &[&str] = &["Value", ":", ":", "Array"];
/// A token fragment that field recursions call.
const OBJECT_ARM: &[&str] = &["Value", ":", ":", "Object"];
/// A token fragment of any string replacement.
const REPLACEMENT: &[&str] = &[".", "replace", "("];
/// A token fragment that selects extension input events.
const INPUT_EVENT: &[&str] = &["[", "\"type\"", "]", "=", "=", "\"input\""];
/// A token fragment that reads an extension event's source.
const EVENT_SOURCE: &[&str] = &["[", "\"source\"", "]"];
/// A token fragment that reads the prompt and queue corpus.
const CORPUS_CASE: &[&str] = &["corpus", "(", "\"prompt_queue\"", ")"];
/// A token fragment that subscribes to the session's own events.
const SESSION_SUBSCRIPTION: &[&str] = &["session", ".", "subscribe", "("];
/// A token fragment that selects the session's agent events.
const SESSION_AGENT_EVENT: &[&str] = &["AgentSessionEvent", ":", ":", "Agent"];
/// A token fragment that selects the agent's run end.
const RUN_END: &[&str] = &["AgentEvent", ":", ":", "AgentEnd"];

/// An observation helper that only the harness may hold.
struct Helper {
    /// The diagnostic name of the copied helper.
    name: &'static str,
    /// The fragments whose joint presence is one copy of the helper.
    fragments: &'static [&'static [&'static str]],
}

/// The helpers the session harness owns.
const HELPERS: &[Helper] = &[
    Helper {
        name: "JSON substitution walk: use session_harness::substituting",
        fragments: &[STRING_ARM, ARRAY_ARM, OBJECT_ARM, REPLACEMENT],
    },
    Helper {
        name: "input source selection: use session_harness::input_sources",
        fragments: &[INPUT_EVENT, EVENT_SOURCE],
    },
    Helper {
        name: "prompt and queue case lookup: use session_harness::expected",
        fragments: &[CORPUS_CASE],
    },
    Helper {
        name: "session run end observer: use session_harness::session_run_ended",
        fragments: &[SESSION_SUBSCRIPTION, SESSION_AGENT_EVENT, RUN_END],
    },
];

/// Reject a case observation helper copied beside the session harness.
pub(super) fn check(members: &[Member]) -> Result<(), String> {
    for member in members {
        let directory = &member.directory;
        if !member
            .sources
            .iter()
            .any(|source| source.path == directory.join(HARNESS))
        {
            continue;
        }
        for source in &member.sources {
            if !candidate(&source.path, directory) {
                continue;
            }
            if let Some((offset, helper)) = copied(source) {
                return Err(format!(
                    "{}:{}: copied {helper}",
                    source.path.display(),
                    crate::source::line(&source.contents, offset)
                ));
            }
        }
    }
    Ok(())
}

/// Whether `path` holds the crate's own case checks rather than its support code.
fn candidate(path: &Path, directory: &Path) -> bool {
    path.starts_with(directory.join("tests")) && !path.starts_with(directory.join("tests/support"))
}

/// How many tokens after its first fragment the rest of a helper may appear.
const WINDOW: usize = 120;

/// The first observation helper `source` copies, with its first token's offset.
fn copied(source: &Source) -> Option<(usize, &'static str)> {
    let tokens = tokens(&source.contents);
    HELPERS.iter().find_map(|helper| {
        let anchor = *helper.fragments.first()?;
        let mut from = 0;
        while let Some(offset) = index_of(&tokens[from..], anchor) {
            let start = from + offset;
            let end = (start + WINDOW).min(tokens.len());
            if helper
                .fragments
                .iter()
                .skip(1)
                .all(|fragment| index_of(&tokens[start..end], fragment).is_some())
            {
                return Some((tokens[start].1, helper.name));
            }
            from = start + 1;
        }
        None
    })
}

/// The tokens of `contents` without whitespace or comments, in source order.
fn tokens(contents: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut start = ra_ap_rustc_lexer::strip_shebang(contents).unwrap_or(0);
    for token in ra_ap_rustc_lexer::tokenize(&contents[start..], FrontmatterAllowed::No) {
        let end = start + token.len as usize;
        if !matches!(
            token.kind,
            TokenKind::Whitespace | TokenKind::LineComment { .. } | TokenKind::BlockComment { .. }
        ) {
            tokens.push((&contents[start..end], start));
        }
        start = end;
    }
    tokens
}

/// The index of the first `fragment` found in `tokens`.
fn index_of(tokens: &[Token<'_>], fragment: &[&str]) -> Option<usize> {
    tokens.windows(fragment.len()).position(|window| {
        window
            .iter()
            .zip(fragment)
            .all(|((text, _), expected)| text == expected)
    })
}

#[cfg(test)]
mod tests {
    use super::{Source, candidate, copied};
    use std::path::{Path, PathBuf};

    /// A source at `path` holding `contents`.
    fn source(path: &str, contents: &str) -> Source {
        Source {
            path: PathBuf::from(path),
            contents: contents.into(),
            syntax: None,
        }
    }

    /// The name of the helper `contents` copies, if any.
    fn copied_name(contents: &str) -> Option<&'static str> {
        copied(&source("crates/demo/tests/cases.rs", contents)).map(|(_, name)| name)
    }

    /// A recursive JSON string replacement walk.
    const WALK: &str = r"
fn replace(value: Value, from: &str, to: &str) -> Value {
    match value {
        Value::String(text) => json!(text.replace(from, to)),
        Value::Array(items) => items
            .into_iter()
            .map(|item| replace(item, from, to))
            .collect(),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, item)| (key, replace(item, from, to)))
                .collect(),
        ),
        other => other,
    }
}
";

    #[test]
    fn a_copy_in_a_function_is_rejected() {
        assert!(copied_name(WALK).is_some_and(|name| name.contains("substituting")));
    }

    #[test]
    fn a_copy_in_a_macro_argument_is_rejected() {
        let seeded = format!("fn case() {{\n    println!(\"{{:?}}\", {WALK});\n}}\n");
        assert!(copied_name(&seeded).is_some_and(|name| name.contains("substituting")));
    }

    #[test]
    fn a_copy_in_a_closure_is_rejected() {
        let seeded = format!("fn case() {{\n    let walk = |value| {{ {WALK} }};\n}}\n");
        assert!(copied_name(&seeded).is_some_and(|name| name.contains("substituting")));
    }

    #[test]
    fn a_copy_in_an_impl_method_is_rejected() {
        let seeded = format!("impl Case {{\n    {WALK}\n}}\n");
        assert!(copied_name(&seeded).is_some_and(|name| name.contains("substituting")));
    }

    #[test]
    fn a_copy_beside_a_call_to_the_owner_is_rejected() {
        let seeded = format!(
            "fn case(value: Value) {{\n    let _ = substituting(value.clone(), &[]);\n    {WALK}\n}}\n"
        );
        assert!(copied_name(&seeded).is_some_and(|name| name.contains("substituting")));
    }

    #[test]
    fn a_call_to_the_owner_is_accepted() {
        assert_eq!(
            copied_name("fn case(value: Value) -> Value { substituting(value, &[]) }"),
            None
        );
    }

    #[test]
    fn an_input_source_selection_outside_the_harness_is_rejected() {
        let seeded = r#"
fn sources(transcript: &Transcript) -> Vec<Value> {
    transcript
        .extension
        .borrow()
        .iter()
        .filter(|event| event["type"] == "input")
        .map(|event| event["source"].clone())
        .collect()
}
"#;
        assert!(copied_name(seeded).is_some_and(|name| name.contains("input_sources")));
    }

    #[test]
    fn a_case_lookup_outside_the_harness_is_rejected() {
        assert!(
            copied_name(
                "fn expected(name: &str) -> Value { corpus(\"prompt_queue\")[name].clone() }"
            )
            .is_some_and(|name| name.contains("expected"))
        );
    }

    /// A local copy of the harness run-end observer.
    const RUN_END_OBSERVER: &str = r"
fn run_ended(h: &Harness) -> Rc<Notify> {
    let ended = Rc::new(Notify::new());
    let notify = Rc::clone(&ended);
    drop(h.session.subscribe(Rc::new(move |event| {
        if let AgentSessionEvent::Agent(event) = event
            && matches!(*event.read().unwrap(), AgentEvent::AgentEnd { .. })
        {
            notify.notify_one();
        }
    })));
    ended
}
";

    #[test]
    fn a_run_end_observer_outside_the_harness_is_rejected() {
        assert!(
            copied_name(RUN_END_OBSERVER).is_some_and(|name| name.contains("session_run_ended"))
        );
    }

    #[test]
    fn a_run_end_observer_after_another_subscription_is_rejected() {
        let seeded = format!(
            "fn queued(h: &Harness) {{\n    drop(h.session.subscribe(Rc::new(|event| {{\n        let _ = event;\n    }})));\n}}\n{RUN_END_OBSERVER}"
        );
        assert!(copied_name(&seeded).is_some_and(|name| name.contains("session_run_ended")));
    }

    #[test]
    fn an_agent_end_observer_is_accepted() {
        let seeded = r"
fn agent_ended(h: &Harness) -> Rc<Notify> {
    let ended = Rc::new(Notify::new());
    let notify = Rc::clone(&ended);
    drop(h.session.agent().subscribe(Rc::new(move |event, _| {
        if matches!(*event.read().unwrap(), AgentEvent::AgentEnd { .. }) {
            notify.notify_one();
        }
    })));
    ended
}
";
        assert_eq!(copied_name(seeded), None);
    }

    #[test]
    fn a_case_check_without_helpers_is_accepted() {
        assert_eq!(
            copied_name("fn case() { assert_eq!(expected(\"a_case\"), observed()); }"),
            None
        );
    }

    #[test]
    fn the_harness_keeps_the_helpers() {
        let directory = Path::new("crates/demo");
        assert!(candidate(&directory.join("tests/cases.rs"), directory));
        assert!(!candidate(
            &directory.join("tests/support/session_harness.rs"),
            directory
        ));
    }
}
