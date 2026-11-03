//! The shipped summary command through the controlled compiled-component host.
use super::{host::Harness, observed::Observed};
use serde_json::{Value, json};

/// The existing compiled author host, serving controlled catalog and branch replies.
struct Host(Box<Harness>);
impl Host {
    /// Starts one copy of the shipped factory.
    fn new() -> Result<Self, String> {
        let path = super::build::example_component("summarize")?;
        Ok(Self(Box::new(
            super::block_on(Harness::start(&path)).map_err(|e| e.to_string())?,
        )))
    }
    /// Observes without holding a borrow across guest execution.
    fn observe<T>(&mut self, f: impl FnOnce(&mut Observed) -> T) -> T {
        f(self.0.observed())
    }
    /// Supplies one current-branch message and fixed catalog outcomes.
    fn supply(&mut self, text: &str, model: &Value, auth: &Value) {
        self.observe(|o| {
            o.catalog.data.has_ui = true;
            o.actions.value = json!({"is_whitespace_char":false});
            o.readers.control(
                &json!({"bind":{"replies":[{"method":"getBranch","args":[null],"value":[{
                    "type":"message","id":"one","parentId":null,"timestamp":"2026-01-01T00:00:00Z",
                    "message":{"role":"user","content":text,"timestamp":1}
                }]}]}})
                .to_string(),
            );
            o.catalog.control(
                &json!({"registry":{"replies":[
                    {"method":"find","args":["openai","gpt-5.2"],"value":model},
                    {"method":"auth","args":[model],"value":auth}
                ]}})
                .to_string(),
            );
        });
    }
    /// Invokes the registered command, not a private example function.
    fn command(&mut self) -> Result<(), String> {
        let key = self.observe(|o| o.callback("command summarize"))?;
        super::example_support::invoke_command_on_component(&mut self.0, key, "ignored", false)
    }
    /// Notification messages and severities in their arrival order.
    fn notifications(&mut self) -> Vec<Value> {
        self.observe(|o| {
            super::ui_host::receipts(&o.transcript)
                .into_iter()
                .filter(|r| r.method == "notify")
                .map(|r| r.operands)
                .collect()
        })
    }
}
#[test]
fn summarize_no_text_skips_catalog_and_completion() -> Result<(), String> {
    for text in ["", "\u{feff}\u{a0} "] {
        let mut host = Host::new()?;
        host.supply(text, &Value::Null, &Value::Null);
        host.observe(|o| o.actions.value = json!({"is_whitespace_char":true}));
        host.command()?;
        assert_eq!(
            host.notifications(),
            [json!(["No conversation text found", "Warning"])]
        );
        host.observe(|o| assert!(o.catalog.registry.queries().is_empty()));
        host.observe(|o| {
            o.catalog.data.has_ui = false;
            o.transcript.clear();
        });
        host.command()?;
        assert!(host.notifications().is_empty());
    }
    Ok(())
}

#[test]
fn summarize_reports_missing_model_and_auth_failures() -> Result<(), String> {
    let model = json!({"id":"gpt-5.2","name":"Summary model","provider":"openai","api":"openai-responses",
        "baseUrl":"https://example.invalid","reasoning":true,"input":["text"],
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128,"maxTokens":16});
    let model: crate::Model = serde_json::from_value(model).map_err(|e| e.to_string())?;
    let model = serde_json::to_value(model).map_err(|e| e.to_string())?;
    for (found, auth, warning) in [
        (Value::Null, Value::Null, "Model openai/gpt-5.2 not found"),
        (
            model.clone(),
            json!({"error":"controlled auth error"}),
            "controlled auth error",
        ),
        (model.clone(), json!({}), "No API key for openai/gpt-5.2"),
        (model, json!({"apiKey":""}), "No API key for openai/gpt-5.2"),
    ] {
        let mut host = Host::new()?;
        host.supply("hello", &found, &auth);
        host.command()?;
        assert_eq!(
            host.notifications(),
            [
                json!(["Preparing summary...", "Info"]),
                json!([warning, "Warning"])
            ]
        );
        host.observe(|o| {
            o.catalog.data.has_ui = false;
            o.transcript.clear();
        });
        host.command()?;
        assert!(host.notifications().is_empty());
    }
    Ok(())
}
