//! Selected-path extraction.
use super::{
    FileEntry, ReadonlySessionManager, SessionEntry, SessionManager, build_index, generate_id,
    session_header,
};
use maestro_request::presence::Presence;
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};
use std::io;
use uuid::Uuid;

impl SessionManager {
    /// Replace the history with the ancestry of `leaf_id` and its applicable resolved labels.
    ///
    /// Label records on the ancestry are dropped and each retained record that pointed at one
    /// is reconnected to that label's own parent. Resolved labels whose target is retained are
    /// recreated, in resolved order with their stored timestamps, after the retained tail.
    /// Native mode returns the new session file, writing it at once only when a retained
    /// message is from the assistant; memory mode returns `None`.
    ///
    /// # Errors
    /// Fails with `Entry {leaf_id} not found`, changing nothing, when the ancestry is empty.
    /// A native publication error leaves the new history and the previous publication state.
    pub fn create_branched_session(&mut self, leaf_id: &str) -> io::Result<Option<String>> {
        self.branch_history(leaf_id, Uuid::new_v4)?;
        #[cfg(not(target_arch = "wasm32"))]
        self.publish_branched(super::persistence::rewrite)?;
        Ok(self.session_file.clone().filter(|_| self.persist))
    }

    /// Install the extracted history; `next_uuid` supplies recreated label identities.
    pub(super) fn branch_history(
        &mut self,
        leaf_id: &str,
        next_uuid: impl FnMut() -> Uuid,
    ) -> io::Result<()> {
        let path = self.get_branch(Some(leaf_id))?;
        if path.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Entry {leaf_id} not found"),
            ));
        }
        let mut entries = vec![Value::Null];
        entries.extend(retained(&path));
        self.append_labels(&mut entries, next_uuid);
        let parent = self.session_file.clone().filter(|_| self.persist);
        let id = Uuid::now_v7().to_string();
        let (header, file) = session_header(
            &id,
            &self.cwd,
            parent.map_or(Presence::Missing, Presence::Present),
        );
        entries[0] = header;
        let (mut by_id, mut leaf, mut labels, mut stamps) =
            (HashMap::new(), None, Map::new(), Map::new());
        build_index(&entries, &mut by_id, &mut leaf, &mut labels, &mut stamps)?;
        if self.persist {
            self.session_file = Some(maestro_path::join(&[&self.session_dir, &file]));
        }
        (self.file_entries, self.session_id) = (entries, id);
        (self.by_id, self.leaf_id) = (by_id, leaf);
        (self.labels, self.label_timestamps) = (labels, stamps);
        Ok(())
    }

    /// Chain recreated label records for retained targets after the last retained entry.
    fn append_labels(&self, entries: &mut Vec<FileEntry>, mut next_uuid: impl FnMut() -> Uuid) {
        let mut occupied: HashSet<String> = entries
            .iter()
            .filter_map(|entry| entry["id"].as_str().map(str::to_owned))
            .collect();
        let mut parent = entries
            .last()
            .map_or(Value::Null, |entry| entry["id"].clone());
        let targets: Vec<&String> = self
            .labels
            .keys()
            .filter(|target| occupied.contains(*target))
            .collect();
        let mut recreated = Vec::new();
        for target in targets {
            let id = generate_id(|id| occupied.contains(id), &mut next_uuid);
            occupied.insert(id.clone());
            recreated.push(json!({
                "type": "label", "id": id, "parentId": parent,
                "timestamp": self.label_timestamps[target.as_str()], "targetId": target,
                "label": self.labels[target.as_str()],
            }));
            parent = json!(id);
        }
        entries.extend(recreated);
    }
}

/// Copy the nonlabel records of an ancestry, reconnecting parents that were removed labels.
fn retained(path: &[&SessionEntry]) -> Vec<FileEntry> {
    let mut removed: HashMap<&str, &Value> = HashMap::new();
    let mut kept = Vec::new();
    for &entry in path {
        let moved = entry["parentId"]
            .as_str()
            .and_then(|parent| removed.get(parent));
        if entry["type"] == "label" {
            if let Some(id) = entry["id"].as_str() {
                removed.insert(id, moved.copied().unwrap_or(&entry["parentId"]));
            }
            continue;
        }
        let mut copy = entry.clone();
        if let Some(parent) = moved {
            copy["parentId"] = (*parent).clone();
        }
        kept.push(copy);
    }
    kept
}
