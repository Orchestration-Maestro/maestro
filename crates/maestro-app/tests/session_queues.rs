//! Queuing steering and follow-up messages, refusing registered commands and clearing the
//! queues, compared with the recorded queue cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, Transcript, assert_transcript, case, command_extension, entries_view,
    labelled, on_task_set, outcome, queue_view, recorded_skills, template, transcribed,
    with_placeholders,
};
use maestro_models::ImageContent;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The recorded queue case `name`.
fn expected(name: &str) -> Value {
    case("queues", name)
}

/// Asserts the queue mirrors, the queue updates and the untouched history, transcript
/// and session against `case`.
fn assert_queue_case(h: &Harness, t: &Transcript, case: &Value) {
    assert_eq!(with_placeholders(queue_view(h, t), h), case["queue"]);
    let updates = t.events_of(&["queue_update"]);
    assert_eq!(with_placeholders(json!(updates), h), case["sessionEvents"]);
    assert_eq!(entries_view(h), Vec::<Value>::new());
    let mut transcript = case.clone();
    transcript["contexts"] = json!({"calls": 0});
    assert_transcript(h, t, &transcript);
}

#[test]
fn steer_expands_skills_and_templates() {
    on_task_set(|_| async {
        let expected = expected("steering expands skill commands and templates");
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let (h, t) = transcribed(HarnessOptions {
            skills: recorded_skills(&scratch),
            prompts: vec![template("plan", "Plan $1")],
            scratch: Some(scratch),
            ..HarnessOptions::default()
        })
        .await;
        let first = h.session.steer("/skill:review look", &[]);
        let second = h.session.steer("/plan alpha", &[]);
        assert_eq!(
            json!([outcome(first), outcome(second)]),
            expected["results"]
        );
        assert_queue_case(&h, &t, &expected);
        assert_eq!(h.session.pending_message_count(), 2);
    });
}

#[test]
fn queuing_a_registered_command_is_refused() {
    on_task_set(|_| async {
        let expected = expected("queuing a registered command is refused");
        let heard = Rc::new(RefCell::new(None));
        let (h, t) = transcribed(HarnessOptions {
            factories: vec![command_extension("hello", Rc::clone(&heard), None)],
            ..HarnessOptions::default()
        })
        .await;
        let steer = h.session.steer("/hello now", &[]);
        let follow_up = h.session.follow_up("/hello", &[]);
        assert_eq!(
            json!([outcome(steer), outcome(follow_up)]),
            expected["results"]
        );
        assert_queue_case(&h, &t, &expected);
        assert_eq!(*heard.borrow(), None);
    });
}

#[test]
fn queuing_unknown_slash_text_is_accepted() {
    on_task_set(|_| async {
        let expected = expected("a slash text that names no command still queues");
        let (h, t) = transcribed(HarnessOptions::default()).await;
        let steer = h.session.steer("/notacommand text", &[]);
        let follow_up = h.session.follow_up("/nothing", &[]);
        assert_eq!(
            json!([outcome(steer), outcome(follow_up)]),
            expected["results"]
        );
        assert_queue_case(&h, &t, &expected);
    });
}

#[test]
fn clearing_the_queue_returns_and_publishes_texts() {
    on_task_set(|_| async {
        let expected = expected("clearing the queue returns the texts and publishes");
        let (h, t) = transcribed(HarnessOptions::default()).await;
        let image = ImageContent {
            data: "AAAA".into(),
            mime_type: "image/png".into(),
        };
        let steered = h.session.steer("one", &[]);
        let followed = h.session.follow_up("two", &[image]);
        let before = queue_view(&h, &t);
        let cleared = h.session.clear_queue();
        let cleared = json!({"ok": {"steering": cleared.steering, "followUp": cleared.follow_up}});
        let results = json!([
            outcome(steered),
            outcome(followed),
            labelled("before", before),
            cleared
        ]);
        assert_eq!(results, expected["results"]);
        assert_queue_case(&h, &t, &expected);
        assert!(!h.session.agent().has_queued_messages());
    });
}

#[test]
fn pending_message_count_sums_both_queues() {
    on_task_set(|_| async {
        let (h, _) = transcribed(HarnessOptions::default()).await;
        let counts: Vec<usize> = [("steer", "one"), ("follow", "two"), ("steer", "three")]
            .into_iter()
            .map(|(queue, text)| {
                let queued = if queue == "steer" {
                    h.session.steer(text, &[])
                } else {
                    h.session.follow_up(text, &[])
                };
                queued.unwrap();
                h.session.pending_message_count()
            })
            .collect();
        assert_eq!(counts, [1, 2, 3]);
        assert_eq!(h.session.get_steering_messages(), ["one", "three"]);
        assert_eq!(h.session.get_follow_up_messages(), ["two"]);
        h.session.clear_queue();
        assert_eq!(h.session.pending_message_count(), 0);
    });
}
