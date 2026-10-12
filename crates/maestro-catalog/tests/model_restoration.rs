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

/// Stdout that renames a model as soon as the first notice is written.
struct Renamer {
    model: scopes::Handle,
    text: Vec<u8>,
}

impl std::io::Write for Renamer {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.model.write().unwrap().id = "renamed".into();
        self.text.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn restore_message_reads_fallback_identity_after_the_notice() {
    let (saved, current) = (scopes::model("p", "saved"), scopes::model("c", "current"));
    let registry = scopes::registry(std::slice::from_ref(&saved), &["p".into()]);
    for (use_current, expected) in [(true, "c/renamed"), (false, "p/renamed")] {
        let target = if use_current { &current } else { &saved };
        let mut stdout = Renamer {
            model: Arc::clone(target),
            text: Vec::new(),
        };
        let mut stderr = Vec::new();
        let restored = restore_model_from_session(
            RestoreModelFromSessionOptions {
                saved_provider: "p",
                saved_model_id: "missing",
                current_model: use_current.then_some(&current),
                should_print_messages: true,
                model_registry: &registry,
            },
            &mut ModelResolverOutput {
                stdout: &mut stdout,
                stderr: &mut stderr,
                color: false,
            },
        );
        let noted = if use_current { "c/current" } else { "p/saved" };
        assert_eq!(
            String::from_utf8(stdout.text).unwrap(),
            format!("Falling back to: {noted}\n")
        );
        assert!(Arc::ptr_eq(restored.model.as_ref().unwrap(), target));
        assert_eq!(
            restored.fallback_message.unwrap(),
            format!(
                "Could not restore model p/missing (model no longer exists). Using {expected}."
            )
        );
        target.write().unwrap().id = if use_current { "current" } else { "saved" }.into();
    }
}
