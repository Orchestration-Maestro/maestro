//! Native producer completion and destructor reentry at queue boundaries.
use super::{assistant, owned, ready};
use maestro_agent::{
    Agent, AgentMessage, AgentOptions, AgentTool, CustomAgentMessages, ToolExecutionMode,
};
use maestro_models::{
    AssistantContent, AssistantMessageEventStream, JsonObject, TextContent, ToolCall, UserBlock,
};
use std::sync::{Arc, Mutex, RwLock, Weak};
/// Driver-side release endpoints for the two controlled tools.
type Permits = Vec<Option<tokio::sync::oneshot::Sender<()>>>;
/// Driver-side readiness or completion endpoints for the two controlled tools.
type Receipts = Vec<Option<tokio::sync::oneshot::Receiver<()>>>;
/// Native overlapping tool effects settle before queued steering dispatch.
///
/// # Panics
/// Panics if readiness or complete-batch ordering is lost.
pub async fn batches() {
    for mode in [ToolExecutionMode::Parallel, ToolExecutionMode::Sequential] {
        batch(mode).await;
    }
}
/// Drive one native tool-batch policy with named effect acknowledgments.
async fn batch(mode: ToolExecutionMode) {
    let (agent, tasks) = owned::agent();
    *agent.tool_execution.write().unwrap() = mode;
    let (mut permits, mut entries, mut completions) = tools(&agent);
    let requests = Arc::new(Mutex::new(Vec::new()));
    responses(&agent, &requests);
    let observe = agent.prompt(owned::input("start"));
    let joined = tokio::spawn(tasks.take());
    entries[0].take().unwrap().await.unwrap();
    if mode == ToolExecutionMode::Parallel {
        entries[1].take().unwrap().await.unwrap();
        permits[1].take().unwrap().send(()).unwrap();
        completions[1].take().unwrap().await.unwrap();
        assert_eq!(requests.lock().unwrap().len(), 1);
        permits[0].take().unwrap().send(()).unwrap();
        completions[0].take().unwrap().await.unwrap();
    } else {
        assert_eq!(requests.lock().unwrap().len(), 1);
        permits[0].take().unwrap().send(()).unwrap();
        completions[0].take().unwrap().await.unwrap();
        entries[1].take().unwrap().await.unwrap();
        assert_eq!(requests.lock().unwrap().len(), 1);
        permits[1].take().unwrap().send(()).unwrap();
        completions[1].take().unwrap().await.unwrap();
    }
    joined.await.unwrap();
    observe.await.unwrap();
    assert_batch(&requests);
}
/// Assert full post-settlement request count and source-ordered artifacts.
fn assert_batch(requests: &Arc<Mutex<Vec<Vec<maestro_models::Message>>>>) {
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    let results = requests[1]
        .iter()
        .filter_map(|v| {
            if let maestro_models::Message::ToolResult(v) = v {
                Some(v.tool_call_id.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(results, ["zulu", "alpha"]);
}
/// Dispatch a tool batch once, then terminating responses while recording requests.
fn responses(agent: &Agent, requests: &Arc<Mutex<Vec<Vec<maestro_models::Message>>>>) {
    let recorded = requests.clone();
    *agent.stream_fn.write().unwrap() = Some(Arc::new(move |_, context, _, _| {
        let first = {
            let mut requests = recorded.lock().unwrap();
            let first = requests.is_empty();
            requests.push(context.messages);
            first
        };
        let message = assistant("done");
        if first {
            message.write().unwrap().content = ["zulu", "alpha"]
                .into_iter()
                .map(|id| {
                    AssistantContent::ToolCall(ToolCall {
                        id: id.into(),
                        name: "echo".into(),
                        arguments: JsonObject::new().into(),
                        thought_signature: None,
                    })
                })
                .collect();
        }
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(message));
        ready(Ok(stream))
    }));
}
/// Build gated tool effects and return their driver-side readiness endpoints.
fn tools(agent: &Arc<Agent>) -> (Permits, Receipts, Receipts) {
    let mut permits = Vec::new();
    let mut entries = Vec::new();
    let mut completions = Vec::new();
    let mut gates = Vec::new();
    for _ in 0..2 {
        let (enter, entered) = tokio::sync::oneshot::channel();
        let (release, permission) = tokio::sync::oneshot::channel();
        let (complete, completed) = tokio::sync::oneshot::channel();
        permits.push(Some(release));
        entries.push(Some(entered));
        completions.push(Some(completed));
        gates.push(Some((enter, permission, complete)));
    }
    let gates = Mutex::new(gates);
    let owner = Arc::downgrade(agent);
    let tool = Arc::new(RwLock::new(AgentTool {
        description: None,
        definition: maestro_models::Tool {
            name: "echo".into(),
            description: "Echo".into(),
            parameters: serde_json::json!({"type":"object"}).into(),
        },
        label: "Echo".into(),
        prepare_arguments: None,
        execution_mode: None,
        execute: Arc::new(move |id, _, _, _| {
            owner
                .upgrade()
                .unwrap()
                .steer(super::user(&format!("during:{id}")));
            let index = if id == "zulu" {
                0
            } else {
                assert_eq!(id, "alpha");
                1
            };
            let (enter, permission, complete) = gates.lock().unwrap()[index].take().unwrap();
            Box::pin(async move {
                enter.send(()).unwrap();
                permission.await.unwrap();
                let result = maestro_agent::AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(format!("result:{id}")))],
                    details: Some(serde_json::Value::Null),
                    terminate: None,
                    usage: None,
                };
                complete.send(()).unwrap();
                Ok(result)
            })
        }),
    }));
    agent.state().write().unwrap().set_tools(vec![tool]);
    (permits, entries, completions)
}
/// Payload retired by a queue clear.
struct Discard {
    /// Weak owner permits destructor reentry without a retained cycle.
    owner: Weak<Agent<Discard>>,
    /// Completed drop count.
    drops: Arc<std::sync::atomic::AtomicUsize>,
}
impl CustomAgentMessages for Discard {
    fn role(&self) -> &'static str {
        "discard"
    }
}
impl Drop for Discard {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.upgrade() {
            let _ = owner.has_queued_messages();
        }
        self.drops
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}
/// Clearing an individual queue permits the final payload alias outside its guard.
///
/// # Panics
/// Panics when clear retires the other queue or retains a discarded payload.
pub fn clear_reentry() {
    for steering in [false, true] {
        let agent: Arc<Agent<Discard>> = Arc::new(Agent::new(AgentOptions::default()));
        let drops = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        for queue in [false, true] {
            let value = AgentMessage::Custom(Arc::new(RwLock::new(Discard {
                owner: Arc::downgrade(&agent),
                drops: drops.clone(),
            })));
            if queue {
                agent.steer(value);
            } else {
                agent.follow_up(value);
            }
        }
        if steering {
            agent.clear_steering_queue();
        } else {
            agent.clear_follow_up_queue();
        }
        assert_eq!(drops.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert!(agent.has_queued_messages());
        agent.clear_all_queues();
        assert_eq!(drops.load(std::sync::atomic::Ordering::Relaxed), 2);
        assert!(!agent.has_queued_messages());
    }
}

/// Follow-ups remain behind a tool's injected steering under either drain policy.
///
/// # Panics
/// Panics if the second request consumes deferred input before steering.
pub async fn follow_priority() {
    for mode in [
        maestro_agent::QueueMode::All,
        maestro_agent::QueueMode::OneAtATime,
    ] {
        let (agent, tasks) = owned::agent();
        agent.set_steering_mode(mode);
        agent.set_follow_up_mode(mode);
        let owner = Arc::downgrade(&agent);
        let tool = super::with_execute(Arc::new(move |_, _, _, _| {
            owner
                .upgrade()
                .unwrap()
                .steer(super::user("priority-steering"));
            ready(Ok(maestro_agent::AgentToolResult {
                content: vec![],
                details: Some(serde_json::Value::Null),
                terminate: None,
                usage: None,
            }))
        }))
        .tools
        .unwrap()
        .read()
        .unwrap()[0]
            .clone();
        agent.state().write().unwrap().set_tools(vec![tool]);
        let requests = Arc::new(Mutex::new(Vec::new()));
        priority_responses(&agent, &requests);
        agent.follow_up(super::user("deferred-a"));
        agent.follow_up(super::user("deferred-b"));
        let observe = agent.prompt(owned::input("priority"));
        let joined = tokio::spawn(tasks.take());
        joined.await.unwrap();
        observe.await.unwrap();
        let requests = requests.lock().unwrap();
        assert_eq!(
            requests[1],
            ["priority-steering"],
            "follow-up overtook tool steering"
        );
        assert_eq!(
            requests.last().unwrap(),
            &["priority-steering", "deferred-a", "deferred-b"]
        );
        assert_eq!(
            requests.len(),
            if mode == maestro_agent::QueueMode::All {
                3
            } else {
                4
            }
        );
    }
}

/// Record request operands across tool, steering and deferred-input phases.
fn priority_responses(agent: &Agent, requests: &Arc<Mutex<Vec<Vec<String>>>>) {
    let recorded = requests.clone();
    *agent.stream_fn.write().unwrap() = Some(Arc::new(move |_, context, _, _| {
        let users = context
            .messages
            .iter()
            .filter_map(|v| match v {
                maestro_models::Message::User(v) => match &v.content {
                    maestro_models::UserContent::Text(v) => Some(v.clone()),
                    maestro_models::UserContent::Blocks(_) => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        let first = {
            let mut requests = recorded.lock().unwrap();
            let first = requests.is_empty();
            requests.push(users);
            first
        };
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(if first {
            super::tool_call()
        } else {
            assistant("done")
        }));
        ready(Ok(stream))
    }));
}
