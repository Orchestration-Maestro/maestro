//! Session events in the form the recorded cases assert.
use super::with_field;
use crate::agent_session::CompactionReason;
use crate::{AgentSession, AgentSessionEvent};
use maestro_models::Local;
use maestro_session::CompactionResult;
use serde_json::{Value, json};
use std::{cell::RefCell, rc::Rc};

/// The session events of `session` the scripts assert, in their recorded form.
pub(super) fn recorded_session_events(session: &AgentSession<Local>) -> Rc<RefCell<Vec<Value>>> {
    let events = Rc::new(RefCell::new(Vec::new()));
    let recorded = Rc::clone(&events);
    let _unsubscribe = session.subscribe(Rc::new(move |event| {
        if let Some(view) = session_event_view(event) {
            recorded.borrow_mut().push(view);
        }
    }));
    events
}

/// The recorded form of a session event the scripts assert.
#[must_use]
pub fn session_event_view(event: &AgentSessionEvent) -> Option<Value> {
    match event {
        AgentSessionEvent::ThinkingLevelChanged { level } => {
            Some(json!({"type": "thinking_level_changed", "level": level}))
        }
        AgentSessionEvent::SessionInfoChanged { name: Some(name) } => {
            Some(json!({"type": "session_info_changed", "name": name}))
        }
        AgentSessionEvent::SessionInfoChanged { name: None } => {
            Some(json!({"type": "session_info_changed"}))
        }
        AgentSessionEvent::CompactionStart { reason } => {
            Some(json!({"type": "compaction_start", "reason": reason_view(*reason)}))
        }
        AgentSessionEvent::AutoRetryStart {
            attempt,
            max_attempts,
            delay_ms,
            error_message,
        } => Some(json!({
            "type": "auto_retry_start",
            "attempt": attempt,
            "maxAttempts": recorded_number(*max_attempts),
            "delayMs": recorded_number(*delay_ms),
            "errorMessage": error_message,
        })),
        AgentSessionEvent::CompactionEnd { reason, result, .. } => {
            Some(compaction_end_view(event, *reason, result.as_ref()))
        }
        AgentSessionEvent::AutoRetryEnd {
            success,
            attempt,
            final_error,
        } => {
            let view = json!({
                "type": "auto_retry_end",
                "attempt": attempt,
                "success": success,
            });
            Some(match final_error {
                Some(error) => with_field(view, "finalError", json!(error)),
                None => view,
            })
        }
        _ => None,
    }
}

/// The recorded form of a compaction end.
fn compaction_end_view(
    event: &AgentSessionEvent,
    reason: CompactionReason,
    result: Option<&CompactionResult>,
) -> Value {
    let AgentSessionEvent::CompactionEnd {
        aborted,
        will_retry,
        error_message,
        ..
    } = event
    else {
        return Value::Null;
    };
    let view = json!({
        "type": "compaction_end",
        "reason": reason_view(reason),
        "aborted": aborted,
        "willRetry": will_retry,
        "result": result.map(compaction_result_view),
    });
    match error_message {
        Some(message) => with_field(view, "errorMessage", json!(message)),
        None => view,
    }
}

/// The recorded form of a stored compaction result.
fn compaction_result_view(result: &CompactionResult) -> Value {
    let view = json!({
        "summary": result.summary,
        "firstKeptEntryId": result.first_kept_entry_id,
        "tokensBefore": recorded_number(result.tokens_before),
    });
    if result.details.is_missing() {
        return view;
    }
    with_field(view, "details", json!(result.details))
}

/// What started a compaction, as the recorded cases spell it.
fn reason_view(reason: CompactionReason) -> &'static str {
    match reason {
        CompactionReason::Manual => "manual",
        CompactionReason::Threshold => "threshold",
        CompactionReason::Overflow => "overflow",
    }
}

/// A number as a script writes it: integral values without a fraction.
#[must_use]
pub fn recorded_number(value: f64) -> Value {
    if value.is_finite()
        && value.fract() == 0.0
        && let Some(integral) = num_traits::cast::cast::<f64, i64>(value)
    {
        return json!(integral);
    }
    json!(value)
}
