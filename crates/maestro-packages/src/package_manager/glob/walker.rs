//! Iterative synchronous directory phases without recursive callback machinery.
use super::{
    pattern::Pattern,
    processor::{Context, Processor, Target},
};
use maestro_pattern::Minimatch;
use maestro_resources::skills::ResourceOperations;
use std::{collections::HashSet, io, path::Path};
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
    operations: &dyn ResourceOperations,
) -> io::Result<Vec<String>> {
    let observed = operations.symlink_metadata(Path::new(root));
    let missing = observed.as_ref().is_err_and(|e| {
        matches!(
            e.kind(),
            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
        )
    });
    let target = Target {
        path: root.into(),
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

/// Observes one native entry list and tests its children in insertion order.
fn scan(
    processor: &mut Processor,
    target: &Target,
    patterns: &[Pattern],
    context: &mut Context<'_>,
) {
    let Ok(entries) = context.operations.read_dir(Path::new(&target.path)) else {
        return;
    };
    for entry in entries.into_iter().rev() {
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
