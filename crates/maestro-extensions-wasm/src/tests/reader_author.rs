//! Author-side reader operations used unchanged by both adapters.
use maestro_extensions_wasm::{
    CommandOptions, ExtensionAPI, ExtensionCommandContext, ExtensionContext, ExtensionEvent,
    ExtensionEventResult, ExtensionResult, InputEventResult, InputTransform, Presence,
    ReadonlySessionManager, SessionTreeNode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;

/// Test operation delivered as input text or command arguments.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    /// Synchronously acknowledged host mutation before the operation.
    control: Option<Value>,
    /// Operation on the author facade.
    action: String,
    /// Query spelling.
    #[serde(default)]
    method: String,
    /// Literal query operands.
    #[serde(default)]
    args: Vec<Value>,
    /// Select a previously retained reader rather than acquiring one.
    #[serde(default)]
    retained: bool,
}
/// Handles deliberately retained between callback invocations.
#[derive(Default)]
struct Kept {
    /// Reader aliases, each sharing its original owner.
    readers: Vec<ReadonlySessionManager>,
    /// Captured tree node independent of its ancestors.
    node: Option<SessionTreeNode>,
    /// Ordinary context retained for stale acquisition checks.
    ordinary: Option<ExtensionContext>,
    /// Command context retained for stale acquisition checks.
    command: Option<ExtensionCommandContext>,
}
/// Installs the event and command reader probes only for reader scenarios.
pub fn register(api: &ExtensionAPI) -> ExtensionResult<()> {
    let kept = Rc::new(RefCell::new(Kept::default()));
    let event_kept = Rc::clone(&kept);
    let log = api.clone();
    api.on(
        "reader",
        Rc::new(move |event, ctx| {
            let kept = Rc::clone(&event_kept);
            let log = log.clone();
            Box::pin(async move {
                let ExtensionEvent::Input(input) = event else {
                    return Err("expected input".into());
                };
                let request = prepare(&log, &input.text)?;
                let outcome = run(&kept, &request, &ctx);
                let text = answer(outcome).to_string();
                Ok(Some(ExtensionEventResult::Input(
                    InputEventResult::Transform(InputTransform {
                        text,
                        images: Presence::Missing,
                    }),
                )))
            })
        }),
    )?;
    let log = api.clone();
    api.register_command(
        "reader",
        CommandOptions {
            description: None,
            handler: Rc::new(move |args, ctx| {
                let kept = Rc::clone(&kept);
                let log = log.clone();
                Box::pin(async move {
                    let request = prepare(&log, &args)?;
                    if request.action == "keep-context" {
                        kept.borrow_mut().command = Some(ctx.clone());
                    }
                    let outcome = run(&kept, &request, &ctx);
                    log.append_entry("reader-answer", Some(answer(outcome)))
                })
            }),
        },
    )
}
/// Apply host changes before reading any capability.
fn prepare(api: &ExtensionAPI, text: &str) -> ExtensionResult<Request> {
    let mut request: Request = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if let Some(control) = request.control.take() {
        api.append_entry("reader-control", Some(control))?;
    }
    Ok(request)
}
/// Return successes and caught errors through the same public output.
fn answer(result: ExtensionResult<Value>) -> Value {
    match result {
        Ok(value) => json!({"ok":value}),
        Err(error) => json!({"error":error}),
    }
}
/// Execute one operation without holding a borrow across host calls.
fn run(kept: &RefCell<Kept>, request: &Request, ctx: &ExtensionContext) -> ExtensionResult<Value> {
    match request.action.as_str() {
        "keep-context" => {
            kept.borrow_mut().ordinary = Some(ctx.clone());
            return Ok(json!("kept"));
        }
        "old-ordinary" => {
            let ctx = kept.borrow().ordinary.clone().ok_or("missing context")?;
            return query(&ctx.session_manager()?, &request.method, &request.args);
        }
        "old-command" => {
            let ctx = kept.borrow().command.clone().ok_or("missing context")?;
            return query(&ctx.session_manager()?, &request.method, &request.args);
        }
        "drop" => {
            let old = std::mem::take(&mut *kept.borrow_mut());
            drop(old);
            return Ok(json!("dropped"));
        }
        "drop-reader" => {
            let old = std::mem::take(&mut kept.borrow_mut().readers);
            drop(old);
            return Ok(json!("dropped"));
        }
        "drop-alias" => {
            let old = kept.borrow_mut().readers.pop();
            drop(old);
            return Ok(json!("dropped"));
        }
        "node" => {
            let node = kept.borrow().node.clone().ok_or("missing node")?;
            return node_query(&node, &request.method);
        }
        _ => {}
    }
    let reader = if request.retained {
        kept.borrow()
            .readers
            .first()
            .cloned()
            .ok_or("missing reader")?
    } else {
        ctx.session_manager()?
    };
    run_reader(kept, request, reader)
}
/// Execute the operation that requires a reader.
fn run_reader(
    kept: &RefCell<Kept>,
    request: &Request,
    reader: ReadonlySessionManager,
) -> ExtensionResult<Value> {
    match request.action.as_str() {
        "retain" => {
            kept.borrow_mut().readers.push(reader);
            Ok(json!("kept"))
        }
        "alias" => {
            kept.borrow_mut().readers.push(reader);
            Ok(json!("aliased"))
        }
        "retain-node" => {
            let roots = reader.get_tree()?;
            let mut node = roots.into_iter().next().ok_or("missing root")?;
            for _ in 0..request.args.first().and_then(Value::as_u64).unwrap_or(0) {
                node = node.children()?.into_iter().next().ok_or("missing child")?;
            }
            kept.borrow_mut().node = Some(node);
            Ok(json!("kept"))
        }
        "query" => query(&reader, &request.method, &request.args),
        _ => Err("unknown reader action".into()),
    }
}
/// Serialize the typed record actually returned by the facade.
fn encoded(value: impl serde::Serialize) -> ExtensionResult<Value> {
    serde_json::to_value(value).map_err(|e| e.to_string())
}
/// Forward literal query operands to the public facade.
fn query(reader: &ReadonlySessionManager, method: &str, args: &[Value]) -> ExtensionResult<Value> {
    let id = || args.first().and_then(Value::as_str).unwrap_or("");
    match method {
        "getCwd" => encoded(reader.get_cwd()?),
        "getSessionDir" => encoded(reader.get_session_dir()?),
        "getSessionId" => encoded(reader.get_session_id()?),
        "getSessionFile" => encoded(reader.get_session_file()?),
        "getLeafId" => encoded(reader.get_leaf_id()?),
        "getLeafEntry" => encoded(reader.get_leaf_entry()?),
        "getEntry" => encoded(reader.get_entry(id())?),
        "getLabel" => encoded(reader.get_label(id())?),
        "getBranch" => encoded(reader.get_branch(args.first().and_then(Value::as_str))?),
        "getHeader" => encoded(reader.get_header()?),
        "getEntries" => encoded(reader.get_entries()?),
        "getTree" => tree(reader.get_tree()?),
        "getSessionName" => encoded(reader.get_session_name()?),
        _ => Err("unknown query".into()),
    }
}
/// Query one independently retained node.
fn node_query(node: &SessionTreeNode, method: &str) -> ExtensionResult<Value> {
    match method {
        "entry" => encoded(node.entry()?),
        "children" => tree(node.children()?),
        "label" => encoded(node.label()?),
        "labelTimestamp" => encoded(node.label_timestamp()?),
        _ => Err("unknown node query".into()),
    }
}
/// Traverse each returned node once, emitting a reversible flat preorder table.
fn tree(roots: Vec<SessionTreeNode>) -> ExtensionResult<Value> {
    let mut pending: Vec<(SessionTreeNode, Option<usize>)> =
        roots.into_iter().rev().map(|node| (node, None)).collect();
    let mut roots = Vec::new();
    let mut nodes: Vec<Value> = Vec::new();
    while let Some((node, parent)) = pending.pop() {
        let index = nodes.len();
        if let Some(parent) = parent {
            nodes[parent]["children"]
                .as_array_mut()
                .ok_or("missing child output list")?
                .push(json!(index));
        } else {
            roots.push(index);
        }
        let mut value = json!({"entry":encoded(node.entry()?)?,"children":[]});
        if let Some(label) = node.label()? {
            value["label"] = json!(label);
        }
        if let Some(timestamp) = node.label_timestamp()? {
            value["labelTimestamp"] = json!(timestamp);
        }
        nodes.push(value);
        pending.extend(
            node.children()?
                .into_iter()
                .rev()
                .map(|node| (node, Some(index))),
        );
    }
    Ok(json!({"roots":roots,"nodes":nodes}))
}
