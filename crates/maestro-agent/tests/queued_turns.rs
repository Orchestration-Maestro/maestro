//! Public queue policy at conversation boundaries.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
/// Controlled public-operation fixtures.
pub mod support;

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
