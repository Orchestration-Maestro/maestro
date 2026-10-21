//! Balanced persistent concatenations shared by text and source projections.
use std::{ops::Range, sync::Arc};

/// A selected logical window with cached native bounds.
#[derive(Clone, Debug)]
pub(crate) struct Segments<T> {
    /// Shared leaf or balanced pair of source operands.
    storage: Arc<Storage<T>>,
    /// Cached ownership height; selected windows occur only on leaves.
    height: usize,
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
    Joined([(usize, Segments<T>); 2]),
}

impl<T: Clone> Segments<T> {
    /// Admit one operand with its logical length and native extent.
    pub(crate) fn new(value: T, length: usize, extent: Range<usize>) -> Self {
        Self {
            storage: Arc::new(Storage::Leaf(value)),
            height: 1,
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

    /// Join roots while preserving logarithmic ownership independently of snapshots.
    pub(crate) fn append(&mut self, other: &Self) {
        if other.len() != 0 {
            *self = Self::join(self.clone(), other.clone());
        }
    }

    /// Build one parent from balanced operands with adjacent logical offsets.
    fn branch(left: Self, right: Self) -> Self {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.source_views += 1);
        let length = left.len() + right.len();
        let extent = left.extent.start..right.extent.end;
        let height = 1 + left.height.max(right.height);
        Self {
            storage: Arc::new(Storage::Joined([(left.len(), left), (length, right)])),
            height,
            window: 0..length,
            extent,
        }
    }

    /// Descend the taller spine before restoring AVL balance.
    fn join(left: Self, right: Self) -> Self {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.source_views += 1);
        if left.len() == 0 {
            return right;
        }
        if right.len() == 0 {
            return left;
        }
        if left.height > right.height + 1
            && let Storage::Joined([(_, a), (_, b)]) = left.storage.as_ref()
        {
            return Self::balance(a.clone(), Self::join(b.clone(), right));
        }
        if right.height > left.height + 1
            && let Storage::Joined([(_, a), (_, b)]) = right.storage.as_ref()
        {
            return Self::balance(Self::join(left, a.clone()), b.clone());
        }
        Self::branch(left, right)
    }

    /// Rotate a joined spine using the inner child's height to choose a double turn.
    fn balance(left: Self, right: Self) -> Self {
        if left.height > right.height + 1
            && let Storage::Joined([(_, a), (_, b)]) = left.storage.as_ref()
        {
            if a.height < b.height
                && let Storage::Joined([(_, c), (_, d)]) = b.storage.as_ref()
            {
                return Self::branch(
                    Self::branch(a.clone(), c.clone()),
                    Self::branch(d.clone(), right),
                );
            }
            return Self::branch(a.clone(), Self::branch(b.clone(), right));
        }
        if right.height > left.height + 1
            && let Storage::Joined([(_, a), (_, b)]) = right.storage.as_ref()
        {
            if b.height < a.height
                && let Storage::Joined([(_, c), (_, d)]) = a.storage.as_ref()
            {
                return Self::branch(
                    Self::branch(left, c.clone()),
                    Self::branch(d.clone(), b.clone()),
                );
            }
            return Self::branch(Self::branch(left, a.clone()), b.clone());
        }
        Self::branch(left, right)
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

    /// Clip leaves and rebuild only the selected balanced spines.
    pub(crate) fn select(
        &self,
        range: Range<usize>,
        project: fn(&T, Range<usize>) -> Option<Range<usize>>,
    ) -> Self {
        #[cfg(test)]
        crate::tests::WORK.with_borrow_mut(|work| work.source_views += 1);
        if range == (0..self.len()) {
            return self.clone();
        }
        match self.storage.as_ref() {
            Storage::Leaf(_) => Self {
                storage: Arc::clone(&self.storage),
                height: 1,
                window: self.window.start + range.start..self.window.start + range.end,
                extent: self
                    .native(&range, project)
                    .unwrap_or(self.extent.end..self.extent.end),
            },
            Storage::Joined([(split, left), (_, right)]) => {
                if range.end <= *split {
                    left.select(range, project)
                } else if range.start >= *split {
                    right.select(range.start - split..range.end - split, project)
                } else {
                    Self::join(
                        left.select(range.start..*split, project),
                        right.select(0..range.end - split, project),
                    )
                }
            }
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

    /// Visit the two owned children of a branch.
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
            if !retained.arc(&source.storage, 0) {
                continue;
            }
            if let Some((value, _)) = source.leaf() {
                leaf(value, retained);
            }
            pending.extend(source.owned_parts());
        }
    }
}
