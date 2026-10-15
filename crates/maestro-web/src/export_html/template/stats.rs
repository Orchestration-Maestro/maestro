//! Whole-document message and reported-usage accounting.
use super::document::{invalid, is_tool_call};
use maestro_app::{SessionEntry, Usage, export_html::template::read_message, is_truthy};
use serde_json::Value;
use std::io;

/// Whole-document counts and encountered model identities.
///
/// [`compute_stats`] fills these caller-editable aggregate fields.
#[derive(Debug, Default)]
pub struct Stats {
    /// User message count.
    pub user_messages: usize,
    /// Assistant message count.
    pub assistant_messages: usize,
    /// Tool result count.
    pub tool_results: usize,
    /// Custom message count, including hidden messages.
    pub custom_messages: usize,
    /// Compaction count.
    pub compactions: usize,
    /// Branch summary count.
    pub branch_summaries: usize,
    /// Assistant tool call count.
    pub tool_calls: usize,
    /// Reported token and cost components; total slots are not computed.
    pub tokens: Usage,
    /// Distinct model identities in encounter order.
    pub models: Vec<String>,
}

/// Account for every supplied entry, independent of the selected leaf.
/// # Errors
/// Returns invalid data for reached missing message/content receivers or truthy
/// nonnumeric usage components and nonstring model/provider identities.
pub fn compute_stats(entry_list: &[SessionEntry]) -> io::Result<Stats> {
    let mut stats = Stats::default();
    let mut models = indexmap::IndexSet::new();
    for entry in entry_list {
        match entry["type"].as_str() {
            Some("message") => stats.add_message(read_message(entry)?, &mut models)?,
            Some("custom_message") => stats.custom_messages += 1,
            Some("compaction") => stats.compactions += 1,
            Some("branch_summary") => stats.branch_summaries += 1,
            _ => {}
        }
    }
    stats.models = models.into_iter().collect();
    Ok(stats)
}

/// Resolve the optional provider-qualified string identity without coercion.
fn model_identity(message: &Value) -> io::Result<Option<String>> {
    if !is_truthy(&message["model"]) {
        return Ok(None);
    }
    let model = message["model"]
        .as_str()
        .ok_or_else(|| invalid("nonstring model"))?;
    if !is_truthy(&message["provider"]) {
        return Ok(Some(model.to_owned()));
    }
    let provider = message["provider"]
        .as_str()
        .ok_or_else(|| invalid("nonstring provider"))?;
    Ok(Some(format!("{provider}/{model}")))
}

/// Add each component in entry order without consulting supplied totals.
fn accumulate(tokens: &mut Usage, usage: &Value) -> io::Result<()> {
    let cost = &usage["cost"];
    for (target, source, key) in [
        (&mut tokens.input, usage, "input"),
        (&mut tokens.output, usage, "output"),
        (&mut tokens.cache_read, usage, "cacheRead"),
        (&mut tokens.cache_write, usage, "cacheWrite"),
        (&mut tokens.cost.input, cost, "input"),
        (&mut tokens.cost.output, cost, "output"),
        (&mut tokens.cost.cache_read, cost, "cacheRead"),
        (&mut tokens.cost.cache_write, cost, "cacheWrite"),
    ] {
        let value = &source[key];
        if is_truthy(value) {
            *target += value
                .as_f64()
                .ok_or_else(|| invalid("nonnumeric usage component"))?;
        }
    }
    Ok(())
}

impl Stats {
    /// Consume one reached message, validating only assistant accounting operands.
    fn add_message(
        &mut self,
        message: &Value,
        models: &mut indexmap::IndexSet<String>,
    ) -> io::Result<()> {
        match message["role"].as_str() {
            Some("user") => self.user_messages += 1,
            Some("toolResult") => self.tool_results += 1,
            Some("assistant") => {
                self.assistant_messages += 1;
                if let Some(model) = model_identity(message)? {
                    models.insert(model);
                }
                accumulate(&mut self.tokens, &message["usage"])?;
                let content = message["content"]
                    .as_array()
                    .ok_or_else(|| invalid("missing assistant content array"))?;
                for block in content {
                    self.tool_calls += usize::from(is_tool_call(block)?);
                }
            }
            _ => {}
        }
        Ok(())
    }
}
