//! Stable scope/source grouping and basename-based display labels.
use super::{RESOURCE_TYPE_LABELS, ResourceItem};
use indexmap::IndexMap;
use maestro_app::{PathMetadata, ResolvedPaths, SourceOrigin, SourceScope};
use maestro_path::{basename, dirname};
use maestro_theme::Collator;
use std::{cell::RefCell, rc::Rc};

/// Captured source heading and kind groups.
pub(super) struct ResourceGroup {
    /// Source heading captured at construction.
    pub label: Rc<str>,
    /// Captured scope ordering.
    scope: SourceScope,
    /// Captured origin ordering.
    origin: SourceOrigin,
    /// Source name used for locale ordering.
    source: String,
    /// Kind headings in default order.
    pub subgroups: Vec<ResourceSubgroup>,
}
/// One kind within a source group.
pub(super) struct ResourceSubgroup {
    /// Existing resource vocabulary value.
    resource_type: String,
    /// Captured kind heading.
    pub label: Rc<str>,
    /// Live shared item handles in stable locale order.
    pub items: Vec<Rc<RefCell<ResourceItem>>>,
}
/// Names a source without collapsing its identity to the label.
fn group_label(metadata: &PathMetadata) -> String {
    if metadata.origin == SourceOrigin::Package {
        return format!("{} ({})", metadata.source, scope_name(metadata.scope));
    }
    let user = metadata.scope == SourceScope::User;
    if metadata.source == "auto" {
        if user {
            "User (~/.maestro/agent/)"
        } else {
            "Project (.maestro/)"
        }
        .into()
    } else {
        if user {
            "User settings"
        } else {
            "Project settings"
        }
        .into()
    }
}
/// Source vocabulary spelling used in keys and package labels.
fn scope_name(scope: SourceScope) -> &'static str {
    match scope {
        SourceScope::User => "user",
        SourceScope::Project => "project",
        SourceScope::Temporary => "temporary",
    }
}
/// Total scope priority, including temporary resources.
fn scope_order(scope: SourceScope) -> u8 {
    match scope {
        SourceScope::User => 0,
        SourceScope::Project => 1,
        SourceScope::Temporary => 2,
    }
}
/// Basename display exceptions, never a settings-relative path.
fn display_name(path: &str, kind: &str) -> String {
    let name = basename(path, None);
    let parent = basename(&dirname(path), None);
    match (kind, name.as_str(), parent.as_str()) {
        ("extensions", _, parent) if parent != "extensions" => format!("{parent}/{name}"),
        ("skills", "SKILL.md", _) => parent,
        _ => name,
    }
}
/// Inserts one resource into its captured source and kind owners.
fn add(
    groups: &mut IndexMap<String, ResourceGroup>,
    resource: maestro_app::ResolvedResource,
    kind: &str,
    label: &str,
) {
    let origin = if resource.metadata.origin == SourceOrigin::Package {
        "package"
    } else {
        "top-level"
    };
    let key = format!(
        "{origin}:{}:{}",
        scope_name(resource.metadata.scope),
        resource.metadata.source
    );
    let group = groups.entry(key.clone()).or_insert_with(|| ResourceGroup {
        label: group_label(&resource.metadata).into(),
        scope: resource.metadata.scope,
        origin: resource.metadata.origin,
        source: resource.metadata.source.clone(),
        subgroups: Vec::new(),
    });
    let index = group
        .subgroups
        .iter()
        .position(|subgroup| subgroup.resource_type == kind)
        .unwrap_or_else(|| {
            group.subgroups.push(ResourceSubgroup {
                resource_type: kind.into(),
                label: label.into(),
                items: Vec::new(),
            });
            group.subgroups.len() - 1
        });
    group.subgroups[index]
        .items
        .push(Rc::new(RefCell::new(ResourceItem {
            display_name: display_name(&resource.path, kind),
            path: resource.path,
            enabled: resource.enabled,
            metadata: resource.metadata,
            resource_type: kind.into(),
            subgroup_key: format!("{key}:{kind}"),
            group_key: key,
        })));
}
/// Builds stable group/kind/item order through the existing locale owner.
pub(super) fn build_groups(resolved: ResolvedPaths, collator: &Collator) -> Vec<ResourceGroup> {
    let mut groups = IndexMap::new();
    for (resources, (kind, label)) in [
        resolved.extensions,
        resolved.skills,
        resolved.prompts,
        resolved.themes,
    ]
    .into_iter()
    .zip(RESOURCE_TYPE_LABELS)
    {
        for resource in resources {
            add(&mut groups, resource, kind, label);
        }
    }
    let mut groups: Vec<_> = groups.into_values().collect();
    groups.sort_by(|a, b| {
        (a.origin != SourceOrigin::Package)
            .cmp(&(b.origin != SourceOrigin::Package))
            .then_with(|| scope_order(a.scope).cmp(&scope_order(b.scope)))
            .then_with(|| collator.compare(&a.source, &b.source))
    });
    for group in &mut groups {
        for subgroup in &mut group.subgroups {
            subgroup.items.sort_by(|a, b| {
                collator.compare(&a.borrow().display_name, &b.borrow().display_name)
            });
        }
    }
    groups
}
