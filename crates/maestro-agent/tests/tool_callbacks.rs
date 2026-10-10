//! Public typed callback conversion through the common tool record.
#![cfg(test)]
use maestro_agent::{AgentTool, AgentToolResult, ExecuteTool};
use maestro_models::{BoxFuture, DiagnosticErrorInfo, Tool, UserBlock};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll, Waker},
};

/// Parameters with a named field, distinct from their resulting details.
#[derive(Deserialize)]
struct Parameters {
    /// Operand transformed by the body.
    count: u32,
}
/// Structured result details.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct Details {
    /// Transformed operand.
    doubled: u32,
}
/// Poll the controlled ready callbacks without a runtime dependency.
fn completed<T>(mut future: BoxFuture<T>) -> T {
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("controlled callback must be ready"),
    }
}
/// A declaration for the callback boundary, not a schema validation invocation.
fn definition() -> Tool {
    Tool {
        name: "double".into(),
        description: String::new(),
        parameters: json!({"type":"object"}),
    }
}
/// Controlled body with a distinct progress result.
fn typed_body(calls: Arc<AtomicUsize>) -> ExecuteTool<Parameters, Details> {
    Arc::new(move |id, args, signal, update| {
        assert_eq!(id, "call-1");
        assert!(signal.is_none());
        calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            update.unwrap()(AgentToolResult {
                content: vec![UserBlock::Text(maestro_models::TextContent {
                    text: "partial".into(),
                    text_signature: None,
                })],
                details: Details {
                    doubled: args.count + 1,
                },
                terminate: Some(false),
            });
            Ok(AgentToolResult {
                content: vec![UserBlock::Text(maestro_models::TextContent {
                    text: "complete".into(),
                    text_signature: None,
                })],
                details: Details {
                    doubled: args.count * 2,
                },
                terminate: Some(true),
            })
        })
    })
}
/// Typed arguments, progress and final details survive the erased callback boundary.
#[test]
fn maestro_typed_tool_round_trips_arguments_results_and_progress() {
    let calls = Arc::new(AtomicUsize::new(0));
    let execute = typed_body(Arc::clone(&calls));
    let tool = AgentTool::typed(definition(), "Double".into(), execute);
    let partials = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&partials);
    let result = completed((tool.execute)(
        "call-1".into(),
        json!({"count":7}),
        None,
        Some(Arc::new(move |partial| {
            captured.lock().unwrap().push(partial);
        })),
    ))
    .unwrap();
    assert_eq!(
        serde_json::from_value::<Details>(result.details).unwrap(),
        Details { doubled: 14 }
    );
    assert_eq!(
        serde_json::to_value(result.content).unwrap(),
        json!([{"type":"text","text":"complete"}])
    );
    assert_eq!(result.terminate, Some(true));
    let partial = partials.lock().unwrap().pop().unwrap();
    assert_eq!(
        serde_json::from_value::<Details>(partial.details).unwrap(),
        Details { doubled: 8 }
    );
    assert_eq!(
        serde_json::to_value(partial.content).unwrap(),
        json!([{"type":"text","text":"partial"}])
    );
    assert_eq!(partial.terminate, Some(false));
    let error = completed((tool.execute)(
        "call-1".into(),
        json!({"count":"wrong"}),
        None,
        None,
    ))
    .err()
    .unwrap();
    assert!(error.message.contains("invalid type"), "{}", error.message);
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "decode failure must not call body"
    );
}
/// Host details follow ordinary JSON serialization, including null for nonfinite numbers.
#[test]
fn maestro_typed_tool_uses_host_json_number_semantics() {
    let tool = AgentTool::typed::<u32, f64>(
        definition(),
        "Numbers".into(),
        Arc::new(|_, _, _, _| {
            Box::pin(async {
                Ok(AgentToolResult {
                    content: Vec::new(),
                    details: f64::INFINITY,
                    terminate: None,
                })
            })
        }),
    );
    let result = completed((tool.execute)("numbers".into(), json!(1), None, None)).unwrap();
    assert_eq!(result.details, serde_json::Value::Null);
    assert_eq!(result.terminate, None);
}
/// A details type whose serializer fails rather than producing JSON.
struct Unencodable(bool);
impl Serialize for Unencodable {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0 {
            Err(serde::ser::Error::custom("details cannot encode"))
        } else {
            serializer.serialize_unit()
        }
    }
}
/// Select a final failure or a failed update followed by encodable final details.
fn encoding_failure_tool(progress: bool) -> AgentTool {
    AgentTool::typed::<u32, Unencodable>(
        definition(),
        "Failure".into(),
        Arc::new(move |_, _, _, update| {
            Box::pin(async move {
                if progress {
                    update.unwrap()(AgentToolResult {
                        content: Vec::new(),
                        details: Unencodable(true),
                        terminate: None,
                    });
                }
                Ok(AgentToolResult {
                    content: Vec::new(),
                    details: Unencodable(!progress),
                    terminate: None,
                })
            })
        }),
    )
}
/// Final and progress serialization failures are returned, and body failures survive.
#[test]
fn maestro_typed_tool_returns_callback_and_encoding_failures() {
    for progress in [false, true] {
        let tool = encoding_failure_tool(progress);
        let error = completed((tool.execute)(
            "failure".into(),
            json!(1),
            None,
            Some(Arc::new(|_| panic!("failed progress must not publish"))),
        ))
        .err()
        .unwrap();
        assert_eq!(error.message, "details cannot encode");
    }
    let tool = AgentTool::typed::<u32, ()>(
        definition(),
        "Failure".into(),
        Arc::new(|_, _, _, _| {
            Box::pin(async {
                Err(DiagnosticErrorInfo {
                    name: None,
                    message: "body failed".into(),
                    stack: None,
                    code: None,
                })
            })
        }),
    );
    assert_eq!(
        completed((tool.execute)("failure".into(), json!(1), None, None))
            .err()
            .unwrap()
            .message,
        "body failed"
    );
}
