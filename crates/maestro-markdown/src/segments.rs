//! Shared, offset-indexed concatenations with bounded flat-input ownership.
use std::{ops::Range, sync::Arc};

/// A selected logical window with cached native bounds.
#[derive(Clone, Debug)]
pub(crate) struct Segments<T> {
    /// Shared leaf or flat sequence of real snapshots.
    storage: Arc<Storage<T>>,
    /// Selected logical bytes in this storage.
    window: Range<usize>,
    /// Native bounding range of the selected value.
    pub(crate) extent: Range<usize>,
}

/// Concatenation metadata has one owner for text and source projections.
#[derive(Clone, Debug)]
enum Storage<T> {
    /// One caller-owned operand.
    Leaf(T),
    /// Logical end offsets paired with ordered source operands.
    Joined(Vec<(usize, Segments<T>)>),
}

impl<T: Clone> Segments<T> {
    /// Admit one operand with its logical length and native extent.
    pub(crate) fn new(value: T, length: usize, extent: Range<usize>) -> Self {
        Self {
            storage: Arc::new(Storage::Leaf(value)),
            window: 0..length,
            extent,
        }
    }

    /// Logical byte length without traversing retained operands.
    pub(crate) fn len(&self) -> usize {
        self.window.len()
    }

    /// Borrow a selected leaf and its original logical coordinates.
    pub(crate) fn leaf(&self) -> Option<(&T, Range<usize>)> {
        match self.storage.as_ref() {
            Storage::Leaf(value) => Some((value, self.window.clone())),
            Storage::Joined(_) => None,
        }
    }

