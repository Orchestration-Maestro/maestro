//! Signed-pattern updates using raw scoped snapshots and canonical adapters.
use super::{ResourceItem, resource_list::ResourceList};
use maestro_app::{
    DecisionRequest, PathMetadata, SettingsChange, SettingsListEntry, SettingsScope, SourceOrigin,
    SourceScope,
};
use maestro_path::{Cwd, dirname, join, relative, resolve_with};
use serde_json::{Map, Value};
use std::{cell::RefCell, io, rc::Rc};

/// Replaces only exact signed strings, retaining unknown entries and order.
fn patterns(current: Option<&Value>, pattern: &str, enabled: bool) -> Vec<SettingsListEntry> {
    let mut entries: Vec<_> = current
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(SettingsListEntry::from_value)
        .filter(|entry| match entry {
            SettingsListEntry::String(text) => {
                text.strip_prefix(['!', '+', '-']).unwrap_or(text) != pattern
            }
            SettingsListEntry::Unknown(_) => true,
        })
        .collect();
    entries.push(SettingsListEntry::String(format!(
        "{}{pattern}",
        if enabled { '+' } else { '-' }
    )));
    entries
}
impl ResourceList {
    /// Reads the selected scope before deriving and persisting its pattern.
    pub(super) fn toggle_resource(
        &self,
        item: &Rc<RefCell<ResourceItem>>,
        enabled: bool,
    ) -> io::Result<()> {
        let (path, kind, metadata) = {
            let item = item.borrow();
            (
                item.path.clone(),
                item.resource_type.clone(),
                item.metadata.clone(),
            )
        };
        let snapshot = if metadata.scope == SourceScope::Project {
            self.settings.borrow().get_project_settings()
        } else {
            self.settings.borrow().get_global_settings()
        };
        if metadata.origin == SourceOrigin::TopLevel {
            let base = if metadata.scope == SourceScope::Project {
                join(&[
                    &self.options.cwd,
                    maestro_app::configuration::config::CONFIG_DIR_NAME,
                ])
            } else {
                self.options.agent_dir.clone()
            };
            let pattern = Self::relative_pattern(&base, &path)?;
            let entries = patterns(snapshot.0.get(&kind), &pattern, enabled);
            self.set_paths(metadata.scope, &kind, entries)?;
        } else {
            self.toggle_package(&snapshot.0, (&path, &kind, &metadata), enabled)?;
        }
        Ok(())
    }
    /// Edits only the first package with a matching string source.
    fn toggle_package(
        &self,
        snapshot: &Map<String, Value>,
        item: (&str, &str, &PathMetadata),
        enabled: bool,
    ) -> io::Result<()> {
        let (path, kind, metadata) = item;
        let mut packages = snapshot
            .get("packages")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let Some(index) = packages.iter().position(|package| {
            package
                .as_str()
                .or_else(|| package.get("source").and_then(Value::as_str))
                == Some(metadata.source.as_str())
        }) else {
            return Ok(());
        };
        let base = metadata.base_dir.clone().unwrap_or_else(|| dirname(path));
        let pattern = Self::relative_pattern(&base, path)?;
        let selected = &mut packages[index];
        if let Value::String(source) = selected {
            let mut object = Map::new();
            object.insert("source".into(), Value::String(std::mem::take(source)));
            *selected = Value::Object(object);
        }
        if let Value::Object(object) = selected {
            let entries = patterns(object.get(kind), &pattern, enabled);
            object.insert(
                kind.into(),
                Value::Array(
                    entries
                        .into_iter()
                        .map(SettingsListEntry::into_value)
                        .collect(),
                ),
            );
        }
        self.write_paths(metadata.scope, "packages", packages)
    }
    /// Binds open authored operands to one live ambient observation per toggle.
    fn relative_pattern(from: &str, to: &str) -> io::Result<String> {
        if from == to {
            return Ok(String::new());
        }
        let paths = resolve_with(
            &[&[from], &[to]],
            maestro_app::drive_directories,
            maestro_app::current_directory,
        )?;
        Ok(relative(
            &paths[0],
            &paths[1],
            &Cwd {
                current: "",
                drive_directories: &[],
            },
        ))
    }
    /// Routes default kinds to one admitted scoped edit.
    fn set_paths(
        &self,
        scope: SourceScope,
        kind: &str,
        entries: Vec<SettingsListEntry>,
    ) -> io::Result<()> {
        if !matches!(kind, "extensions" | "skills" | "prompts" | "themes") {
            return Ok(());
        }
        self.write_paths(
            scope,
            kind,
            entries
                .into_iter()
                .map(SettingsListEntry::into_value)
                .collect(),
        )
    }
    /// Admits the exact preference edit before the retained row or callback can change.
    fn write_paths(&self, scope: SourceScope, kind: &str, entries: Vec<Value>) -> io::Result<()> {
        let scope = if scope == SourceScope::Project {
            SettingsScope::Project
        } else {
            SettingsScope::Global
        };
        let changes = vec![SettingsChange {
            path: vec![kind.into()],
            value: Some(Value::Array(entries)),
        }];
        let mut settings = self.settings.borrow_mut();
        let edit = settings
            .prepare_operation(
                &DecisionRequest::SettingsEdit {
                    scope,
                    changes: &changes,
                },
                scope,
                changes.clone(),
            )
            .map_err(|message| {
                settings.report_refusal(scope, message.clone());
                io::Error::new(io::ErrorKind::PermissionDenied, message)
            })?;
        settings.apply_prepared_edit(edit);
        Ok(())
    }
}
