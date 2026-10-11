//! Directory-local literal matches and pending component scans.
use super::pattern::Pattern;
use crate::package_manager::resource_discovery::resolve_entry_path;
use maestro_pattern::{Minimatch, ParseReturnFiltered};
use maestro_resources::skills::{ResourceFileType, ResourceOperations};
use std::{
    collections::{HashMap, HashSet},
    io,
};
/// Authored target identity with an optional directory-entry observation.
#[derive(Clone)]
pub(super) struct Target {
    /// Absolute authored path, never canonicalized.
    pub(super) path: String,
    /// Known no-follow entry kind.
    pub(super) kind: Option<ResourceFileType>,
}
impl Target {
    /// Whether this kind permits a directory observation.
    pub(super) fn can_read(&self) -> bool {
        matches!(
            self.kind,
            None | Some(ResourceFileType::Directory | ResourceFileType::Symlink)
        )
    }
    /// Resolves a literal without following its final link.
    fn resolve(&self, text: &str, context: &mut Context<'_>) -> io::Result<Self> {
        let path = resolve_entry_path(&self.path, text, context.operations)?;
        context.observe(&path);
        Ok(Self { path, kind: None })
    }
}
/// Ordered decisions produced before recursive directory work.
#[derive(Default)]
pub(super) struct Processor {
    /// Matches and their directory-only requirement.
    pub(super) matches: Vec<(Target, bool)>,
    /// Match positions for first-occurrence and requirement merging.
    matched: HashMap<String, usize>,
    /// Ordered directory targets and pending suffixes.
    pub(super) subwalks: Vec<(Target, Vec<Pattern>)>,
    /// Target positions retaining insertion order.
    targets: HashMap<String, usize>,
}
impl Processor {
    /// Keeps the first match position; any non-directory match relaxes the guard.
    fn add_match(&mut self, target: Target, directory: bool) {
        if let Some(&at) = self.matched.get(&target.path) {
            self.matches[at].1 &= directory;
        } else {
            self.matched.insert(target.path.clone(), self.matches.len());
            self.matches.push((target, directory));
        }
    }
    /// Groups suffixes for one authored directory before scanning it.
    fn add_walk(&mut self, target: Target, pattern: Pattern) {
        if !target.can_read() {
            return;
        }
        let at = *self.targets.entry(target.path.clone()).or_insert_with(|| {
            let at = self.subwalks.len();
            self.subwalks.push((target, Vec::new()));
            at
        });
        if !self.subwalks[at].1.contains(&pattern) {
            self.subwalks[at].1.push(pattern);
        }
    }
    /// Resolves literal heads before mounting magic suffixes on their directories.
    pub(super) fn process(
        &mut self,
        mut target: Target,
        mut pattern: Pattern,
        context: &mut Context<'_>,
    ) -> io::Result<()> {
        if let Some((root, index)) = pattern.root(context.matcher) {
            target = target.resolve(&root, context)?;
            if index >= context.matcher.set()[pattern.alternative].len() {
                self.add_match(target, false);
                return Ok(());
            }
            pattern.index = index;
        }
        while let ParseReturnFiltered::Literal(text) = pattern.part(context.matcher) {
            let Some(rest) = pattern.rest(context.matcher) else {
                self.add_match(
                    target.resolve(text, context)?,
                    matches!(text.as_str(), "" | "." | ".."),
                );
                return Ok(());
            };
            target = target.resolve(text, context)?;
            pattern = rest;
        }
        if !context.walked.insert((target.path.clone(), pattern)) {
            return Ok(());
        }
        self.mount(target, pattern, context)
    }
    /// Mounts one magic suffix after literal resolution has finished.
    fn mount(
        &mut self,
        target: Target,
        pattern: Pattern,
        context: &mut Context<'_>,
    ) -> io::Result<()> {
        let rest = pattern.rest(context.matcher);
        if matches!(pattern.part(context.matcher), ParseReturnFiltered::Globstar) {
            if target.kind != Some(ResourceFileType::Symlink) || context.follows(pattern) {
                self.add_walk(target.clone(), pattern);
            }
            match rest.map(|p| (p, p.part(context.matcher))) {
                None => self.add_match(target, false),
                Some((rest, ParseReturnFiltered::Literal(text)))
                    if matches!(text.as_str(), "" | ".")
                        && rest.rest(context.matcher).is_none() =>
                {
                    self.add_match(target, true);
                }
                Some((rest, ParseReturnFiltered::Literal(text))) if text == ".." => {
                    let parent = target.resolve("..", context)?;
                    self.accept_parent(parent, rest.rest(context.matcher));
                }
                _ => {}
            }
        } else {
            self.add_walk(target, pattern);
        }
        Ok(())
    }
    /// Tests one native child against a pending component and its continuation.
    pub(super) fn test(&mut self, target: Target, pattern: Pattern, context: &mut Context<'_>) {
        let rest = pattern.rest(context.matcher);
        let name = maestro_path::basename(&target.path, None);
        match pattern.part(context.matcher) {
            ParseReturnFiltered::Globstar => self.test_globstar(target, pattern, context),
            ParseReturnFiltered::Literal(text) if context.is_named(text, &name) => {
                self.accept(target, rest);
            }
            ParseReturnFiltered::Pattern(segment) if segment.test(&name) => {
                self.accept(target, rest);
            }
            _ => {}
        }
    }
    /// Separates globstar continuation from its next component's independent test.
    fn test_globstar(&mut self, target: Target, pattern: Pattern, context: &mut Context<'_>) {
        let rest = pattern.rest(context.matcher);
        let name = maestro_path::basename(&target.path, None);
        if !name.starts_with('.') {
            if rest.is_none() {
                self.add_match(target.clone(), false);
            }
            if target.kind != Some(ResourceFileType::Symlink) {
                self.add_walk(target.clone(), pattern);
            } else if let Some(rest) = rest.filter(|_| context.follows(pattern)) {
                self.add_walk(target.clone(), rest);
            } else if context.follows(pattern) && context.followed.insert(pattern) {
                self.add_walk(target.clone(), pattern);
            }
        }
        if let Some(rest) = rest {
            if matches!(rest.part(context.matcher), ParseReturnFiltered::Literal(text) if text == "..")
            {
                let parent = Target {
                    path: maestro_path::dirname(&target.path),
                    kind: None,
                };
                self.add_walk(parent, rest);
            } else if !matches!(rest.part(context.matcher), ParseReturnFiltered::Literal(text) if matches!(text.as_str(), "" | "."))
            {
                self.test(target, rest, context);
            }
        }
    }
    /// Retains the directory-only requirement of a terminal parent operand.
    fn accept_parent(&mut self, target: Target, rest: Option<Pattern>) {
        if let Some(rest) = rest {
            self.add_walk(target, rest);
        } else {
            self.add_match(target, true);
        }
    }
    /// Emits a terminal or retains the pending suffix on this child.
    fn accept(&mut self, target: Target, rest: Option<Pattern>) {
        if let Some(rest) = rest {
            self.add_walk(target, rest);
        } else {
            self.add_match(target, false);
        }
    }
}
/// Per-expansion suffix state and filesystem port.
pub(super) struct Context<'a> {
    /// One shared compiled matcher.
    pub(super) matcher: &'a Minimatch,
    /// Whether native child comparisons ignore case.
    pub(super) nocase: bool,
    /// Supplied filesystem effects.
    pub(super) operations: &'a dyn ResourceOperations,
    /// Ordered child identities established by literal resolution or scanning.
    pub(super) children: HashMap<String, Vec<String>>,
    /// Processed authored-target/suffix transitions.
    pub(super) walked: HashSet<(String, Pattern)>,
    /// Suffixes whose one eligible recursive link transition was consumed.
    pub(super) followed: HashSet<Pattern>,
}
impl Context<'_> {
    /// Retains literal child insertion order along the resolved authored path.
    fn observe(&mut self, path: &str) {
        let mut child = path.to_owned();
        loop {
            let parent = maestro_path::dirname(&child);
            if parent == child {
                break;
            }
            let children = self.children.entry(parent.clone()).or_default();
            let name = maestro_path::basename(&child, None);
            if children.contains(&name) {
                break;
            }
            children.push(name);
            child = parent;
        }
    }
    /// Compares names without normalizing or replacing authored identities.
    fn is_named(&self, expected: &str, name: &str) -> bool {
        if self.nocase {
            expected.to_lowercase() == name.to_lowercase()
        } else {
            expected == name
        }
    }
    /// Initial globstars cannot traverse an encountered link.
    fn follows(&self, pattern: Pattern) -> bool {
        pattern.index > 0 && !self.followed.contains(&pattern)
    }
}
