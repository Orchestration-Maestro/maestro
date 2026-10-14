//! Public queue policy at conversation boundaries.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;

/// Complete recorded queue decisions through public operations.
#[test]
fn mode_replacement_applies_when_queues_are_drained() {
    support::run(support::continuation_corpus::replay("Q2"));
}

/// Complete recorded queue decisions through public operations.
#[test]
fn entry_callbacks_enqueue_before_first_request() {
    support::run(support::continuation_corpus::replay("Q3"));
}

/// Complete recorded queue decisions through public operations.
#[test]
fn steering_waits_for_completed_tool_batches() {
    support::run(async {
        support::queue_effects::batches().await;
        support::continuation_corpus::replay("Q4").await;
    });
}

/// Complete recorded queue decisions through public operations.
#[test]
fn followups_wait_for_tool_and_steering_exhaustion() {
    support::run(async {
        support::queue_effects::follow_priority().await;
        support::continuation_corpus::replay("Q5").await;
    });
}

/// Complete recorded queue decisions through public operations.
#[test]
fn failed_or_aborted_turns_leave_unconsumed_queues() {
    support::run(support::continuation_corpus::replay("Q7"));
}

/// Complete recorded queue decisions through public operations.
#[test]
fn clear_operations_keep_other_queue_independent() {
    support::queue_effects::clear_reentry();
    support::run(support::continuation_corpus::replay("Q8"));
}

/// Active cancellation and reset use the same retained run signal.
#[test]
fn abort_preserves_and_reset_clears_pending_inputs() {
    support::run(support::binding_corpus::controls("active_control"));
}

/// Repeated handles and distinct equal-valued entries remain ordered.
#[test]
fn queues_drain_in_order_under_both_policies() {
    support::run(async {
        for mode in [
            maestro_agent::QueueMode::All,
            maestro_agent::QueueMode::OneAtATime,
        ] {
            for follow in [false, true] {
                queue_identity(mode, follow).await;
            }
        }
    });
}

/// Drive one queue policy and compare retained allocation identities.
async fn queue_identity(mode: maestro_agent::QueueMode, follow: bool) {
    let (agent, tasks) = support::owned::agent();
    agent.set_steering_mode(mode);
    agent.set_follow_up_mode(mode);
    let first = support::user("zulu");
    let second = support::user("alpha");
    let distinct = support::user("zulu");
    for entry in [
        first.clone(),
        second.clone(),
        first.clone(),
        distinct.clone(),
    ] {
        if follow {
            agent.follow_up(entry);
        } else {
            agent.steer(entry);
        }
    }
    let requests = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    capture_users(&agent, &requests);
    let observe = agent.prompt(support::owned::input("prompt"));
    tasks.take().await;
    observe.await.unwrap();
    let requests = requests.lock().unwrap();
    assert_eq!(requests.last().unwrap(), &["zulu", "alpha", "zulu", "zulu"]);
    assert_eq!(
        requests.len(),
        if mode == maestro_agent::QueueMode::All {
            1 + usize::from(follow)
        } else {
            4 + usize::from(follow)
        }
    );
    drop(requests);
    let state = agent.state().read().unwrap();
    let messages = state.messages().read().unwrap();
    let users = messages
        .iter()
        .filter_map(|v| {
            if let maestro_agent::AgentMessage::User(v) = v {
                Some(v)
            } else {
                None
            }
        })
        .skip(1)
        .collect::<Vec<_>>();
    for (actual, expected) in users.into_iter().zip([&first, &second, &first, &distinct]) {
        let maestro_agent::AgentMessage::User(expected) = expected else {
            unreachable!()
        };
        assert!(std::sync::Arc::ptr_eq(actual, expected));
    }
}

/// Capture queued text operands separately from the block-form initial prompt.
fn capture_users(
    agent: &maestro_agent::Agent,
    requests: &std::sync::Arc<std::sync::Mutex<Vec<Vec<String>>>>,
) {
    let captured = requests.clone();
    *agent.stream_fn.write().unwrap() = Some(std::sync::Arc::new(move |_, context, _, _| {
        captured.lock().unwrap().push(
            context
                .messages
                .iter()
                .filter_map(|v| match v {
                    maestro_models::Message::User(v) => match &v.content {
                        maestro_models::UserContent::Text(v) => Some(v.clone()),
                        maestro_models::UserContent::Blocks(_) => None,
                    },
                    _ => None,
                })
                .collect::<Vec<_>>(),
        );
        let stream = maestro_models::AssistantMessageEventStream::new();
        stream.end(Some(support::assistant("done")));
        support::ready(Ok(stream))
    }));
}

/// Constructor policies reach independent queue drains in a public run.
#[test]
fn constructor_queue_modes_are_consumed_independently() {
    support::run(mixed_modes(ModeChange::Constructor));
}

/// Replacing one policy after enqueue leaves the sibling drain unchanged.
#[test]
fn changing_one_queue_mode_after_enqueue_preserves_sibling_policy() {
    support::run(mixed_modes(ModeChange::AfterEnqueue));
}

