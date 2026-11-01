//! Literal writer observations shared by the two host adapters.
use super::{
    observed::Observed,
    reader_host::{Reader, Reply},
};
use serde::Deserialize;
use serde_json::{Value, json};
/// Controlled results for a writable capability.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Bank {
    /// Result selected by the host.
    pub value: Value,
    /// Selected raw record, when supplied.
    pub raw: Option<String>,
    /// Host rejection before a writer effect.
    pub error: Option<String>,
    /// Whether the host cancels without admitting callbacks.
    pub cancelled: bool,
    /// Whether the host leaves supplied callbacks uninvoked.
    pub unused: bool,
    /// Record the typed command operands before the host settles.
    pub record_options: Option<bool>,
    /// Keep the host operation pending before selecting its result.
    pub hold_return: bool,
    /// Host-level command rejection before admitting callbacks.
    pub session_error: Option<String>,
}
/// Record exactly one synchronous import, then supply its injected reply.
pub fn call(
    observed: &mut Observed,
    reader: &Reader,
    method: &str,
    args: &Value,
) -> Result<Value, String> {
    if let Some(error) = &observed.setup.error {
        return Err(error.clone());
    }
    observed.log(format!("writer {method} {args}"));
    if method == "append_session_info" {
        let mut data = reader.0.lock().unwrap();
        data.replies.retain(|r| r.method != "getSessionName");
        data.replies.push(Reply {
            method: "getSessionName".into(),
            args: vec![],
            value: args[0].clone(),
            raw: None,
            error: None,
            once: false,
        });
    }
    Ok(observed.setup.value.clone())
}
/// Optional document keeps omission separate from explicit JSON null.
pub fn document(value: Option<&str>) -> Value {
    super::action_host::document(value)
}
/// A new host session with a deliberately distinct name.
pub fn reader(name: &str) -> Reader {
    let reader = Reader::default();
    reader.0.lock().unwrap().replies.push(Reply {
        method: "getSessionName".into(),
        args: vec![],
        value: json!(name),
        raw: None,
        error: None,
        once: false,
    });
    reader
}

