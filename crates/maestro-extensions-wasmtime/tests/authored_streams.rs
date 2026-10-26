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
