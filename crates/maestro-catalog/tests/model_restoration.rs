#![cfg(test)]
//! Saved model restoration and fallback notices.
#[path = "support/descriptors.rs"]
mod descriptors;
#[path = "support/isolation.rs"]
mod isolation;
#[path = "selection/scopes.rs"]
mod scopes;
use maestro_catalog::{
    ModelResolverOutput, RestoreModelFromSessionOptions, restore_model_from_session,
};
use scopes::replay;
use std::sync::Arc;

#[test]
fn restore_authenticated_model_wins_over_current() {
    replay(&["case_103", "case_104"]);
}

#[test]
fn restore_auth_command_is_presence_only() {
    replay(&["case_105"]);
}

#[test]
fn restore_failure_reason_distinguishes_missing_and_auth() {
    replay(&["case_106", "case_107"]);
}

#[test]
fn restore_current_model_needs_no_auth_or_membership() {
    replay(&["case_108", "case_109"]);
}

#[test]
fn restore_fallback_uses_live_default_order() {
    if isolation::isolated("restore_fallback_uses_live_default_order") {
        return;
    }
    replay(&["case_110", "case_111", "case_112"]);
}

#[test]
fn restore_first_available_preserves_order() {
    replay(&["case_113"]);
}

#[test]
fn restore_without_fallback_has_no_message() {
    replay(&["case_114", "case_115"]);
}

#[test]
fn restore_message_gate_and_styles_preserve_streams() {
    replay(&[
        "case_116", "case_117", "case_118", "case_119", "case_120", "case_121", "case_122",
    ]);
}

#[test]
fn restore_messages_keep_authored_identity() {
    replay(&["case_123"]);
}

#[test]
fn restore_diagnostics_are_best_effort() {
    let (saved, current) = (scopes::model("p", "saved"), scopes::model("c", "current"));
    let registry = scopes::registry(std::slice::from_ref(&saved), &["p".into()]);
    let run = |name: &str,
               current: Option<&scopes::Handle>,
               print: bool,
               out: &mut ModelResolverOutput<'_>| {
        restore_model_from_session(
            RestoreModelFromSessionOptions {
                saved_provider: "p",
                saved_model_id: name,
                current_model: current,
                should_print_messages: print,
                model_registry: &registry,
            },
            out,
        )
    };
    let (mut stdout, mut stderr) = (scopes::Broken::default(), scopes::Broken::default());
    let mut output = ModelResolverOutput {
        stdout: &mut stdout,
        stderr: &mut stderr,
        color: false,
    };
    let restored = run("saved", None, true, &mut output);
    assert!(Arc::ptr_eq(restored.model.as_ref().unwrap(), &saved));
    assert_eq!(restored.fallback_message, None);
    let held = run("missing", Some(&current), true, &mut output);
    assert!(Arc::ptr_eq(held.model.as_ref().unwrap(), &current));
    assert_eq!(
        held.fallback_message.as_deref(),
        Some("Could not restore model p/missing (model no longer exists). Using c/current.")
    );
    let first = run("missing", None, true, &mut output);
    assert!(Arc::ptr_eq(first.model.as_ref().unwrap(), &saved));
    assert_eq!(
        first.fallback_message.as_deref(),
        Some("Could not restore model p/missing (model no longer exists). Using p/saved.")
    );
    assert_eq!((stdout.0, stderr.0 > 0), (3, true));
    let (mut quiet_out, mut quiet_err) = (scopes::Broken::default(), scopes::Broken::default());
    let mut quiet = ModelResolverOutput {
        stdout: &mut quiet_out,
        stderr: &mut quiet_err,
        color: true,
    };
    for name in ["saved", "missing"] {
        run(name, Some(&current), false, &mut quiet);
    }
    assert_eq!((quiet_out.0, quiet_err.0), (0, 0));
}
