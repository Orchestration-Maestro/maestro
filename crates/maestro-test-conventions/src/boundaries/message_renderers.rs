//! Single-owner rule for retained message renderer calls.
use std::collections::BTreeMap;

use syn::ext::IdentExt;

use crate::source::{
    Member, Source,
    imports::{Import, flatten},
};

/// Crate that owns the projected call and the callbacks it retains.
const OWNER: &str = "maestro-extensions";
/// Frontend that may only use the projected call.
const FRONTEND: &str = "maestro-chat";
/// Record that binds a callback to its registering event bus.
const PROJECTED: &str = "MessageRendererCall";
/// Raw extension machinery the frontend must not reach directly.
const RAW: &[&str] = &[
    "MessageRenderer",
    "CallbackEmitter",
    "EventBus",
    "get_message_renderer",
    "invoke_callback",
];

/// Relative path of the file that declares the projected record.
const OWNER_FILE: &str = "src/types/renderers.rs";

/// Reject raw renderer plumbing in frontend production syntax and foreign declarations.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut diagnostics = Vec::new();
    for member in members {
        for source in &member.sources {
            if source.syntax.is_none() {
                continue;
            }
            let path = source.path.display().to_string();
            diagnostics.extend(
                source
                    .walk(member)
                    .renderers
                    .iter()
                    .map(|(position, message)| (path.clone(), *position, message.clone())),
            );
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        diagnostics.sort_by(|left, right| (&left.0, left.1).cmp(&(&right.0, right.1)));
        diagnostics.dedup();
        Err(diagnostics
            .into_iter()
            .map(|(file, (line, _), message)| format!("{file}:{line}: {message}"))
            .collect::<Vec<_>>()
            .join("\n"))
    }
}

impl Import<'_> {
    /// Name of the item the import binds, where `self` names the module above it.
    fn item(&self) -> Option<&syn::Ident> {
        let (last, above) = self.path.split_last()?;
        if **last == "self" {
            above.last().copied()
        } else {
            Some(last)
        }
    }
}

#[cfg(test)]
thread_local! {
    /// Identifier spellings materialized by the renderer check on the current test thread.
    static SPELLINGS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Restricted name an identifier spells, ignoring a raw prefix.
fn restricted(name: &syn::Ident) -> Option<&'static str> {
    #[cfg(test)]
    SPELLINGS.set(SPELLINGS.get() + 1);
    let spelling = name.to_string();
    let spelling = spelling.strip_prefix("r#").unwrap_or(&spelling);
    RAW.iter().copied().find(|raw| *raw == spelling)
}

/// One finding of the renderer check, in source order.
enum Event<'a> {
    /// A frontend production identifier, resolved once every alias of the file is known.
    Name(&'a syn::Ident),
    /// A located declaration violation.
    Found((usize, usize), String),
}

/// Located rule violations for one source, gathered by the shared syntax walk.
///
/// The walk reports production syntax only; test-owned syntax never reaches these methods.
pub(crate) struct Guard<'a> {
    /// Crate the source belongs to.
    member: &'a str,
    /// Raw identifiers imported under another local name, across the whole file.
    aliases: BTreeMap<String, &'static str>,
    /// Whether this source is the file that must hold the single declaration.
    owner_file: bool,
    /// Production declarations of the projected record seen in this source.
    declarations: usize,
    /// Findings in source order.
    events: Vec<Event<'a>>,
}

impl<'a> Guard<'a> {
    /// Start the renderer check of one member's source.
    pub(crate) fn new(member: &'a Member, source: &Source) -> Self {
        Self {
            member: &member.name,
            aliases: BTreeMap::new(),
            owner_file: source
                .path
                .strip_prefix(&member.directory)
                .is_ok_and(|relative| relative == std::path::Path::new(OWNER_FILE)),
            declarations: 0,
            events: Vec::new(),
        }
    }

    /// Records a raw identifier reached from the frontend.
    pub(crate) fn raw(&mut self, ident: &'a syn::Ident) {
        if self.member == FRONTEND {
            self.events.push(Event::Name(ident));
        }
    }

    /// Records the frontend's restricted aliases and every restricted name a use tree
    /// writes, prefix segments included.
    pub(crate) fn item_use(&mut self, item: &'a syn::ItemUse) {
        // Only the frontend's raw names are reported, so only its aliases are read.
        if self.member != FRONTEND {
            return;
        }
        for import in flatten(&item.tree) {
            if let (Some(binding), Some(raw)) = (import.binding, import.item().and_then(restricted))
            {
                self.aliases.insert(binding.unraw().to_string(), raw);
            }
            for segment in import.path {
                self.raw(segment);
            }
        }
    }

    /// Records the segments of a path written in the frontend.
    pub(crate) fn path(&mut self, path: &'a syn::Path) {
        for segment in &path.segments {
            self.raw(&segment.ident);
        }
    }

    /// Records a foreign declaration of the projected record.
    pub(crate) fn declaration(&mut self, name: &syn::Ident) {
        if name.unraw() != PROJECTED {
            return;
        }
        let position = name.span().start();
        if self.member != OWNER {
            self.events.push(Event::Found(
                (position.line, position.column),
                format!(
                    "{PROJECTED} declaration belongs to {OWNER}, not {}",
                    self.member
                ),
            ));
            return;
        }
        self.declarations += 1;
        let message = if !self.owner_file {
            format!("duplicate {PROJECTED} declaration; {OWNER} declares it once in {OWNER_FILE}")
        } else if self.declarations > 1 {
            format!("second {PROJECTED} declaration; {OWNER} declares it once in {OWNER_FILE}")
        } else {
            return;
        };
        self.events
            .push(Event::Found((position.line, position.column), message));
    }

    /// Resolve the recorded names against the file's aliases, keeping source order.
    pub(crate) fn finish(self) -> Vec<((usize, usize), String)> {
        let aliases = self.aliases;
        self.events
            .into_iter()
            .filter_map(|event| match event {
                Event::Name(ident) => {
                    #[cfg(test)]
                    SPELLINGS.set(SPELLINGS.get() + 1);
                    let written = ident.unraw().to_string();
                    let raw = aliases
                        .get(&written)
                        .copied()
                        .or_else(|| RAW.iter().copied().find(|raw| *raw == written))?;
                    let position = ident.span().start();
                    Some((
                        (position.line, position.column),
                        format!("raw {raw} outside {OWNER}; bind a renderer through {PROJECTED} and call that"),
                    ))
                }
                Event::Found(position, message) => Some((position, message)),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