/// A policy changed inside a model turn governs later queue consumption.
#[test]
fn changing_one_queue_mode_during_turn_preserves_sibling_policy() {
    support::run(mixed_modes(ModeChange::DuringTurn));
}

/// The public operation at which one independent policy is selected.
#[derive(Clone, Copy, PartialEq)]
enum ModeChange {
    /// Both unequal policies are supplied at construction.
    Constructor,
    /// One policy is changed after entries have been queued.
    AfterEnqueue,
    /// Entries and one policy change occur inside the first model call.
    DuringTurn,
}

/// Exercise each unequal combination with each setter independently.
async fn mixed_modes(change: ModeChange) {
    use maestro_agent::QueueMode::{All, OneAtATime};
    for (steering, follow) in [(All, OneAtATime), (OneAtATime, All)] {
        for change_steering in [true, false] {
            if change == ModeChange::Constructor && !change_steering {
                continue;
            }
            mixed_mode_run(steering, follow, change_steering, change).await;
        }
    }
}

/// Queue distinguishable repeated entries before selecting one policy.
fn enqueue_mixed(agent: &maestro_agent::Agent) {
    for text in ["zulu", "alpha", "zulu"] {
        agent.steer(support::user(&format!("steer:{text}")));
        agent.follow_up(support::user(&format!("follow:{text}")));
    }
}

/// Choose only the requested policy; the sibling retains its constructor value.
fn change_mode(agent: &maestro_agent::Agent, steering: bool, mode: maestro_agent::QueueMode) {
    if steering {
        agent.set_steering_mode(mode);
    } else {
        agent.set_follow_up_mode(mode);
    }
}

/// Drive one run to settlement and assert ordered request boundaries.
async fn mixed_mode_run(
    steering: maestro_agent::QueueMode,
    follow: maestro_agent::QueueMode,
    change_steering: bool,
    change: ModeChange,
) {
    use maestro_agent::{Agent, AgentOptions};
    use std::sync::{Arc, Mutex};
    let tasks = Arc::new(support::owned::Tasks::default());
    let replace = change != ModeChange::Constructor;
    let agent = Arc::new(Agent::new(AgentOptions {
        spawn: Some(tasks.spawner()),
        steering_mode: Some(if replace && change_steering {
            follow
        } else {
            steering
        }),
        follow_up_mode: Some(if replace && !change_steering {
            steering
        } else {
            follow
        }),
        ..Default::default()
    }));
    let selected = if change_steering { steering } else { follow };
    if change != ModeChange::DuringTurn {
        enqueue_mixed(&agent);
        if replace {
            change_mode(&agent, change_steering, selected);
        }
    }
    let requests = Arc::new(Mutex::new(Vec::new()));
    capture_mixed(&agent, &requests, selected, change_steering, change);
    let observe = agent.prompt(support::owned::input("prompt"));
    tasks.take().await;
    observe.await.unwrap();
    assert_mixed_boundaries(&requests.lock().unwrap(), steering, change);
    assert!(!agent.has_queued_messages());
}

/// Observe requests and replace one policy inside the initial model turn.
fn capture_mixed(
    agent: &std::sync::Arc<maestro_agent::Agent>,
    requests: &std::sync::Arc<std::sync::Mutex<Vec<Vec<String>>>>,
    selected: maestro_agent::QueueMode,
    change_steering: bool,
    change: ModeChange,
) {
    use std::sync::Arc;
    let captured = requests.clone();
    let owner = Arc::downgrade(agent);
    *agent.stream_fn.write().unwrap() = Some(Arc::new(move |_, context, _, _| {
        let users = context
            .messages
            .iter()
            .filter_map(|message| match message {
                maestro_models::Message::User(message) => match &message.content {
                    maestro_models::UserContent::Text(text) => Some(text.clone()),
                    maestro_models::UserContent::Blocks(_) => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        let first = {
            let mut requests = captured.lock().unwrap();
            requests.push(users);
            requests.len() == 1
        };
        if first && change == ModeChange::DuringTurn {
            let agent = owner.upgrade().unwrap();
            enqueue_mixed(&agent);
            change_mode(&agent, change_steering, selected);
        }
        let stream = maestro_models::AssistantMessageEventStream::new();
        stream.end(Some(support::assistant("done")));
        support::ready(Ok(stream))
    }));
}

/// Compare ordered cumulative request contents, including duplicate entries.
fn assert_mixed_boundaries(
    requests: &[Vec<String>],
    steering: maestro_agent::QueueMode,
    change: ModeChange,
) {
    use maestro_agent::QueueMode;
    let all = [
        "steer:zulu",
        "steer:alpha",
        "steer:zulu",
        "follow:zulu",
        "follow:alpha",
        "follow:zulu",
    ];
    let boundaries = if steering == QueueMode::All {
        [3, 4, 5, 6]
    } else {
        [1, 2, 3, 6]
    };
    let mut expected = Vec::new();
    if change == ModeChange::DuringTurn {
        expected.push(Vec::new());
    }
    expected.extend(boundaries.map(|end| {
        all[..end]
            .iter()
            .map(|v| (*v).to_owned())
            .collect::<Vec<_>>()
    }));
    assert_eq!(requests, expected);
}
