//! Iterative synchronous directory phases without recursive callback machinery.
use super::{
    pattern::Pattern,
    processor::{Context, Processor, Target},
};
use maestro_pattern::{Minimatch, Platform};
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
pub(super) fn walk(matcher: &Minimatch, root: &str, platform: Platform) -> io::Result<Vec<String>> {
    let root = maestro_resources::paths::resolve(&[root])?;
    let observed = maestro_resources::symlink_metadata(Path::new(&root));
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
    let mut context = Context {
        matcher,
        nocase: platform != Platform::Posix,
        children: HashMap::new(),
        walked: HashSet::new(),
        followed: HashSet::new(),
    };
    context.seed_cwd(&target.path);
    let mut pending = vec![Phase::Process(target, patterns)];
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
        emit(processor.matches, &mut seen, &mut result);
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

/// Establishes a listing once, then tests its retained authored identities.
fn scan(
    processor: &mut Processor,
    target: &Target,
    patterns: &[Pattern],
    context: &mut Context<'_>,
) {
    let names = context.children.entry(target.path.clone()).or_default();
    let mut kinds = HashMap::new();
    if !names.complete {
        let Ok(entries) = maestro_resources::read_entries(Path::new(&target.path)) else {
            return;
        };
        let mut listing = Vec::new();
        for entry in entries {
            let name = entry.name.to_string_lossy().into_owned();
            kinds.insert(name.clone(), entry.file_type);
            listing.push(name);
        }
        names.readdir_sync(listing.into_iter(), context.nocase);
    }
    let children: Vec<_> = names
        .listed()
        .iter()
        .map(|child| {
            let path = maestro_path::join(&[&target.path, &child.authored_name]);
            let kind = kinds
                .remove(&child.name)
                .or_else(|| maestro_resources::symlink_metadata(Path::new(&path)).ok());
            (Target { path, kind }, child.name.clone())
        })
        .collect();
    for (child, name) in children {
        for pattern in patterns {
            processor.test(child.clone(), &name, *pattern, context);
        }
    }
}

/// Publishes admitted authored identities after no-follow terminal checks.
fn emit(matches: Vec<(Target, bool)>, seen: &mut HashSet<String>, result: &mut Vec<String>) {
    for (mut target, directory) in matches {
        if target.kind.is_none() {
            target.kind = maestro_resources::symlink_metadata(Path::new(&target.path)).ok();
        }
        if target.kind.is_some()
            && (!directory || target.can_read())
            && seen.insert(target.path.clone())
        {
            result.push(target.path);
        }
    }
}
