//! Reader-follow boundaries and usage totals.
use super::*;
use maestro_app::{AgentMessage, NoCustomMessages};

/// Apply observations of `(top, scroll height, viewport height)` in order.
fn after(observations: &[(f64, f64, f64)]) -> Follow {
    let mut follow = Follow::default();
    for &(top, height, client) in observations {
        follow.scrolled(top, height, client);
    }
    follow
}

#[test]
fn following_stops_only_when_scrolling_up_more_than_fifty_pixels_from_the_bottom() {
    assert!(!after(&[(500.0, 1000.0, 400.0), (449.5, 1000.0, 400.0)]).enabled);
    assert!(
        after(&[(600.0, 1000.0, 400.0), (550.0, 1000.0, 400.0)]).enabled,
        "exactly 50 px"
    );
    assert!(
        after(&[(500.0, 1000.0, 400.0), (520.0, 1000.0, 400.0)]).enabled,
        "scrolling down"
    );
    assert!(
        after(&[(100.0, 1000.0, 400.0), (0.0, 1000.0, 400.0)]).enabled,
        "a zero offset"
    );
}

#[test]
fn following_resumes_strictly_within_ten_pixels_of_the_bottom() {
    let stopped = [(500.0, 1000.0, 400.0), (300.0, 1000.0, 400.0)];
    let mut follow = after(&stopped);
    assert!(!follow.enabled);
    follow.scrolled(590.0, 1000.0, 400.0);
    assert!(!follow.enabled, "exactly 10 px keeps the reader's position");
    follow.scrolled(590.25, 1000.0, 400.0);
    assert!(follow.enabled, "fractional offsets count");
}

#[test]
fn a_shrinking_viewport_only_records_its_height() {
    let mut follow = after(&[(500.0, 1000.0, 400.0)]);
    follow.scrolled(300.0, 1000.0, 380.0);
    assert!(follow.enabled, "relayout is not reading");
    follow.scrolled(300.0, 1000.0, 380.0);
    assert!(
        !follow.enabled,
        "the remembered offset is still the one before the relayout"
    );
}

/// An assistant entry reporting `usage`.
fn assistant(usage: &serde_json::Value) -> AgentMessage<NoCustomMessages> {
    serde_json::from_value(serde_json::json!({
        "role": "assistant", "content": [], "api": "openai-completions", "provider": "box",
        "model": "m", "usage": usage, "stopReason": "stop", "timestamp": 1
    }))
    .unwrap()
}
/// A usage report with `cost` total.
fn usage(input: f64, output: f64, read: f64, write: f64, cost: f64) -> serde_json::Value {
    serde_json::json!({
        "input": input, "output": output, "cacheRead": read, "cacheWrite": write,
        "totalTokens": input + output,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": cost}
    })
}

#[test]
fn totals_sum_assistant_usage_and_need_a_counted_token() {
    let user: AgentMessage<NoCustomMessages> = serde_json::from_value(
        serde_json::json!({"role": "user", "content": "hi", "timestamp": 1}),
    )
    .unwrap();
    let messages = [
        assistant(&usage(10.0, 5.0, 1.0, 2.0, 0.5)),
        user,
        assistant(&usage(3.0, 0.0, 0.0, 0.0, 0.25)),
    ];
    let totals = usage_totals(&messages).unwrap();
    assert_eq!(
        (
            totals.input,
            totals.output,
            totals.cache_read,
            totals.cache_write,
            totals.cost.total
        ),
        (13.0, 5.0, 1.0, 2.0, 0.75)
    );
    assert!(
        usage_totals(&[assistant(&usage(0.0, 0.0, 0.0, 0.0, 1.0))]).is_none(),
        "cost alone"
    );
    assert!(usage_totals::<NoCustomMessages>(&[]).is_none());
}
