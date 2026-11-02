//! Executable schemas at preparation, validation and typed execution boundaries.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::agent_loop::LocalAgentLoopOptions;
use maestro_agent::{AgentContextRecord, AgentLoopConfigRecord, AgentToolRecord, NoCustomMessages};
use maestro_models::{
    AssistantContent, Base, Local, ParameterSchema, SchemaCallback, SchemaCode, SchemaKind,
    SchemaNode, Tool, ToolCall,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{cell::RefCell, collections::VecDeque, rc::Rc, sync::RwLock};

/// The validation or typed-decoding branch selected by the author.
#[derive(Clone, Copy)]
enum Outcome {
    /// Validation and typed decoding both succeed.
    Execute,
    /// The custom guard rejects before execution.
    Reject,
    /// Validation succeeds but typed decoding rejects.
    InvalidDecode,
}
/// One retained local tool record.
type SharedTool = Rc<RwLock<AgentToolRecord<Local>>>;
/// Values observed by the typed execution body.
type Executions = Rc<RefCell<Vec<Value>>>;
/// Construct one retained local tool and its execution observations.
fn tool(preparation: bool, outcome: Outcome) -> (SharedTool, Executions) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let executed = Rc::clone(&seen);
    let code = SchemaCode::from([(
        "/properties/value".into(),
        SchemaNode {
            base: Some(Base {
                convert: converter("7", outcome),
                check: SchemaCallback::new(move |_| Ok(!matches!(outcome, Outcome::Reject))),
                ..Default::default()
            }),
            refinements: Vec::new(),
        },
    )]);
    let tool = AgentToolRecord::<Local>::typed(
        Tool {
            name: "echo".into(),
            description: "Echo".into(),
            parameters: ParameterSchema::<()>::authored(
                json!({"type":"object","properties":{"value":{}},"required":["value"]}),
                [
                    (String::new(), SchemaKind::Object),
                    ("/properties/value".into(), SchemaKind::Base),
                ],
            )
            .with_code(code),
        },
        "Echo".into(),
        Rc::new(move |_, args: Arguments, _, _| {
            executed.borrow_mut().push(json!(args.value));
            Box::pin(std::future::ready(Ok(support::result())))
        }),
    );
    let tool = Rc::new(RwLock::new(tool));
    if preparation {
        let owner = Rc::downgrade(&tool);
        tool.write().unwrap().prepare_arguments = Some(Rc::new(move |value| {
            assert_eq!(*value, json!({"value":"7"}));
            let tool = owner.upgrade().unwrap();
            let parameters = tool.read().unwrap().definition.parameters.clone();
            let mut code = parameters.code().clone();
            code.get_mut("/properties/value")
                .unwrap()
                .base
                .as_mut()
                .unwrap()
                .convert = converter("8", outcome);
            tool.write().unwrap().definition.parameters = parameters.with_code(code);
            Ok(json!({"value":"8"}))
        }));
    }
    install_unlocked_guard(&tool);
    (tool, seen)
}
/// Distinguish the original declaration from the declaration preparation installs.
fn converter(expected: &'static str, outcome: Outcome) -> SchemaCallback<Option<Value>> {
    SchemaCallback::new(move |value| {
        assert_eq!(
            *value,
            json!(expected),
            "validation reads the declaration after preparation"
        );
        Ok(Some(if matches!(outcome, Outcome::InvalidDecode) {
            json!("bad")
        } else {
            json!(9)
        }))
    })
}
/// Keep author execution outside the retained tool's lock.
fn install_unlocked_guard(tool: &SharedTool) {
    let unlocked = Rc::downgrade(tool);
    let check = SchemaCallback::new(move |_| {
        assert!(
            unlocked.upgrade().unwrap().try_write().is_ok(),
            "schema author runs outside tool lock"
        );
        Ok(true)
    });
    // The root guard observes the real retained tool while member conversion runs.
    let parameters = tool.read().unwrap().definition.parameters.clone();
    let mut code = parameters.code().clone();
    code.insert(
        String::new(),
        SchemaNode {
            base: Some(Base {
                check,
                ..Default::default()
            }),
            refinements: Vec::new(),
        },
    );
    tool.write().unwrap().definition.parameters = parameters.with_code(code);
}
/// Execute one controlled call and return exactly the typed body observations.
fn run(preparation: bool, outcome: Outcome) -> Vec<Value> {
    let (tool, seen) = tool(preparation, outcome);
    let response = support::assistant("call");
    response.write().unwrap().content = vec![AssistantContent::ToolCall(ToolCall {
        id: "id".into(),
        name: "echo".into(),
        arguments: json!({"value":"7"}),
        thought_signature: None,
    })];
    let queue = Rc::new(RefCell::new(VecDeque::from([
        response.clone(),
        support::assistant("done"),
    ])));
    let config = configuration();
    drive(tool, &config, queue);
    let AssistantContent::ToolCall(call) = &response.read().unwrap().content[0] else {
        panic!("tool call")
    };
    assert_eq!(call.arguments, json!({"value":"7"}));
    Rc::try_unwrap(seen).unwrap().into_inner()
}
/// Resolve the standard history through local provider callbacks.
fn configuration() -> AgentLoopConfigRecord<NoCustomMessages, Local> {
    AgentLoopConfigRecord {
        model: support::config().model,
        options: maestro_models::SimpleStreamOptions::default(),
        extra: serde_json::Map::default(),
        objects: maestro_models::ProviderObjects::default(),
        thinking_budgets: None,
        convert_to_llm: <Local as maestro_agent::AgentExecution>::default_converter(),
        transform_context: None,
        get_api_key: None,
        before_tool_call: None,
        after_tool_call: None,
        should_stop_after_turn: None,
        prepare_next_turn: None,
        get_steering_messages: None,
        get_follow_up_messages: None,
        tool_execution: None,
    }
}
/// Drive the existing public local loop without registering a provider.
fn drive(
    tool: SharedTool,
    config: &AgentLoopConfigRecord<NoCustomMessages, Local>,
    queue: Rc<RefCell<VecDeque<maestro_models::SharedAssistantMessage>>>,
) {
    support::run(maestro_agent::agent_loop::local::run_agent_loop_continue(
        AgentContextRecord {
            system_prompt: String::new(),
            messages: std::sync::Arc::new(RwLock::new(vec![support::user("earlier")])),
            tools: Some(Rc::new(RwLock::new(vec![tool]))),
        },
        config,
        Rc::new(|_| Box::pin(std::future::ready(Ok(())))),
        LocalAgentLoopOptions::ReplacementStream {
            signal: None,
            stream_fn: Rc::new(move |_, _, _, _| {
                let stream = maestro_models::AssistantMessageEventStream::new();
                stream.end(Some(queue.borrow_mut().pop_front().unwrap()));
                Box::pin(std::future::ready(Ok(stream)))
            }),
        },
    ))
    .unwrap();
}
/// The author's execution type is decoded only after supplied schema code.
#[derive(Deserialize)]
struct Arguments {
    /// The converted argument.
    value: u32,
}
#[test]
fn agent_execution_runs_schema_code_after_preparation() {
    for preparation in [false, true] {
        assert_eq!(run(preparation, Outcome::Execute), [json!(9)]);
        assert!(run(preparation, Outcome::Reject).is_empty());
    }
}
#[test]
fn typed_execution_deserializes_after_schema_code() {
    assert_eq!(run(false, Outcome::Execute), [json!(9)]);
    assert!(run(false, Outcome::InvalidDecode).is_empty());
}
