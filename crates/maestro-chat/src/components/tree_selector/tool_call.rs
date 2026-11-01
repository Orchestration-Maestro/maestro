//! Borrowed assistant tool-call lookup.
use super::{SessionTreeNode, tree_list::TreeState};
use serde_json::Value;
use std::collections::HashMap;
/// Keep the last call in full navigation order, including hidden rows.
pub(super) fn collect(nodes: &[SessionTreeNode]) -> HashMap<String, (usize, usize)> {
    nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            node.entry()["type"] == "message" && node.entry()["message"]["role"] == "assistant"
        })
        .flat_map(|(index, node)| {
            node.entry()["message"]["content"]
                .as_array()
                .into_iter()
                .flatten()
                .enumerate()
                .filter(|(_, block)| block["type"] == "toolCall")
                .filter_map(move |(block_index, block)| {
                    block["id"]
                        .as_str()
                        .map(|id| (id.to_owned(), (index, block_index)))
                })
        })
        .collect()
}
use super::entry_text::{normalize, text};
use maestro_app::{is_truthy, truncate_utf16};
use maestro_tui::tui::TuiRuntime;
use std::{borrow::Cow, io};
impl TreeState {
    /// Resolve a visible result against all retained assistant calls.
    pub(super) fn tool_result(
        &self,
        message: &Value,
        runtime: &dyn TuiRuntime,
    ) -> io::Result<String> {
        if let Some(&(index, block)) = message["toolCallId"]
            .as_str()
            .and_then(|id| self.tool_calls.get(id))
        {
            let call = &self.nodes[index].entry()["message"]["content"][block];
            return format_call(text(&call["name"])?, &call["arguments"], runtime);
        }
        let name = message
            .get("toolName")
            .filter(|v| !v.is_null())
            .map_or(Ok("tool"), text)?;
        Ok(format!("[{name}]"))
    }
}
/// Summarize only the fields used by the selected tool kind.
fn format_call(name: &str, args: &Value, runtime: &dyn TuiRuntime) -> io::Result<String> {
    match name {
        "bash" => {
            let raw = argument(args, "command", "")?;
            let command = truncate_utf16(&normalize(&raw), 50);
            Ok(format!(
                "[bash: {command}{}]",
                if raw.encode_utf16().count() > 50 {
                    "..."
                } else {
                    ""
                }
            ))
        }
        "read" | "write" | "edit" => {
            let raw = if args.get("path").is_some_and(is_truthy) {
                argument(args, "path", "")?
            } else {
                argument(args, "file_path", "")?
            };
            let path = shorten_path(&raw, runtime);
            let range = if name == "read" {
                maestro_app::format_read_line_range(args)
            } else {
                String::new()
            };
            Ok(format!("[{name}: {path}{range}]"))
        }
        "grep" | "find" | "ls" => {
            let path = shorten_path(&argument(args, "path", ".")?, runtime);
            if name == "ls" {
                return Ok(format!("[ls: {path}]"));
            }
            let pattern = argument(args, "pattern", "")?;
            let pattern = if name == "grep" {
                format!("/{pattern}/")
            } else {
                pattern.into_owned()
            };
            Ok(format!("[{name}: {pattern} in {path}]"))
        }
        _ => {
            let serialized = maestro_app::compact_json(args).map_err(io::Error::other)?;
            let preview = truncate_utf16(&serialized, 40);
            Ok(format!(
                "[{name}: {preview}{}]",
                if serialized.encode_utf16().count() > 40 {
                    "..."
                } else {
                    ""
                }
            ))
        }
    }
}
/// Display truthy arguments, otherwise the tool's own default.
///
/// A named tool reads its fields from an arguments value, so a null receiver has none;
/// a generic tool prints the whole value, which includes a null root.
fn argument<'a>(args: &'a Value, key: &str, default: &'a str) -> io::Result<Cow<'a, str>> {
    if args.is_null() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "missing tool arguments receiver",
        ));
    }
    maestro_app::display_truthy(args.get(key))
        .map(|text| text.unwrap_or(Cow::Borrowed(default)))
        .map_err(io::Error::other)
}
/// Bind the live nonempty home observation only when formatting a path.
fn shorten_path(path: &str, runtime: &dyn TuiRuntime) -> String {
    super::super::shorten_path_with_home(path, &|variable| runtime.environment(variable))
}
