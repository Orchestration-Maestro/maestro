//! Authored values cross the production host only as retained resources.
#![cfg(test)]
use maestro_extensions::Local;
use serde_json::{Value, json};
use std::rc::Rc;

/// Invoke one compiled-guest observation through the real extension factory.
fn report(behavior: &str) -> Value {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = maestro_extensions::create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let (_owner, runner, _) = super::retained_components::loaded_on(bus.for_extension()).await;
        let command = runner
            .get_command("bridge-observe")
            .unwrap()
            .command
            .handler;
        let observed = command(
            format!("stream {behavior}"),
            runner.create_command_context(),
        )
        .await
        .unwrap_err()
        .to_string();
        let observed =
            serde_json::from_str(&observed).unwrap_or_else(|_| panic!("{behavior}: {observed}"));
        bus.clear();
        observed
    })
}

#[test]
fn generic_callbacks_keep_non_json_local_values() {
    assert_eq!(
        report("generic"),
        json!({
            "first":8, "terminal":-1, "result":"answer:-1:雪", "sameOwner":true,
            "eof":true, "classified":2, "extracted":1,
        })
    );
}
#[test]
fn stream_readers_compete_in_registration_order() {
    assert_eq!(
        report("readers"),
        json!({"queued":[["duplicate",1],["duplicate",2]],"pending":[7,8,null],"eof":true})
    );
}
#[test]
fn terminal_admission_keeps_the_first_result() {
    assert_eq!(
        report("first-terminal"),
        json!({"events":[1,-1],"before":-1,"after":-1,"sameOwner":true,"classified":2,"extracted":1})
    );
}
#[test]
fn ending_without_a_result_keeps_observation_pending() {
    assert_eq!(
        report("optional-results"),
        json!([
            {"value":null,"sameOwner":true}, {"value":false,"sameOwner":true},
            {"value":0,"sameOwner":true}, {"value":"","sameOwner":true},
            {"value":{},"sameOwner":true}, {"value":[],"sameOwner":true},
        ])
    );
}
#[test]
fn generic_callbacks_reenter_outside_callback_storage() {
    assert_eq!(
        report("reentry"),
        json!([
            {"calls":["classify:-1","extract:-1"],"events":[-1],"result":-1},
            {"calls":["classify:-1","classify:2","extract:-1"],"events":[2,-1],"result":-1},
        ])
    );
}
#[test]
fn terminal_push_finishes_other_pending_readers() {
    assert_eq!(report("terminal-readers"), json!([-1, null, null, null]));
}

#[test]
fn assistant_stream_keeps_events_and_terminal_reasons() {
    let observed = report("assistant-events");
    assert_eq!(
        observed["events"],
        json!({
            "updates":[
                {"type":"start"},
                {"type":"text_start","contentIndex":2},
                {"type":"text_delta","contentIndex":2,"delta":"雪\n"},
                {"type":"text_end","contentIndex":2,"content":"finished text"},
                {"type":"thinking_start","contentIndex":3},
                {"type":"thinking_delta","contentIndex":3,"delta":"考え"},
                {"type":"thinking_end","contentIndex":3,"content":"finished thinking"},
                {"type":"toolcall_start","contentIndex":4},
                {"type":"toolcall_delta","contentIndex":4,"delta":"{\"n\":"},
                {"type":"toolcall_end","contentIndex":4,"toolCall":{"type":"toolCall","id":"delta-id","name":"delta-tool","arguments":[false,null,0],"thoughtSignature":"delta-signature"}},
                {"type":"done","reason":"stop"}
            ],
            "liveOwner":true,"resultOwner":true,"eof":true,
        })
    );
    assert_eq!(
        observed["reasons"],
        json!([
            {"kind":"done","reason":"stop","resultOwner":true,"sameOwner":true,"eventOwner":true,"eof":true},
            {"kind":"done","reason":"length","resultOwner":true,"sameOwner":true,"eventOwner":true,"eof":true},
            {"kind":"done","reason":"toolUse","resultOwner":true,"sameOwner":true,"eventOwner":true,"eof":true},
            {"kind":"error","reason":"error","resultOwner":true,"sameOwner":true,"eventOwner":true,"eof":true},
            {"kind":"error","reason":"aborted","resultOwner":true,"sameOwner":true,"eventOwner":true,"eof":true},
        ])
    );
}

/// Literal full-message expectations at the compiled-guest result consumer.
fn terminal_message(reason: &str, diagnostics: bool) -> Value {
    let mut value = json!({
        "role":"assistant", "api":"authored-api", "provider":"authored-provider", "model":"雪-model",
        "responseModel":"response-model", "responseId":"response-id",
        "content":[
            {"type":"text","text":"雪\n\u{0000}\u{001b}","textSignature":"text-signature"},
            {"type":"thinking","thinking":"考え","thinkingSignature":"thinking-signature","redacted":false},
            {"type":"toolCall","id":"call-id","name":"tool","arguments":{"n":-0.0,"roots":[null,false,0,"",{},[]]},"thoughtSignature":"tool-signature"}
        ],
        "usage":{"input":1.5,"output":2.5,"cacheRead":3.5,"cacheWrite":4.5,"totalTokens":null,
            "cost":{"input":0.125,"output":0.25,"cacheRead":0.5,"cacheWrite":1.0,"total":1.875}},
        "diagnostics":[{"type":"controlled","timestamp":6.25,
            "error":{"name":"ControlledError","message":"literal failure 雪","stack":"line\ntrace","code":17.5},
            "details":{"roots":[null,false,0,"",{},[]],"negativeZero":-0.0}}],
        "stopReason":reason,"errorMessage":"literal terminal detail 雪\n", "timestamp":-0.0,
    });
    if !diagnostics {
        for field in ["diagnostics", "errorMessage", "responseModel", "responseId"] {
            value.as_object_mut().unwrap().shift_remove(field);
        }
    }
    value
}

#[test]
fn stream_terminal_results_keep_exact_shared_messages() {
    let observed = report("assistant-results");
    let mut cases = observed.as_array().unwrap().iter();
    for diagnostics in [true, false] {
        for reason in ["stop", "length", "toolUse", "error", "aborted"] {
            let selected = cases.next().unwrap();
            assert_eq!(
                selected,
                &json!({"diagnostics":diagnostics,
                "message":terminal_message(reason, diagnostics),"sameOwner":true})
            );
            assert!(
                selected["message"]["timestamp"]
                    .as_f64()
                    .unwrap()
                    .is_sign_negative()
            );
            assert!(
                selected["message"]["content"][2]["arguments"]["n"]
                    .as_f64()
                    .unwrap()
                    .is_sign_negative()
            );
        }
    }
    assert!(cases.next().is_none());
}

#[test]
fn generic_stream_admission_is_single_pass() {
    let observed = report("admission");
    assert_eq!(
        observed["singlePass"],
        json!([
            {"size":64,"classified":64,"extracted":1,"delivered":64,"result":64},
            {"size":128,"classified":128,"extracted":1,"delivered":128,"result":128},
            {"size":256,"classified":256,"extracted":1,"delivered":256,"result":256},
        ])
    );
    assert_eq!(
        observed["rescanned"],
        json!([
            {"size":16,"classified":136,"extracted":1,"delivered":136,"result":16},
            {"size":32,"classified":528,"extracted":1,"delivered":528,"result":32},
            {"size":64,"classified":2080,"extracted":1,"delivered":2080,"result":64},
        ])
    );
    for control in observed["rescanned"].as_array().unwrap() {
        assert!(control["classified"].as_u64().unwrap() > control["size"].as_u64().unwrap());
    }
}