    /// Append in place unless another snapshot or a selected window retains this storage.
    pub(crate) fn append(&mut self, other: &Self) {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.source_views += 1);
        if other.len() == 0 {
            return;
        }
        if self.len() == 0 {
            *self = other.clone();
            return;
        }
        let length = self.len() + other.len();
        if Arc::strong_count(&self.storage) == 1
            && let Storage::Joined(parts) = Arc::make_mut(&mut self.storage)
            && self.window == (0..parts.last().map_or(0, |part| part.0))
        {
            parts.push((length, other.clone()));
            self.window.end = length;
            self.extent.end = other.extent.end;
            return;
        }
        *self = Self {
            storage: Arc::new(Storage::Joined(vec![
                (self.len(), self.clone()),
                (length, other.clone()),
            ])),
            window: 0..length,
            extent: self.extent.start..other.extent.end,
        };
    }

    /// Indexed children intersecting a logical selection, in source order.
    pub(crate) fn parts(
        &self,
        range: Range<usize>,
    ) -> impl DoubleEndedIterator<Item = (&Self, Range<usize>, usize)> {
        let absolute = self.window.start + range.start..self.window.start + range.end;
        let parts = match self.storage.as_ref() {
            Storage::Leaf(_) => &[][..],
            Storage::Joined(parts) => parts.as_slice(),
        };
        let first = parts.partition_point(|part| {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.source_views += 1);
            part.0 < absolute.start || !absolute.is_empty() && part.0 == absolute.start
        });
        let last = parts.partition_point(|part| {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.source_views += 1);
            part.0 < absolute.end
        });
        (first..(last + usize::from(last < parts.len())).max(first)).map(move |index| {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.source_views += 1);
            let start = if index == 0 { 0 } else { parts[index - 1].0 };
            let end = parts[index].0.min(absolute.end);
            let begin = start.max(absolute.start);
            (
                &parts[index].1,
                begin - start..end - start,
                begin - absolute.start,
            )
        })
    }

    /// Retire excluded ancestors before retaining a selected window.
    pub(crate) fn select(
        &self,
        mut range: Range<usize>,
        project: fn(&T, Range<usize>) -> Option<Range<usize>>,
    ) -> Self {
        let mut source = self;
        loop {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.source_views += 1);
            if range == (0..source.len()) {
                return source.clone();
            }
            let mut parts = source.parts(range.clone());
            if let Some((part, selected, _)) = parts.next()
                && parts.next().is_none()
            {
                source = part;
                range = selected;
                continue;
            }
            return Self {
                storage: Arc::clone(&source.storage),
                window: source.window.start + range.start..source.window.start + range.end,
                extent: source
                    .native(&range, project)
                    .unwrap_or(source.extent.end..source.extent.end),
            };
        }
    }

    /// Project selected logical bytes through ordered source operands.
    pub(crate) fn native(
        &self,
        range: &Range<usize>,
        project: fn(&T, Range<usize>) -> Option<Range<usize>>,
    ) -> Option<Range<usize>> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.projections += 1);
        if *range == (0..self.len()) {
            return Some(self.extent.clone());
        }
        if let Some((value, window)) = self.leaf() {
            return project(value, window.start + range.start..window.start + range.end);
        }
        let mut parts = self.parts(range.clone());
        let (first, selected, _) = parts.next()?;
        let start = first.native(&selected, project)?.start;
        let end = if let Some((last, selected, _)) = parts.next_back() {
            last.native(&selected, project)?.end
        } else {
            first.native(&selected, project)?.end
        };
        Some(start..end)
    }

    /// Locate native bytes using the ordered native bounds of retained operands.
    pub(crate) fn local(
        &self,
        range: &Range<usize>,
        project: fn(&T, &Range<usize>) -> Option<Range<usize>>,
    ) -> Option<Range<usize>> {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.projections += 1);
        if range.start <= self.extent.start && range.end >= self.extent.end {
            return Some(0..self.len());
        }
        if range.start > self.extent.end || range.end < self.extent.start {
            return None;
        }
        let result = if let Some((value, _)) = self.leaf() {
            project(value, range)?
        } else {
            self.local_parts(range, project)?
        };
        let start = result.start.max(self.window.start);
        let end = result.end.min(self.window.end);
        (start <= end).then(|| start - self.window.start..end - self.window.start)
    }

    /// Search only native-overlapping children, retaining overlapping atomic spellings.
    fn local_parts(
        &self,
        range: &Range<usize>,
        project: fn(&T, &Range<usize>) -> Option<Range<usize>>,
    ) -> Option<Range<usize>> {
        let Storage::Joined(parts) = self.storage.as_ref() else {
            return None;
        };
        let first = parts.partition_point(|(_, part)| {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.projections += 1);
            part.extent.end < range.start
        });
        let last = parts.partition_point(|(_, part)| {
            #[cfg(test)]
            crate::tests::WORK.with_borrow_mut(|work| work.projections += 1);
            part.extent.start <= range.end
        });
        let mut found: Option<Range<usize>> = None;
        for index in first..last {
            let Some(selected) = parts[index].1.local(range, project) else {
                continue;
            };
            let offset = if index == 0 { 0 } else { parts[index - 1].0 };
            let selected = offset + selected.start..offset + selected.end;
            found = Some(found.map_or(selected.start, |old| old.start)..selected.end);
        }
        found
    }

    /// Include operands outside the selected window while their backing vector owns them.
    #[cfg(test)]
    fn owned_parts(&self) -> impl Iterator<Item = &Self> {
        let parts = match self.storage.as_ref() {
            Storage::Leaf(_) => &[][..],
            Storage::Joined(parts) => parts.as_slice(),
        };
        parts.iter().map(|(_, part)| part)
    }

    /// Count ownership depth without invoking recursive diagnostics.
    #[cfg(test)]
    pub(crate) fn depth(&self) -> usize {
        let mut pending = vec![(self, 1)];
        let mut depth = 0;
        while let Some((source, level)) = pending.pop() {
            depth = depth.max(level);
            pending.extend(source.owned_parts().map(|part| (part, level + 1)));
        }
        depth
    }

    /// Charge unique concatenation metadata and backing leaf allocations.
    #[cfg(test)]
    pub(crate) fn retained(
        &self,
        retained: &mut crate::tests::Retained,
        leaf: fn(&T, &mut crate::tests::Retained),
    ) {
        let mut pending = vec![self];
        while let Some(source) = pending.pop() {
            let capacity = match source.storage.as_ref() {
                Storage::Leaf(_) => 0,
                Storage::Joined(parts) => parts.capacity() * std::mem::size_of::<(usize, Self)>(),
            };
            if !retained.arc(&source.storage, capacity) {
                continue;
            }
            if let Some((value, _)) = source.leaf() {
                leaf(value, retained);
            }
            pending.extend(source.owned_parts());
        }
    }
}
