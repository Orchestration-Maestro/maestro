//! Native host bindings for retained browser assertions.

use super::{
    cleanup,
    page::{Context, Page, string},
    scripts,
};
use chromiumoxide::{
    keys::{KeyDefinition, get_key_definition},
    types::CdpJsonEventMessage,
};
use serde_json::{Value, json};
use std::{collections::HashMap, io};

impl Page<'_> {
    /// Resolve the host call in the exact context that emitted it.
    pub(super) async fn binding(&mut self, event: &CdpJsonEventMessage) -> io::Result<()> {
        if event.params["name"] != "__maestroDriver" {
            return Ok(());
        }
        let payload: Value =
            serde_json::from_str(string(&event.params, "payload")?).map_err(io::Error::other)?;
        let Some(context) = self
            .contexts
            .values()
            .find(|context| {
                Some(context.id) == event.params["executionContextId"].as_i64()
                    && Some(context.session.as_ref()) == event.session_id.as_deref()
            })
            .cloned()
        else {
            // A call from a removed renderer has no caller left to answer.
            return Ok(());
        };
        let result = self.invoke(&context, &payload).await;
        let (value, error) = match &result {
            Ok(value) => (value.clone(), Value::Null),
            Err(error) => (Value::Null, json!(error.to_string())),
        };
        let expression = format!("__maestroReply({},{value},{error})", payload["id"]);
        let replied = self
            .evaluate_in(&context, &expression, true)
            .await
            .map(|_| ());
        // A caller removed while its call ran cannot receive the reply, whatever the outcome.
        if !self.context_exists(&context.session, context.id) {
            return Ok(());
        }
        // Rejected callbacks settle the awaiting export.
        match (result, replied) {
            (Ok(_), replied) => replied,
            (Err(_), Ok(())) => Ok(()),
            (Err(error), Err(reply)) => cleanup(Err(error), Err(reply)),
        }
    }

    /// Dispatch only the bindings the browser input contract defines.
    async fn invoke(&mut self, context: &Context, payload: &Value) -> io::Result<Value> {
        let argument = payload["args"][0]
            .as_str()
            .ok_or_else(|| io::Error::other("binding argument must be a string"))?;
        let name = string(payload, "name")?;
        if name == "sandboxDriverPhase" {
            self.phase(&format!(
                "{argument}; frame {}",
                self.urls
                    .get(&context.frame)
                    .map_or("about:blank", String::as_str)
            ));
            return Ok(Value::Null);
        }
        let root = self.root_context()?;
        match name {
            "inspectSandbox" => {
                self.inspect(
                    &root,
                    argument,
                    payload["args"][1].as_str().unwrap_or("state"),
                )
                .await
            }
            "driverPress" => {
                self.press(&root, argument).await?;
                Ok(Value::Null)
            }
            "driverPointer" => {
                let point: (f64, f64, bool) =
                    serde_json::from_str(argument).map_err(io::Error::other)?;
                self.pointer(&root, (point.0, point.1), point.2).await?;
                Ok(Value::Null)
            }
            "driverAria" => self.aria(&root, argument).await.map(Value::String),
            other => Err(io::Error::other(format!("unknown browser binding {other}"))),
        }
    }

    /// Find the mounted frame by native ownership and release its lookup handle on every outcome.
    async fn selected(&mut self, context: &Context, id: &str) -> io::Result<Context> {
        let expression = format!("({})({})", scripts::LOOKUP, json!(id));
        let object = self.evaluate_in(context, &expression, false).await?;
        let handle = object
            .get("objectId")
            .and_then(Value::as_str)
            .ok_or_else(|| io::Error::other(format!("mounted frame not found {id}")))?;
        let result = self
            .command(
                Some(&context.session),
                "DOM.describeNode",
                json!({"objectId":handle}),
            )
            .await;
        let released = self
            .command(
                Some(&context.session),
                "Runtime.releaseObject",
                json!({"objectId":handle}),
            )
            .await
            .map(|_| ());
        let node = cleanup(result, released)?;
        let frame = string(&node["node"], "frameId")?;
        self.contexts
            .get(frame)
            .cloned()
            .ok_or_else(|| io::Error::other(format!("mounted frame removed {id}")))
    }

    /// Observe retained sandbox state or send input directly to the selected renderer.
    async fn inspect(&mut self, caller: &Context, id: &str, action: &str) -> io::Result<Value> {
        let selected = self.selected(caller, id).await?;
        if action != "click" {
            return self.evaluate_in(&selected, scripts::STATE, true).await;
        }
        let href = self.evaluate_in(&selected, "location.href", true).await?;
        self.phase(&format!(
            "waiting for pointer-done {id}; frame {}",
            href.as_str().unwrap_or("")
        ));
        self.evaluate_in(&selected, "window.pointerDone = null", true)
            .await?;
        self.click(&selected, scripts::TARGET).await?;
        let delivered = self
            .evaluate_in(&selected, "window.pointerDone", true)
            .await?;
        if delivered["trusted"] != true || delivered["atTarget"] != true {
            return Err(io::Error::other(format!(
                "native click did not reach mounted target {id}"
            )));
        }
        self.phase(&format!("click returned {id}"));
        Ok(Value::Null)
    }

    /// Dispatch a native click at the element's renderer-relative content quad.
    pub(super) async fn click(&mut self, context: &Context, expression: &str) -> io::Result<()> {
        let object = self.evaluate_in(context, expression, false).await?;
        let handle = string(&object, "objectId")?;
        let result = async {
            let quads = self
                .command(
                    Some(&context.session),
                    "DOM.getContentQuads",
                    json!({"objectId":handle}),
                )
                .await?;
            let quad = quads["quads"][0]
                .as_array()
                .filter(|values| values.len() == 8)
                .ok_or_else(|| io::Error::other("native click target has no visible quad"))?;
            let coordinates = quad
                .iter()
                .map(|value| {
                    value
                        .as_f64()
                        .ok_or_else(|| io::Error::other("invalid native content quad"))
                })
                .collect::<io::Result<Vec<_>>>()?;
            let x = coordinates.iter().step_by(2).sum::<f64>() / 4.0;
            let y = coordinates.iter().skip(1).step_by(2).sum::<f64>() / 4.0;
            self.pointer(context, (x, y), true).await
        }
        .await;
        let released = self
            .command(
                Some(&context.session),
                "Runtime.releaseObject",
                json!({"objectId":handle}),
            )
            .await
            .map(|_| ());
        cleanup(result, released)
    }

    /// Move or click with the selected session, never a top-page coordinate fallback.
    pub(super) async fn pointer(
        &mut self,
        context: &Context,
        point: (f64, f64),
        click: bool,
    ) -> io::Result<()> {
        let types: &[&str] = if click {
            &["mouseMoved", "mousePressed", "mouseReleased"]
        } else {
            &["mouseMoved"]
        };
        for kind in types {
            self.command(Some(&context.session), "Input.dispatchMouseEvent", json!({"type":kind,"x":point.0,"y":point.1,
                "button":if *kind == "mouseMoved" {"none"} else {"left"}, "buttons":i32::from(*kind == "mousePressed"),"clickCount":1})).await?;
        }
        Ok(())
    }

    /// Use the library's native key definitions, including configured modifier chords.
    async fn press(&mut self, context: &Context, chord: &str) -> io::Result<()> {
        let parts: Vec<_> = chord.split('+').collect();
        let (key, modifiers) = parts
            .split_last()
            .ok_or_else(|| io::Error::other("missing keyboard key"))?;
        let definition = key_definition(key)
            .ok_or_else(|| io::Error::other(format!("unknown keyboard key {key}")))?;
        let mut mask = 0;
        for modifier in modifiers {
            mask |= match *modifier {
                "Alt" => 1,
                "Control" => 2,
                "Meta" => 4,
                "Shift" => 8,
                _ => return Err(io::Error::other("unknown keyboard modifier")),
            };
        }
        let text = definition
            .text
            .or_else(|| (definition.key.chars().count() == 1).then_some(definition.key));
        for kind in ["keyDown", "keyUp"] {
            let mut params = json!({"type":kind,"key":definition.key,"code":definition.code,"windowsVirtualKeyCode":definition.key_code,"modifiers":mask});
            if kind == "keyDown"
                && mask == 0
                && let Some(text) = text
            {
                params["text"] = json!(text);
                params["unmodifiedText"] = json!(text);
            }
            self.command(Some(&context.session), "Input.dispatchKeyEvent", params)
                .await?;
        }
        Ok(())
    }

    /// Project native accessibility names and roles within the selected DOM subtree.
    async fn aria(&mut self, context: &Context, selector: &str) -> io::Result<String> {
        self.command(Some(&context.session), "Accessibility.enable", json!({}))
            .await?;
        let expression = format!("document.querySelector({})", json!(selector));
        let object = self.evaluate_in(context, &expression, false).await?;
        let handle = string(&object, "objectId")?;
        let result = self
            .command(
                Some(&context.session),
                "DOM.describeNode",
                json!({"objectId":handle}),
            )
            .await;
        let released = self
            .command(
                Some(&context.session),
                "Runtime.releaseObject",
                json!({"objectId":handle}),
            )
            .await
            .map(|_| ());
        let node = cleanup(result, released)?;
        let tree = self
            .command(
                Some(&context.session),
                "Accessibility.getFullAXTree",
                json!({"frameId":context.frame}),
            )
            .await?;
        let nodes = tree["nodes"]
            .as_array()
            .ok_or_else(|| io::Error::other("missing accessibility tree"))?;
        let root = nodes
            .iter()
            .find(|value| value["backendDOMNodeId"] == node["node"]["backendNodeId"])
            .ok_or_else(|| io::Error::other("selected element has no accessibility node"))?;
        let nodes: HashMap<_, _> = nodes
            .iter()
            .filter_map(|node| node["nodeId"].as_str().map(|id| (id, node)))
            .collect();
        let mut output = String::new();
        accessibility(root, &nodes, &mut output);
        Ok(output)
    }
}

