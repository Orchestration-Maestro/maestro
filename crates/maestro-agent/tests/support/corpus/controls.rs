//! Extend the existing operation replay with hook and queue operands.
use super::*;

/// Serialized fixture mode, separate from SDK-only product records.
#[derive(Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Mode {
    /// One call at a time.
    Sequential,
    /// Concurrent finalization.
    Parallel,
}
impl From<Mode> for ToolExecutionMode {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Sequential => Self::Sequential,
            Mode::Parallel => Self::Parallel,
        }
    }
}
/// Strict operands captured by the complete-entrypoint oracle.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct Control {
    /// Consumed boolean fixture choices.
    #[serde(skip)]
    flags: BTreeMap<&'static str, bool>,
    /// Selected global mode.
    mode: Option<Mode>,
    /// Before-hook return, including an explicit null.
    before: Option<Value>,
    /// Preparation choice.
    prepare: Option<String>,
    /// Failing callback phase.
    failure: Option<String>,
    /// After-hook return.
    after: Option<Value>,
    /// Per-call termination flags.
    terminations: Option<Vec<Value>>,
    /// Call count.
    count: Option<usize>,
    /// Selected declaration mode.
    tool_mode: Option<Mode>,
    /// Later duplicate declaration mode.
    duplicate_mode: Option<Mode>,
    /// Steering batches.
    steering: Option<Vec<Vec<String>>>,
    /// Follow-up batches.
    follow: Option<Vec<Vec<String>>>,
    /// Stop hook outcome.
    stop: Option<bool>,
    /// Assistant outcome.
    stop_reason: Option<StopReason>,
    /// Failing control boundary.
    error_at: Option<String>,
    /// Invocation cancellation state.
    signal: Option<String>,
}
impl Control {
    /// Read an already-consumed fixture flag.
    fn flag(&self, name: &str) -> bool {
        self.flags[name]
    }
}
/// Build existing replay operands without relaxing unread-field checks.
fn input(mut fields: BTreeMap<String, Value>) -> Input {
    let id: String = take(&mut fields, "id").unwrap();
    // Optional JSON null remains distinct from an absent execute detail operand.
    let details = fields.remove("initialDetails");
    let flags = [
        "mutate",
        "invalid",
        "secondSequential",
        "missingFirst",
        "duplicateCalls",
        "continuation",
        "stringSchema",
        "omitControls",
    ]
    .into_iter()
    .map(|name| (name, take(&mut fields, name).unwrap_or(false)))
    .collect();
    let mut control: Control = serde_json::from_value(json!(fields)).unwrap();
    control.flags = flags;
    let before_present = fields.contains_key("before");
    let after_present = fields.contains_key("after");
    let mut base = BTreeMap::from([("id".into(), json!(id))]);
    base.insert("continue".into(), json!(control.flag("continuation")));
    base.insert("prompts".into(), json!("control"));
    base.insert("noSignal".into(), json!(control.signal.is_none()));
    base.insert(
        "abort".into(),
        json!(control.signal.as_deref() == Some("aborted")),
    );
    let mut input = Input::from(base);
    input.details = details;
    input.stop = control.stop_reason.clone().unwrap_or(StopReason::Stop);
    input.emit_error = if control.error_at.as_deref() == Some("turn-end") {
        "turn_end".into()
    } else {
        String::new()
    };
    input.tool_calls = Some(calls(&control));
    input.control = Some(Control {
        before: if before_present {
            Some(control.before.clone().unwrap_or(Value::Null))
        } else {
            None
        },
        after: if after_present {
            Some(control.after.clone().unwrap_or(Value::Null))
        } else {
            None
        },
        ..control
    });
    input
}
/// Construct distinguishable raw call records in their original order.
fn calls(control: &Control) -> Vec<AssistantContent> {
    (0..control.count.unwrap_or(1))
        .map(|index| {
            let id = if control.flag("duplicateCalls") && index == 2 {
                0
            } else {
                index
            };
            let name = if control.flag("missingFirst") && index == 0 {
                "missing"
            } else if control.flag("secondSequential") && index == 1 {
                "serial"
            } else {
                "echo"
            };
            let value =
                if control.flag("invalid") || control.failure.as_deref() == Some("validation") {
                    json!({"bad":true})
                } else {
                    json!((7 + index).to_string())
                };
            AssistantContent::ToolCall(ToolCall {
                id: format!("call-{id}"),
                name: name.into(),
                arguments: JsonObject::from_iter([("value".into(), value)]).into(),
                thought_signature: None,
            })
        })
        .collect()
}
/// Replay a disjoint group with every retained output field checked.
///
/// # Panics
/// Panics for unread operands or mismatched source observations.
pub async fn replay(ids: &[&str]) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../../fixtures/control_cases.json")).unwrap();
    let selected: Vec<_> = cases
        .into_iter()
        .filter(|case| ids.contains(&case.input["id"].as_str().unwrap()))
        .collect();
    assert_eq!(selected.len(), ids.len());
    for case in selected {
        let name = case.input["id"].as_str().unwrap().to_owned();
        let input = Arc::new(input(case.input));
        let control = input.control.as_ref().unwrap();
        let parallel = !matches!(control.mode, Some(Mode::Sequential))
            && !matches!(control.tool_mode, Some(Mode::Sequential))
            && !control.flag("secondSequential");
        let actual = execute(input).await;
        eprintln!("control case: {name}");
        compare_control(actual, case.expected, parallel);
    }
}
/// Install only the callbacks requested by the recorded operands.
pub(super) fn install(
    config: &mut AgentLoopConfig<Notice>,
    input: &Arc<Input>,
    observed: Arc<Mutex<Observations>>,
) {
    let Some(control) = &input.control else {
        return;
    };
    config.tool_execution = control.mode.map(Into::into);
    config.convert_to_llm =
        Arc::new(|messages| ready(Ok(messages.read().unwrap().iter().map(convert).collect())));
    install_before(config, Arc::clone(input), Arc::clone(&observed));
    install_after(config, Arc::clone(input), Arc::clone(&observed));
    if !control.flag("omitControls") {
        config.get_steering_messages = Some(queue(Arc::clone(input), Arc::clone(&observed), false));
        config.get_follow_up_messages = Some(queue(Arc::clone(input), Arc::clone(&observed), true));
        config.should_stop_after_turn = Some(Arc::new({
            let input = Arc::clone(input);
            move |c| {
                let control = input.control.as_ref().unwrap();
                let hook = json!({"phase":"stop","message":&*c.message.read().unwrap(),"toolResults":c.tool_results,"history":c.context.messages.read().unwrap().iter().map(message).collect::<Vec<_>>(),"newMessages":c.new_messages.iter().map(message).collect::<Vec<_>>()});
                record(&observed, "stop".into(), hook);
                Box::pin(std::future::ready(stop_result(control)))
            }
        }));
    }
}
/// Record a callback snapshot after releasing all retained record guards.
fn record(observed: &Mutex<Observations>, trace: String, hook: Value) {
    let mut observed = observed.lock().unwrap();
    observed.trace.push(trace);
    observed.hooks.push(hook);
}
/// Await ordered queue operands and propagate their declared failure boundary.
fn queue(
    input: Arc<Input>,
    observed: Arc<Mutex<Observations>>,
    follow: bool,
) -> GetSteeringMessages<Notice> {
    let count = Mutex::new(0);
    Arc::new(move || {
        let index = {
            let mut count = count.lock().unwrap();
            let index = *count;
            *count += 1;
            index
        };
        let control = input.control.as_ref().unwrap();
        let kind = if follow { "follow" } else { "steering" };
        observed
            .lock()
            .unwrap()
            .trace
            .push(format!("{kind}:{index}"));
        let boundary = if follow {
            "follow"
        } else if index == 0 {
            "initial-steering"
        } else {
            "post-steering"
        };
        if control.error_at.as_deref() == Some(boundary) {
            return ready(Err(error(&format!("{kind} failed"))));
        }
        let batches = if follow {
            &control.follow
        } else {
            &control.steering
        };
        ready(Ok(batches
            .as_ref()
            .and_then(|batches| batches.get(index))
            .map_or_else(Vec::new, |batch| {
                batch.iter().map(|text| entry(text)).collect()
            })))
    })
}
/// Install a borrowed mutable argument hook after model-owned validation.
fn install_before(
    config: &mut AgentLoopConfig<Notice>,
    input: Arc<Input>,
    observed: Arc<Mutex<Observations>>,
) {
    let control = input.control.as_ref().unwrap();
    if control.before.is_none()
        && !control.flag("mutate")
        && control.failure.as_deref() != Some("before")
    {
        return;
    }
    config.before_tool_call = Some(Arc::new(move |c, signal| {
        let control = input.control.as_ref().unwrap();
        let hook = json!({"phase":"before","id":c.tool_call.id,"args":c.args,"raw":c.tool_call.arguments,"history":c.context.messages.read().unwrap().iter().map(message).collect::<Vec<_>>(),"assistant":&*c.assistant_message.read().unwrap(),"signalSame":signal.is_some() == control.signal.is_some()});
        record(&observed, format!("before:{}", c.tool_call.id), hook);
        let edited = if control.flag("stringSchema") {
            json!(123)
        } else {
            json!("not an integer")
        };
        let input = Arc::clone(&input);
        Box::pin(async move {
            let control = input.control.as_ref().unwrap();
            if control.failure.as_deref() == Some("before") {
                return Err(error("before failed"));
            }
            if control.flag("mutate") {
                c.args["value"] = edited;
            }
            Ok(control
                .before
                .as_ref()
                .filter(|value| !value.is_null())
                .map(|value| BeforeToolCallResult {
                    block: value["block"].as_bool().unwrap_or(false),
                    reason: value["reason"].as_str().map(str::to_owned),
                }))
        })
    }));
}
/// Decode only source-admitted SDK return fields at the fixture boundary.
fn install_after(
    config: &mut AgentLoopConfig<Notice>,
    input: Arc<Input>,
    observed: Arc<Mutex<Observations>>,
) {
    let control = input.control.as_ref().unwrap();
    if control.after.is_none() && control.failure.as_deref() != Some("after") {
        return;
    }
    config.after_tool_call = Some(Arc::new(move |c, signal| {
        let control = input.control.as_ref().unwrap();
        let hook = json!({"phase":"after","id":c.tool_call.id,"args":c.args,"result":tool_result(c.result),"isError":c.is_error,"history":c.context.messages.read().unwrap().iter().map(message).collect::<Vec<_>>(),"signalSame":signal.is_some() == control.signal.is_some()});
        record(&observed, format!("after:{}", c.tool_call.id), hook);
        let result = if control.failure.as_deref() == Some("after") {
            Err(error("after failed"))
        } else {
            Ok(control
                .after
                .as_ref()
                .filter(|value| !value.is_null())
                .map(|value| AfterToolCallResult {
                    content: value
                        .get("content")
                        .filter(|v| !v.is_null())
                        .map(|v| serde_json::from_value(v.clone()).unwrap()),
                    details: value.get("details").cloned(),
                    is_error: value["isError"].as_bool(),
                    terminate: value["terminate"].as_bool(),
                }))
        };
        Box::pin(std::future::ready(result))
    }));
}
/// Select source declarations while retaining the existing execution harness.
pub(super) fn tools(
    context: &mut AgentContext<Notice>,
    input: &Arc<Input>,
    observed: Arc<Mutex<Observations>>,
) {
    let control = input.control.as_ref().unwrap();
    let selected = tool(Arc::clone(input), observed);
    selected.write().unwrap().execution_mode = control.tool_mode.map(Into::into);
    let mut inventory = vec![selected];
    if let Some(mode) = control.duplicate_mode {
        let duplicate = tool(
            Arc::clone(input),
            Arc::new(Mutex::new(Observations::default())),
        );
        duplicate.write().unwrap().execution_mode = Some(mode.into());
        duplicate.write().unwrap().execute =
            Arc::new(|_, _, _, _| ready(Err(error("wrong duplicate"))));
        inventory.push(duplicate);
    }
    if control.flag("secondSequential") {
        let serial = tool(
            Arc::clone(input),
            Arc::new(Mutex::new(Observations::default())),
        );
        // Both selected declarations report into the same operation observation set.
        serial.write().unwrap().execute = Arc::clone(&inventory[0].read().unwrap().execute);
        serial.write().unwrap().definition.name = "serial".into();
        serial.write().unwrap().execution_mode = Some(ToolExecutionMode::Sequential);
        inventory.push(serial);
    }
    context.tools = Some(Arc::new(RwLock::new(inventory)));
}
/// Build preparation and execution callbacks consumed by the existing tool path.
fn tool(input: Arc<Input>, observed: Arc<Mutex<Observations>>) -> SharedAgentTool {
    let control = input.control.as_ref().unwrap();
    let preparation = (control.prepare.is_some() || control.failure.as_deref() == Some("prepare"))
        .then(|| {
            let input = Arc::clone(&input);
            let observed = Arc::clone(&observed);
            Arc::new(move |_: &mut Value| {
                observed.lock().unwrap().trace.push("prepare".into());
                let control = input.control.as_ref().unwrap();
                if control.prepare.as_deref() == Some("error")
                    || control.failure.as_deref() == Some("prepare")
                {
                    Err(error("prepare failed"))
                } else {
                    Ok(json!({"value":"8"}))
                }
            }) as PrepareArguments
        });
    Arc::new(RwLock::new(AgentTool {
        description: None,
        definition: Tool {
            name: "echo".into(),
            description: "Echo".into(),
            parameters: json!({"type":"object","properties":{"value":{"type":if control.flag("stringSchema") { "string" } else { "integer" }}},"required":["value"]}).into(),
        },
        label: "Echo".into(),
        prepare_arguments: preparation,
        execution_mode: None,
        execute: Arc::new(move |id, args, signal, _| {
            let control = input.control.as_ref().unwrap();
            record(
                &observed,
                format!("execute:{id}"),
                json!({"phase":"execute","id":id,"args":args,"signalSame":signal.is_some() == control.signal.is_some(),"aborted":signal.as_ref().is_some_and(Cancellation::is_aborted)}),
            );
            if control.failure.as_deref() == Some("execute") {
                return ready(Err(error("execute failed")));
            }
            let index: usize = id.strip_prefix("call-").unwrap().parse().unwrap();
            let terminate = control
                .terminations
                .as_ref()
                .map_or(Some(true), |values| values[index].as_bool());
            ready(Ok(AgentToolResult { content: serde_json::from_value(json!([{"type":"text","text":format!("result:{id}")},{"type":"image","data":"AA==","mimeType":"image/png"}])).unwrap(), details: Some(input.details.clone().unwrap_or_else(|| json!({"z":2,"a":1}))), terminate }))
        }),
    }))
}