use super::host::State;
use crate::bindings::host_side::maestro::extension::host;
use wasmtime::component::Resource;
impl host::HostSessionManager for State {
    fn reader(&mut self, this: Resource<Reader>) -> wasmtime::Result<Resource<Reader>> {
        let reader = self.table.get(&this)?.clone();
        self.push(reader)
    }
    fn drop(&mut self, this: Resource<Reader>) -> wasmtime::Result<()> {
        self.release(this)
    }
    fn set_session_file(
        &mut self,
        this: Resource<Reader>,
        path: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "set_session_file",
            &json!([path]),
        );
        Ok(result.map(|_value| ()))
    }
    fn new_session(
        &mut self,
        this: Resource<Reader>,
        options: Option<String>,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "new_session",
            &json!([document(options.as_deref())]),
        );
        Ok(result.map(|value| value.as_str().map(str::to_owned)))
    }
    fn is_persisted(&mut self, this: Resource<Reader>) -> wasmtime::Result<Result<bool, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(&mut self.observed, &reader, "is_persisted", &json!([]));
        Ok(result.map(|value| value.as_bool().unwrap()))
    }
    fn get_children(
        &mut self,
        this: Resource<Reader>,
        parent_id: String,
    ) -> wasmtime::Result<Result<String, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "get_children",
            &json!([parent_id]),
        );
        Ok(result.map(|value| {
            self.observed
                .setup
                .raw
                .clone()
                .unwrap_or_else(|| value.to_string())
        }))
    }
    fn build_session_context(
        &mut self,
        this: Resource<Reader>,
    ) -> wasmtime::Result<Result<String, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "build_session_context",
            &json!([]),
        );
        Ok(result.map(|value| {
            self.observed
                .setup
                .raw
                .clone()
                .unwrap_or_else(|| value.to_string())
        }))
    }
    fn append_message(
        &mut self,
        this: Resource<Reader>,
        message: String,
    ) -> wasmtime::Result<Result<String, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "append_message",
            &json!([document(Some(&message))]),
        );
        Ok(result.map(|value| value.as_str().unwrap().to_owned()))
    }
    fn append_thinking_level_change(
        &mut self,
        this: Resource<Reader>,
        level: String,
    ) -> wasmtime::Result<Result<String, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "append_thinking_level_change",
            &json!([level]),
        );
        Ok(result.map(|value| value.as_str().unwrap().to_owned()))
    }
    fn append_model_change(
        &mut self,
        this: Resource<Reader>,
        provider: String,
        model_id: String,
    ) -> wasmtime::Result<Result<String, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "append_model_change",
            &json!([provider, model_id]),
        );
        Ok(result.map(|value| value.as_str().unwrap().to_owned()))
    }
    fn append_compaction(
        &mut self,
        this: Resource<Reader>,
        compaction: String,
        from_hook: Option<bool>,
    ) -> wasmtime::Result<Result<String, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "append_compaction",
            &json!([document(Some(&compaction)), from_hook]),
        );
        Ok(result.map(|value| value.as_str().unwrap().to_owned()))
    }
    fn append_custom_entry(
        &mut self,
        this: Resource<Reader>,
        custom_type: String,
        data: Option<String>,
    ) -> wasmtime::Result<Result<String, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "append_custom_entry",
            &json!([custom_type, document(data.as_deref())]),
        );
        Ok(result.map(|value| value.as_str().unwrap().to_owned()))
    }
    fn append_session_info(
        &mut self,
        this: Resource<Reader>,
        name: String,
    ) -> wasmtime::Result<Result<String, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "append_session_info",
            &json!([name]),
        );
        Ok(result.map(|value| value.as_str().unwrap().to_owned()))
    }
    fn append_custom_message_entry(
        &mut self,
        this: Resource<Reader>,
        message: String,
    ) -> wasmtime::Result<Result<String, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "append_custom_message_entry",
            &json!([document(Some(&message))]),
        );
        Ok(result.map(|value| value.as_str().unwrap().to_owned()))
    }
    fn append_label_change(
        &mut self,
        this: Resource<Reader>,
        target_id: String,
        label: Option<String>,
    ) -> wasmtime::Result<Result<String, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "append_label_change",
            &json!([target_id, label]),
        );
        Ok(result.map(|value| value.as_str().unwrap().to_owned()))
    }
    fn branch(
        &mut self,
        this: Resource<Reader>,
        from_id: String,
    ) -> wasmtime::Result<Result<(), String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(&mut self.observed, &reader, "branch", &json!([from_id]));
        Ok(result.map(|_value| ()))
    }
    fn reset_leaf(&mut self, this: Resource<Reader>) -> wasmtime::Result<Result<(), String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(&mut self.observed, &reader, "reset_leaf", &json!([]));
        Ok(result.map(|_value| ()))
    }
    fn branch_with_summary(
        &mut self,
        this: Resource<Reader>,
        from_id: Option<String>,
        summary: String,
        metadata: (Option<String>, Option<bool>),
    ) -> wasmtime::Result<Result<String, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "branch_with_summary",
            &json!([
                from_id,
                summary,
                document(metadata.0.as_deref()),
                metadata.1
            ]),
        );
        Ok(result.map(|value| value.as_str().unwrap().to_owned()))
    }
    fn create_branched_session(
        &mut self,
        this: Resource<Reader>,
        leaf_id: String,
    ) -> wasmtime::Result<Result<Option<String>, String>> {
        let reader = self.table.get(&this)?.clone();
        let result = call(
            &mut self.observed,
            &reader,
            "create_branched_session",
            &json!([leaf_id]),
        );
        Ok(result.map(|value| value.as_str().map(str::to_owned)))
    }
}

/// Record literal command operands and expose a named host-settlement barrier.
pub fn command_start(
    observed: &mut Observed,
    parent: Option<&str>,
    callbacks: (bool, bool),
    present: bool,
) -> Option<super::observed::Hold> {
    if observed.setup.record_options == Some(true) {
        observed.log(format!(
            "setup-options {}",
            json!({"present":present,"parent":parent,"setup":callbacks.0,"continuation":callbacks.1})
        ));
    }
    if observed.setup.hold_return {
        observed.log("setup host pending");
        observed.take_hold()
    } else {
        None
    }
}
