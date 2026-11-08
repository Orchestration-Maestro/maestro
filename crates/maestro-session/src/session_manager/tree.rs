#![doc = include_str!("../../../../docs/tree.md")]
use super::{
    FileEntry, LabelEntry, ReadonlySessionManager, SessionEntry, SessionManager, SessionTreeNode,
    conversions::invalid,
    history::{self as context, ecma_whitespace, truthy},
};
use maestro_request::presence::Presence;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::io;

/// Apply a label record to the resolved maps; a falsy or absent label clears its target.
///
/// Targets that are not strings can never match an entry identity and are ignored.
pub(super) fn apply_label(
    labels: &mut Map<String, Value>,
    timestamps: &mut Map<String, Value>,
    entry: &FileEntry,
) {
    let Some(target) = entry["targetId"].as_str() else {
        return;
    };
    let label = entry.get("label").filter(|label| truthy(label));
    let stamp = entry.get("timestamp").filter(|_| label.is_some());
    for (map, value) in [(labels, label), (timestamps, stamp)] {
        match value {
            Some(value) => map.insert(target.to_owned(), value.clone()),
            None => map.shift_remove(target),
        };
    }
}

impl ReadonlySessionManager for SessionManager {
    fn get_cwd(&self) -> &str {
        &self.cwd
    }
    fn get_session_dir(&self) -> &str {
        &self.session_dir
    }
    fn get_session_id(&self) -> &str {
        &self.session_id
    }
    fn get_session_file(&self) -> Option<&str> {
        self.session_file.as_deref()
    }
    fn get_leaf_id(&self) -> Option<&str> {
        self.leaf_id.as_deref()
    }
    fn get_leaf_entry(&self) -> Option<&SessionEntry> {
        self.get_leaf_id().and_then(|id| self.get_entry(id))
    }
    fn get_entry(&self, id: &str) -> Option<&SessionEntry> {
        self.by_id
            .get(id)
            .and_then(|position| self.file_entries.get(*position))
    }
    fn get_label(&self, id: &str) -> Option<&Value> {
        self.labels.get(id)
    }
    fn get_branch(&self, from_id: Option<&str>) -> Result<Vec<&SessionEntry>, serde_json::Error> {
        match from_id
            .or(self.get_leaf_id())
            .filter(|id| !id.is_empty())
            .and_then(|id| self.get_entry(id))
        {
            Some(leaf) => context::ancestor_path(leaf, |id| self.get_entry(id)),
            None => Ok(Vec::new()),
        }
    }
    fn get_header(&self) -> Option<&FileEntry> {
        self.file_entries.first()
    }
    fn get_entries(&self) -> Vec<&SessionEntry> {
        self.file_entries
            .iter()
            .filter(|entry| entry["type"] != "session")
            .collect()
    }
    fn get_tree(&self) -> Vec<SessionTreeNode<'_>> {
        crate::export_html::template::build_tree(
            self.get_entries(),
            |id| self.labels.get(id),
            |id| self.label_timestamps.get(id),
        )
    }
    fn get_session_name(&self) -> io::Result<Option<&str>> {
        let newest = self
            .file_entries
            .iter()
            .rev()
            .find(|entry| entry["type"] == "session_info");
        match newest.map(|entry| &entry["name"]) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::String(name)) => {
                Ok(Some(name.trim_matches(ecma_whitespace)).filter(|name| !name.is_empty()))
            }
            Some(_) => Err(invalid("nonstring session name")),
        }
    }
}

impl SessionManager {
    /// Borrow direct children of an identity in first-seen identity order.
    ///
    /// Each identity contributes its last record, whose own parent decides membership.
    #[must_use]
    pub fn get_children(&self, parent_id: &str) -> Vec<&SessionEntry> {
        let mut seen = HashSet::new();
        self.get_entries()
            .into_iter()
            .filter_map(|entry| entry["id"].as_str())
            .filter(|id| seen.insert(*id))
            .filter_map(|id| self.get_entry(id))
            .filter(|entry| entry["parentId"].as_str() == Some(parent_id))
            .collect()
    }

    /// Select the position before all entries without writing anything.
    pub fn reset_leaf(&mut self) {
        self.leaf_id = None;
    }

    /// Append a label change below the selected entry, updating the resolved label maps
    /// only after the publication routine returns successfully (it may complete without writing).
    ///
    /// A missing, null or empty label clears the target's label.
    ///
    /// # Errors
    /// Returns `Entry {id} not found` before any change when the target is not indexed;
    /// a publication error leaves the accepted entry but the previous resolved labels.
    pub fn append_label_change(
        &mut self,
        target_id: &str,
        label: Presence<&str>,
    ) -> io::Result<String> {
        self.require_entry(target_id)?;
        let id = self.append_record(|base| LabelEntry {
            base,
            target_id: target_id.to_owned(),
            label: match label {
                Presence::Missing => Presence::Missing,
                Presence::Null => Presence::Null,
                Presence::Present(text) => Presence::Present(text.to_owned()),
            },
        })?;
        if let Some(entry) = self.file_entries.last() {
            apply_label(&mut self.labels, &mut self.label_timestamps, entry);
        }
        Ok(id)
    }
}
