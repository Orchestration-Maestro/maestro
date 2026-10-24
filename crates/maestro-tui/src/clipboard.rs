//! Deleted-text history shared by editable components.

/// Direction and coalescing for a deleted-text entry.
#[derive(Clone, Copy)]
pub struct KillRingOptions {
    /// Prepend rather than append when accumulating.
    pub prepend: bool,
    /// Merge into the newest entry when one exists.
    pub accumulate: bool,
}

/// Deleted strings, with the newest entry at the end.
#[derive(Default)]
pub struct KillRing {
    /// Entries in insertion or rotated order.
    entries: Vec<String>,
}

impl KillRing {
    /// Creates an empty ring.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ignores empty text; otherwise stores or accumulates the requested entry.
    pub fn push(&mut self, text: &str, opts: KillRingOptions) {
        if text.is_empty() {
            return;
        }
        if let Some(last) = self.entries.last_mut().filter(|_| opts.accumulate) {
            if opts.prepend {
                last.insert_str(0, text);
            } else {
                last.push_str(text);
            }
        } else {
            self.entries.push(text.to_owned());
        }
    }

    /// Borrows the newest entry.
    #[must_use]
    pub fn peek(&self) -> Option<&str> {
        self.entries.last().map(String::as_str)
    }

    /// Moves the newest entry to the front when at least two entries exist.
    pub fn rotate(&mut self) {
        if self.entries.len() > 1 {
            self.entries.rotate_right(1);
        }
    }

    /// Returns the entry count.
    #[must_use]
    pub fn length(&self) -> usize {
        self.entries.len()
    }
}
