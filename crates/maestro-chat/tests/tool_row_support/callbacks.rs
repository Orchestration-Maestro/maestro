//! Controlled callbacks capture the complete source-visible context and component identity.
use super::records::Input;
use maestro_app::{ToolRenderContext, ToolRenderShell, ToolRendererBinding, ToolRendererState};
use maestro_tui::{Container, Text, tui::ComponentHandle};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
/// Shared callbacks and their ordered transcript.
#[derive(Default)]
pub struct Callbacks {
    pub events: RefCell<Vec<Value>>,
    handles: RefCell<Vec<ComponentHandle>>,
    texts: RefCell<HashMap<usize, Rc<Text>>>,
    invalidators: RefCell<HashMap<String, Rc<dyn Fn()>>>,
}
impl Callbacks {
    /// Preserve source component identities as monotonically assigned labels.
    fn id(&self, handle: Option<&ComponentHandle>) -> Option<usize> {
        handle.map(|handle| {
            let mut handles = self.handles.borrow_mut();
            if let Some(index) = handles.iter().position(|h| Rc::ptr_eq(h, handle)) {
                index + 1
            } else {
                handles.push(handle.clone());
                handles.len()
            }
        })
    }
    /// Invoke the retained public invalidator after releasing its collection borrow.
    pub fn invalidate(&self, slot: &str) {
        let callback = self.invalidators.borrow()[slot].clone();
        callback();
    }
    /// Bind the two slots independently; renderer projections carry no parameter schema.
    pub fn binding(self: &Rc<Self>, input: &Input) -> Option<ToolRendererBinding> {
        let def = input.definition.as_ref()?;
        let call = def.call.as_ref().map(|kind| {
            let owner = self.clone();
            let kind = kind.clone();
            Rc::new(
                move |_: Option<&Value>,
                      _: &maestro_theme::Theme,
                      context: &ToolRenderContext<'_, ToolRendererState>| {
                    owner.call("call", &kind, context, None)
                },
            ) as maestro_app::ToolRenderCall<ToolRendererState>
        });
        let result = def.result.as_ref().map(|kind| {
            let owner = self.clone();
            let kind = kind.clone();
            Rc::new(
                move |result: &maestro_app::ToolRenderResultLike<'_, Value>,
                      options: maestro_app::ToolRenderResultOptions,
                      _: &maestro_theme::Theme,
                      context: &ToolRenderContext<'_, ToolRendererState>| {
                    let value = result_value(result);
                    owner.call(
                        "result",
                        &kind,
                        context,
                        Some((
                            value,
                            json!({"expanded":options.expanded,"isPartial":options.is_partial}),
                        )),
                    )
                },
            ) as maestro_app::ToolRenderResultCallback<Value, ToolRendererState>
        });
        Some(ToolRendererBinding {
            render_call: call,
            render_result: result,
            render_shell: def.shell.as_deref().map(|s| match s {
                "default" => ToolRenderShell::Default,
                "self" => ToolRenderShell::SelfRendered,
                _ => panic!("unknown shell"),
            }),
        })
    }
    /// Record before executing stateful behavior, matching the source callback boundary.
    fn call(
        &self,
        slot: &str,
        kind: &str,
        c: &ToolRenderContext<'_, ToolRendererState>,
        result: Option<(Value, Value)>,
    ) -> Result<ComponentHandle, maestro_app::DiagnosticErrorInfo> {
        self.invalidators
            .borrow_mut()
            .insert(slot.into(), c.invalidate.clone());
        let state = c.state.borrow().state::<Value>();
        let mut event = json!({"slot":slot,"id":c.tool_call_id,"cwd":c.cwd,"executionStarted":c.phase.execution_started,"argsComplete":c.phase.args_complete,"isPartial":c.phase.is_partial,"expanded":c.expanded,"showImages":c.show_images,"isError":c.is_error,"last":self.id(c.last_component.as_ref()),"stateToken":state.borrow().get("token").cloned().unwrap_or(Value::Null)});
        if let Some(args) = c.args {
            event["args"] = args.clone();
        }
        if let Some((result, options)) = result {
            event["result"] = result;
            event["options"] = options;
        }
        self.events.borrow_mut().push(event);
        let count = self
            .events
            .borrow()
            .iter()
            .filter(|e| e["slot"] == slot)
            .count();
        if kind == "throw" || (kind == "recover" && count == 2) {
            return Err(maestro_app::DiagnosticErrorInfo::error("renderer refused"));
        }
        if kind == "empty" {
            return Ok(Rc::new(Container::new()));
        }
        let text = text(slot, kind, c, &state);
        let reuse = matches!(kind, "reuse" | "recover");
        if reuse && let Some(id) = self.id(c.last_component.as_ref()) {
            let item = self.texts.borrow()[&id].clone();
            item.set_text(text);
            return Ok(item);
        }
        let item = Rc::new(Text::new(text, 0, 0, None));
        if reuse {
            let handle: ComponentHandle = item.clone();
            let id = self.id(Some(&handle)).unwrap();
            self.texts.borrow_mut().insert(id, item.clone());
        }
        Ok(item)
    }
}

/// Preserve raw content/details presence in the callback transcript.
fn result_value(result: &maestro_app::ToolRenderResultLike<'_, Value>) -> Value {
    let mut value = json!({"content":result.content});
    if let Some(details) = result.details {
        value["details"] = details.clone();
    }
    value
}
/// Source callback output, separate from recording and handle identity.
fn text(
    slot: &str,
    kind: &str,
    c: &ToolRenderContext<'_, ToolRendererState>,
    state: &RefCell<Value>,
) -> String {
    match kind {
        "state" => {
            if state.borrow().get("token").is_none() {
                state.borrow_mut()["token"] = json!("shared-token");
            }
            format!("{slot} {}", state.borrow()["token"].as_str().unwrap())
        }
        "context" => {
            if state.borrow().get("token").is_none() {
                state.borrow_mut()["token"] = json!("retained");
            }
            let mut v = json!({});
            if let Some(args) = c.args {
                v["args"] = args.clone();
            }
            v["phase"] = json!([
                c.phase.execution_started,
                c.phase.args_complete,
                c.phase.is_partial,
                c.expanded,
                c.show_images,
                c.is_error
            ]);
            format!("{slot}:{}", maestro_app::compact_json(&v).unwrap())
        }
        "reuse" | "recover" => format!(
            "{slot} {}",
            c.args.map_or_else(
                || "undefined".into(),
                |v| maestro_app::compact_json(v).unwrap()
            )
        ),
        "style" => "\x1b[7m   \x1b[27m".into(),
        _ => kind.into(),
    }
}
