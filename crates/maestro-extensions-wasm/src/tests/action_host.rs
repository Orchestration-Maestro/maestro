//! Host decisions and literal action observations shared by both adapters.
use serde::Deserialize;
use serde_json::{Value, json};
/// A supplied action response and rejection.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Bank {
    /// Selected host reply.
    pub value: Value,
    /// Optional raw wire record.
    pub raw: Option<String>,
    /// Exact helper operands mapped to supplied replies.
    pub helpers: std::collections::HashMap<String, Value>,
    /// Rejection before action effects.
    pub error: Option<String>,
    /// Rejects the next session entry before recording it.
    pub entry_error: Option<String>,
}
impl Bank {
    /// Replace host state through an acknowledged entry.
    pub fn control(&mut self, text: &str) {
        *self = serde_json::from_str(text).unwrap();
    }
    /// Return a recoverable host decision before logging effects.
    pub fn check(&self) -> Result<(), String> {
        self.error.clone().map_or(Ok(()), Err)
    }
    /// Selects an operation's canned reply without implementing that helper.
    pub fn record_for(&self, operation: &str) -> Result<String, String> {
        self.check()?;
        Ok(self
            .raw
            .clone()
            .unwrap_or_else(|| self.value.get(operation).unwrap_or(&self.value).to_string()))
    }
    /// Supplies an exact helper reply before falling back to operation replies.
    pub fn helper(&self, operation: &str, data: &str) -> Result<String, String> {
        self.check()?;
        let key = format!("{operation} {data}");
        if let Some(value) = self.helpers.get(&key) {
            Ok(value.to_string())
        } else if self.value.get(&key).is_some() {
            self.record_for(&key)
        } else {
            self.record_for(operation)
        }
    }
    /// Reads a supplied boolean while preserving literal host refusal messages.
    pub fn boolean(&self, operation: &str) -> Result<bool, String> {
        self.check()?;
        let value = self.value.get(operation).unwrap_or(&self.value);
        value
            .as_bool()
            .ok_or_else(|| value.as_str().unwrap_or("missing boolean reply").to_owned())
    }
    /// Supply raw selected record input without changing decoder policy.
    pub fn record(&self) -> Result<String, String> {
        self.check()?;
        Ok(self.raw.clone().unwrap_or_else(|| self.value.to_string()))
    }
}
/// Decode optional wire documents while retaining absence independently of null.
pub fn document(value: Option<&str>) -> Value {
    value.map_or(
        json!({"present":false}),
        |s| json!({"present":true,"value":serde_json::from_str::<Value>(s).unwrap()}),
    )
}
/// Observe one admitted action, preserving its order and optional operands.
pub fn action(
    observed: &mut super::observed::Observed,
    method: &str,
    args: &Value,
) -> Result<(), String> {
    observed.actions.check()?;
    observed.log(format!("action {method} {args}"));
    Ok(())
}
/// Admit independently optional identities; notification is driven separately after return.
pub fn compact(
    observed: &mut super::observed::Observed,
    instructions: Option<&str>,
    complete: Option<u32>,
    error: Option<u32>,
    present: bool,
) -> Result<(), String> {
    observed.actions.check()?;
    let request = observed.compactions;
    observed.compactions += 1;
    observed.log(format!("compact admission {}",json!({"present":present,"instructions":instructions.map_or(json!({"present":false}),|v|json!({"present":true,"value":v})),"complete":complete.is_some(),"error":error.is_some()})));
    if let Some(id) = complete {
        observed.register(format!("compact {request} complete"), id);
    }
    if let Some(id) = error {
        observed.register(format!("compact {request} error"), id);
    }
    Ok(())
}
