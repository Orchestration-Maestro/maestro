//! Custom and user messages: next-turn, queued, triggered and appended custom messages,
//! and user messages that always start a turn, compared with the recorded message cases.
#![cfg(not(target_arch = "wasm32"))]
use crate::support::session_harness::{
    Harness, HarnessOptions, Transcript, answer, assert_transcript, case, command_extension,
    entries_view, hold_first_answer, labelled, of_kinds, on_task_set, outcome, queue_view,
    recorded_skills, script_responses, start_prompt, template, transcribed, with_placeholders,
};
use maestro_app::agent_session::PromptOptions;
use maestro_models::{ImageContent, TextContent, UserBlock, UserContent};
use maestro_session::Presence;
use maestro_session::messages::{
    CustomMessageInput, MessageDelivery, SendMessageOptions, SendUserMessageOptions,
    UserMessageDelivery,
};
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The recorded message case `name`.
fn expected(name: &str) -> Value {
    case("messages", name)
}

/// A custom `note` message with `text`.
fn note(text: &str, display: bool, details: Presence<Value>) -> CustomMessageInput {
    CustomMessageInput {
        custom_type: "note".into(),
        content: UserContent::Text(text.into()),
        display,
        details,
    }
}

/// Custom-message options delivering as `deliver_as`, triggering a turn when `trigger`.
fn delivery(deliver_as: Option<MessageDelivery>, trigger: Option<bool>) -> SendMessageOptions {
    SendMessageOptions {
        trigger_turn: trigger,
        deliver_as,
    }
}

/// User-message options delivering as `deliver_as`.
fn user_delivery(deliver_as: Option<UserMessageDelivery>) -> SendUserMessageOptions {
    SendUserMessageOptions { deliver_as }
}

/// Asserts the transcript, the final queue, the queue updates and the stored entries
/// against `case`.
fn assert_message_case(h: &Harness, t: &Transcript, case: &Value) {
    assert_transcript(h, t, case);
    let updates = t.events_of(&["queue_update"]);
    let recorded = of_kinds(case["sessionEvents"].as_array().unwrap(), &["queue_update"]);
    assert_eq!(updates, recorded);
    assert_eq!(queue_view(h, t), case["queue"]);
    assert_eq!(
        with_placeholders(json!(entries_view(h)), h),
        case["entries"]
    );
}

/// Sends `message` with `options` while the first answer streams, then lets the turn end;
/// returns the outcome and the queue view while streaming.
async fn streaming_custom_case(
    name: &str,
    message: CustomMessageInput,
    options: SendMessageOptions,
) -> (Harness, Transcript, Value, Value) {
    let (h, t) = transcribed(HarnessOptions::default()).await;
    let (requested, release) = hold_first_answer(&h, "first", &["second"]);
    let running = start_prompt(&h, "first");
    requested.notified().await;
    let sent = h.session.send_custom_message(message, options).await;
    let streaming = queue_view(&h, &t);
    release.notify_one();
    running.await.unwrap().unwrap();
    let expected = expected(name);
    assert_message_case(&h, &t, &expected);
    (h, t, outcome(sent), streaming)
}

#[test]
fn streaming_custom_message_steers_by_default() {
    on_task_set(|_| async {
        let name = "a streaming custom message steers by default";
        let message = note("steered", true, Presence::Missing);
        let (_, _, sent, streaming) =
            streaming_custom_case(name, message, delivery(None, None)).await;
        let results = json!([sent, labelled("queueWhileStreaming", streaming)]);
        assert_eq!(results, expected(name)["results"]);
    });
}

#[test]
fn streaming_custom_message_follows_up_when_selected() {
    on_task_set(|_| async {
        let name = "a streaming custom message follows up when selected";
        let message = note("later", false, Presence::Missing);
        let options = delivery(Some(MessageDelivery::FollowUp), None);
        let (_, _, sent, _) = streaming_custom_case(name, message, options).await;
        assert_eq!(json!([sent]), expected(name)["results"]);
    });
}

#[test]
fn next_turn_custom_message_waits_for_the_next_prompt() {
    on_task_set(|_| async {
        let expected = expected("a next-turn custom message waits even while streaming");
        let (h, t) = transcribed(HarnessOptions::default()).await;
        let (requested, release) = hold_first_answer(&h, "first", &["second"]);
        let running = start_prompt(&h, "first");
        requested.notified().await;
        let message = note("waits", true, Presence::Missing);
        let options = delivery(Some(MessageDelivery::NextTurn), None);
        let sent = h.session.send_custom_message(message, options).await;
        let streaming = queue_view(&h, &t);
        release.notify_one();
        running.await.unwrap().unwrap();
        let next = h.session.prompt("second", PromptOptions::default()).await;
        let results = json!([
            outcome(sent),
            labelled("queueWhileStreaming", streaming),
            outcome(next)
        ]);
        assert_eq!(results, expected["results"]);
        assert_message_case(&h, &t, &expected);
    });
}

