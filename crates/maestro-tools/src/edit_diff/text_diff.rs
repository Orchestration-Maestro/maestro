//! Line change sequences: the shortest edit script over line tokens, choosing among
//! equally short scripts the way the edit tool's numbered diffs require.

use std::rc::Rc;

/// One run of consecutive lines; each value keeps its line separators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Change<'a> {
    /// Lines both texts share, taken from the new text.
    Unchanged(&'a str),
    /// Lines only the new text has.
    Added(&'a str),
    /// Lines only the old text has.
    Removed(&'a str),
}

/// What one run of tokens does.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Kept tokens.
    Unchanged,
    /// Inserted tokens.
    Added,
    /// Deleted tokens.
    Removed,
}

/// One run of a candidate script, sharing its earlier runs with other candidates.
struct Component {
    /// What the run does.
    kind: Kind,
    /// How many tokens it covers.
    count: usize,
    /// The run before it.
    previous: Option<Rc<Component>>,
}

impl Drop for Component {
    /// Release a long chain of runs without one stack frame per run.
    fn drop(&mut self) {
        let mut previous = self.previous.take();
        while let Some(run) = previous {
            previous = Rc::try_unwrap(run)
                .ok()
                .and_then(|mut run| run.previous.take());
        }
    }
}

/// The farthest point a candidate script reached on one diagonal.
#[derive(Clone)]
struct Path {
    /// Old tokens consumed.
    old: usize,
    /// New tokens consumed.
    new: usize,
    /// Its last run.
    last: Option<Rc<Component>>,
}

impl Path {
    /// This path extended by one token of `kind`, merged into a last run of the same kind.
    fn step(&self, kind: Kind) -> Self {
        let run = match &self.last {
            Some(last) if last.kind == kind => Component {
                kind,
                count: last.count + 1,
                previous: last.previous.clone(),
            },
            _ => Component {
                kind,
                count: 1,
                previous: self.last.clone(),
            },
        };
        Self {
            old: self.old + usize::from(kind == Kind::Removed),
            new: self.new + usize::from(kind == Kind::Added),
            last: Some(Rc::new(run)),
        }
    }
}

/// The search over two token lists.
struct Search<'t> {
    /// Old tokens.
    old: &'t [&'t str],
    /// New tokens.
    new: &'t [&'t str],
    /// Best path per diagonal, indexed by `old - new + offset`.
    best: Vec<Option<Path>>,
    /// Lowest diagonal index still worth extending.
    lowest: usize,
    /// Highest diagonal index still worth extending.
    highest: usize,
}

impl Search<'_> {
    /// Follow equal tokens from `path`, recording them as one kept run.
    fn extract_common(&self, path: &mut Path) {
        let start = path.new;
        while path.old < self.old.len()
            && path.new < self.new.len()
            && self.old[path.old] == self.new[path.new]
        {
            path.old += 1;
            path.new += 1;
        }
        if path.new > start {
            path.last = Some(Rc::new(Component {
                kind: Kind::Unchanged,
                count: path.new - start,
                previous: path.last.take(),
            }));
        }
    }

    /// Whether `path` consumed both lists.
    fn finished(&self, path: &Path) -> bool {
        path.old >= self.old.len() && path.new >= self.new.len()
    }

    /// Extend the best neighbour of `diagonal` by one edit; returns a finished path.
    fn visit(&mut self, diagonal: usize) -> Option<Path> {
        let remove = self.best[diagonal - 1].take();
        let add = self.best[diagonal + 1].as_ref();
        let can_add = add.is_some_and(|add| add.new < self.new.len());
        let can_remove = remove
            .as_ref()
            .is_some_and(|remove| remove.old < self.old.len());
        let mut path = match (add, remove) {
            (Some(add), Some(remove)) if can_remove && !(can_add && remove.old < add.old) => {
                remove.step(Kind::Removed)
            }
            (Some(add), _) if can_add => add.step(Kind::Added),
            (_, Some(remove)) if can_remove => remove.step(Kind::Removed),
            _ => {
                self.best[diagonal] = None;
                return None;
            }
        };
        self.extract_common(&mut path);
        if self.finished(&path) {
            return Some(path);
        }
        if path.old >= self.old.len() {
            self.highest = self.highest.min(diagonal - 1);
        }
        if path.new >= self.new.len() {
            self.lowest = self.lowest.max(diagonal + 1);
        }
        self.best[diagonal] = Some(path);
        None
    }

    /// The last run of the first finished script.
    fn run(mut self) -> Option<Rc<Component>> {
        let offset = self.old.len() + self.new.len() + 1;
        let mut start = Path {
            old: 0,
            new: 0,
            last: None,
        };
        self.extract_common(&mut start);
        if self.finished(&start) {
            return start.last;
        }
        self.best[offset] = Some(start);
        // The end is always reached within old plus new edits.
        let mut edits = 1;
        loop {
            if let Some(path) = self.round(offset, edits) {
                return path.last;
            }
            edits += 1;
        }
    }

    /// Try every diagonal still in range for scripts of `edits` edits.
    fn round(&mut self, offset: usize, edits: usize) -> Option<Path> {
        let mut diagonal = self.lowest.max(offset - edits);
        while diagonal <= self.highest.min(offset + edits) {
            if let Some(path) = self.visit(diagonal) {
                return Some(path);
            }
            diagonal += 2;
        }
        None
    }
}

/// One text and how much of it the runs consumed.
struct Side<'a> {
    /// The text.
    text: &'a str,
    /// Its line tokens.
    tokens: Vec<&'a str>,
    /// Tokens consumed.
    token: usize,
    /// Bytes consumed.
    byte: usize,
}

impl<'a> Side<'a> {
    /// Split `text` after every LF; a final line without LF is a token too.
    fn new(text: &'a str) -> Self {
        Self {
            text,
            tokens: text.split_inclusive('\n').collect(),
            token: 0,
            byte: 0,
        }
    }

    /// The next `count` tokens as one slice.
    fn take(&mut self, count: usize) -> &'a str {
        let end = self.byte
            + self.tokens[self.token..self.token + count]
                .iter()
                .map(|t| t.len())
                .sum::<usize>();
        let value = &self.text[self.byte..end];
        (self.token, self.byte) = (self.token + count, end);
        value
    }
}

/// The line change sequence turning `old` into `new`.
#[must_use]
pub(super) fn diff_lines<'a>(old: &'a str, new: &'a str) -> Vec<Change<'a>> {
    let (mut old, mut new) = (Side::new(old), Side::new(new));
    let size = 2 * (old.tokens.len() + new.tokens.len()) + 3;
    let search = Search {
        old: &old.tokens,
        new: &new.tokens,
        best: vec![None; size],
        lowest: 0,
        highest: usize::MAX,
    };
    let last = search.run();
    let mut runs = Vec::new();
    let mut next = last.as_deref();
    while let Some(run) = next {
        runs.push((run.kind, run.count));
        next = run.previous.as_deref();
    }
    runs.into_iter()
        .rev()
        .map(|(kind, count)| match kind {
            Kind::Unchanged => {
                old.take(count);
                Change::Unchanged(new.take(count))
            }
            Kind::Added => Change::Added(new.take(count)),
            Kind::Removed => Change::Removed(old.take(count)),
        })
        .collect()
}