/// Walk each native accessibility node once, flattening ignored containers.
fn accessibility(node: &Value, nodes: &HashMap<&str, &Value>, output: &mut String) {
    use std::fmt::Write;
    if node["ignored"] != true {
        let role = node["role"]["value"].as_str().unwrap_or("");
        let name = node["name"]["value"].as_str().unwrap_or("");
        // Writing into a String cannot fail.
        let _ = writeln!(output, "- {role} {}", json!(name));
    }
    if let Some(children) = node["childIds"].as_array() {
        for child in children {
            if let Some(node) = child.as_str().and_then(|id| nodes.get(id)) {
                accessibility(node, nodes, output);
            }
        }
    }
}

/// Key names the browser input protocol resolves beyond the library's own US table, each carrying
/// the key, code, key code and typed text the dispatcher expects.
const EXTRA_KEYS: [(&str, KeyDefinition); 1] = [(
    "Space",
    KeyDefinition {
        key: " ",
        key_code: 32,
        code: "Space",
        text: Some(" "),
    },
)];

/// Resolve one authored key name into a real key definition.
fn key_definition(name: &str) -> Option<&'static KeyDefinition> {
    get_key_definition(name).or_else(|| {
        EXTRA_KEYS
            .iter()
            .find(|(extra, _)| *extra == name)
            .map(|(_, definition)| definition)
    })
}

#[cfg(test)]
mod tests;