#[test]
fn triggered_custom_message_starts_a_turn() {
    on_task_set(|_| async {
        let expected = expected("a custom message with a triggered turn prompts the agent");
        let (h, t) = transcribed(HarnessOptions::default()).await;
        script_responses(&h, vec![answer("reply", None, None)]);
        let message = note("start", true, Presence::Missing);
        let sent = h
            .session
            .send_custom_message(message, delivery(None, Some(true)))
            .await;
        assert_eq!(json!([outcome(sent)]), expected["results"]);
        assert_message_case(&h, &t, &expected);
    });
}

#[test]
fn direct_custom_message_appends_and_announces_itself() {
    on_task_set(|_| async {
        let expected = expected("a custom message without a trigger appends and announces itself");
        let (h, t) = transcribed(HarnessOptions::default()).await;
        let message = note("appended", false, Presence::Present(json!({"k": 2})));
        let sent = h
            .session
            .send_custom_message(message, delivery(None, None))
            .await;
        assert_eq!(json!([outcome(sent)]), expected["results"]);
        assert_message_case(&h, &t, &expected);
        assert_eq!(json!(entries_view(&h)), expected["entries"]);
    });
}

/// Sends `content` as a user message to an idle session and asserts `name`.
async fn idle_user_message_case(name: &str, content: UserContent) {
    let expected = expected(name);
    let (h, t) = transcribed(HarnessOptions::default()).await;
    script_responses(&h, vec![answer("reply", None, None)]);
    let sent = h
        .session
        .send_user_message(content, user_delivery(None))
        .await;
    assert_eq!(json!([outcome(sent)]), expected["results"]);
    assert_message_case(&h, &t, &expected);
}

#[test]
fn user_message_always_starts_a_turn() {
    on_task_set(|_| async {
        let content = UserContent::Text("hello".into());
        idle_user_message_case("a user message always triggers a turn", content).await;
    });
}

#[test]
fn user_message_puts_images_after_the_joined_text() {
    on_task_set(|_| async {
        let content = UserContent::Blocks(vec![
            UserBlock::Text(TextContent::new("line one".into())),
            UserBlock::Image(ImageContent {
                data: "AAAA".into(),
                mime_type: "image/png".into(),
            }),
            UserBlock::Text(TextContent::new("line two".into())),
        ]);
        idle_user_message_case("a user message splits text and images", content).await;
    });
}

#[test]
fn extension_user_message_skips_expansion_and_reports_its_source() {
    on_task_set(|_| async {
        let expected =
            expected("a user message from an extension skips expansion and reports its source");
        let scratch = maestro_test_directory::Scratch::new().unwrap();
        let heard = Rc::new(RefCell::new(None));
        let (h, t) = transcribed(HarnessOptions {
            skills: recorded_skills(&scratch),
            prompts: vec![template("plan", "Plan $1")],
            scratch: Some(scratch),
            factories: vec![command_extension("hello", Rc::clone(&heard), None)],
            ..HarnessOptions::default()
        })
        .await;
        let replies = ["reply", "reply", "reply"].map(|text| answer(text, None, None));
        script_responses(&h, replies.into());
        let mut results = Vec::new();
        for text in ["/plan alpha", "/skill:review look", "/hello now"] {
            let content = UserContent::Text(text.into());
            let sent = h
                .session
                .send_user_message(content, user_delivery(None))
                .await;
            results.push(outcome(sent));
        }
        results.push(labelled("commandArgs", &*heard.borrow()));
        assert_eq!(json!(results), expected["results"]);
        assert_message_case(&h, &t, &expected);
    });
}

#[test]
fn queued_extension_slash_text_runs_no_command() {
    on_task_set(|_| async {
        let expected = expected(
            "an extension slash text is queued while streaming without running its command",
        );
        let heard = Rc::new(RefCell::new(None));
        let (h, t) = transcribed(HarnessOptions {
            factories: vec![command_extension("hello", Rc::clone(&heard), None)],
            ..HarnessOptions::default()
        })
        .await;
        let (requested, release) = hold_first_answer(&h, "first", &["second"]);
        let running = start_prompt(&h, "first");
        requested.notified().await;
        let content = UserContent::Text("/hello now".into());
        let options = user_delivery(Some(UserMessageDelivery::Steer));
        let queued = h.session.send_user_message(content, options).await;
        release.notify_one();
        running.await.unwrap().unwrap();
        let results = json!([outcome(queued), labelled("commandArgs", &*heard.borrow())]);
        assert_eq!(results, expected["results"]);
        assert_message_case(&h, &t, &expected);
    });
}

#[test]
fn user_message_during_a_response_needs_a_queue_choice() {
    on_task_set(|_| async {
        let expected = expected("a user message while streaming without a behaviour is refused");
        let (h, t) = transcribed(HarnessOptions::default()).await;
        let (requested, release) = hold_first_answer(&h, "first", &["first"]);
        let running = start_prompt(&h, "first");
        requested.notified().await;
        let refused = h
            .session
            .send_user_message(UserContent::Text("queued".into()), user_delivery(None))
            .await;
        let options = user_delivery(Some(UserMessageDelivery::Steer));
        let steered = h
            .session
            .send_user_message(UserContent::Text("steered".into()), options)
            .await;
        release.notify_one();
        running.await.unwrap().unwrap();
        assert_eq!(
            json!([outcome(refused), outcome(steered)]),
            expected["results"]
        );
        assert_message_case(&h, &t, &expected);
    });
}