/// Compare per-call phases without requiring same-turn microtask interleaving.
fn compare_control(mut actual: Value, mut expected: Value, parallel: bool) {
    if !parallel {
        compare(&normalized(actual), &normalized(expected));
        return;
    }
    let trace = actual["trace"].as_array().unwrap();
    let executed: Vec<_> = trace
        .iter()
        .enumerate()
        .filter_map(|(i, v)| {
            v.as_str()
                .unwrap()
                .strip_prefix("execute:")
                .map(|id| (i, id))
        })
        .collect();
    for (index, id) in &executed {
        for (_, other) in &executed {
            let end = trace
                .iter()
                .position(|v| v.as_str() == Some(&format!("tool_execution_end:{other}")))
                .unwrap();
            assert!(
                *index < end,
                "all execute callbacks enter before allowed ends"
            );
        }
        assert!(!id.is_empty());
    }
    for value in [&mut actual, &mut expected] {
        let trace = value["trace"].as_array().unwrap();
        let executed: Vec<_> = trace
            .iter()
            .filter_map(|v| v.as_str().unwrap().strip_prefix("execute:"))
            .collect();
        let mut phases = JsonObject::new();
        for id in executed {
            let sequence: Vec<_> = trace
                .iter()
                .filter(|v| {
                    let text = v.as_str().unwrap();
                    ["execute:", "after:", "tool_execution_end:"]
                        .iter()
                        .any(|prefix| text == format!("{prefix}{id}"))
                })
                .cloned()
                .collect();
            phases.insert(id.into(), json!(sequence));
        }
        value["callPhases"] = json!(phases);
        let trace = value["trace"].as_array_mut().unwrap();
        trace.retain(|v| {
            let text = v.as_str().unwrap();
            !text.starts_with("after:") && !text.starts_with("tool_execution_end:")
        });
    }
    compare(&normalized(actual), &normalized(expected));
}

/// Resolve the stop operand at its callback boundary.
fn stop_result(control: &Control) -> Result<bool, DiagnosticErrorInfo> {
    if control.error_at.as_deref() == Some("stop") {
        Err(error("stop failed"))
    } else {
        Ok(control.stop.unwrap_or(false))
    }
}
