//! Iterative synchronous directory phases without recursive callback machinery.
use super::{
    pattern::Pattern,
    processor::{Context, Processor, Target},
};
use maestro_pattern::{Minimatch, Platform};
use maestro_resources::skills::ResourceOperations;
use std::{
    collections::{HashMap, HashSet},
    io,
    path::Path,
};
/// One directory phase and its pending suffix identities.
enum Phase {
    /// Resolve literal heads and emit their matches first.
    Process(Target, Vec<Pattern>),
    /// Observe children before recursively processing their continuations.
    Read(Target, Vec<Pattern>),
}
/// Emits literals before scans, and completes current matches before subwalks.
pub(super) fn walk(
    matcher: &Minimatch,
    root: &str,
    platform: Platform,
    operations: &dyn ResourceOperations,
) -> io::Result<Vec<String>> {
    let root = super::super::resource_discovery::resolve_entry_path(root, "", operations)?;
    let observed = operations.symlink_metadata(Path::new(&root));
    let missing = observed.as_ref().is_err_and(|e| {
        matches!(
            e.kind(),
            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
        )
    });
    let target = Target {
        path: root,
        kind: observed.ok(),
    };
    let patterns = (0..matcher.set().len())
        .map(|alternative| Pattern {
            alternative,
            index: 0,
        })
        .filter(|pattern| !missing || pattern.root(matcher).is_some())
        .collect();
    let mut pending = vec![Phase::Process(target, patterns)];
    let mut context = Context {
        matcher,
        nocase: platform != Platform::Posix,
        children: HashMap::new(),
        operations,
        walked: HashSet::new(),
        followed: HashSet::new(),
    };
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    while let Some(phase) = pending.pop() {
        let mut processor = Processor::default();
        let scanning = match phase {
            Phase::Process(target, patterns) => {
                for pattern in patterns {
                    processor.process(target.clone(), pattern, &mut context)?;
                }
                true
            }
            Phase::Read(target, patterns) => {
                scan(&mut processor, &target, &patterns, &mut context);
                false
            }
        };
        emit(processor.matches, operations, &mut seen, &mut result);
        for (target, patterns) in processor.subwalks.into_iter().rev() {
            pending.push(if scanning {
                Phase::Read(target, patterns)
            } else {
                Phase::Process(target, patterns)
            });
        }
    }
    Ok(result)
}

/// Promotes previously resolved children while reversing newly observed entries.
fn scan(
    processor: &mut Processor,
    target: &Target,
    patterns: &[Pattern],
    context: &mut Context<'_>,
) {
    let Ok(entries) = context.operations.read_dir(Path::new(&target.path)) else {
        return;
    };
    let names = context.children.entry(target.path.clone()).or_default();
    for (provisional, entry) in entries.iter().enumerate() {
        let name = entry.name.to_string_lossy().into_owned();
        let index = names.iter().position(|known| *known == name);
        if index != Some(provisional) {
            if let Some(index) = index {
                names.remove(index);
            }
            names.insert(0, name);
        }
    }
    names.truncate(entries.len());
    let mut by_name: HashMap<_, _> = entries
        .into_iter()
        .map(|entry| (entry.name.to_string_lossy().into_owned(), entry))
        .collect();
    let entries: Vec<_> = names
        .iter()
        .filter_map(|name| by_name.remove(name))
        .collect();
    for entry in entries {
        let child = Target {
            path: maestro_path::join(&[&target.path, &entry.name.to_string_lossy()]),
            kind: Some(entry.file_type),
        };
        for pattern in patterns {
            processor.test(child.clone(), *pattern, context);
        }
    }
}

/// Publishes admitted authored identities after no-follow terminal checks.
fn emit(
    matches: Vec<(Target, bool)>,
    operations: &dyn ResourceOperations,
    seen: &mut HashSet<String>,
    result: &mut Vec<String>,
) {
    for (mut target, directory) in matches {
        if target.kind.is_none() {
            target.kind = operations.symlink_metadata(Path::new(&target.path)).ok();
        }
        if target.kind.is_some()
            && (!directory || target.can_read())
            && seen.insert(target.path.clone())
        {
            result.push(target.path);
        }
    }
}
