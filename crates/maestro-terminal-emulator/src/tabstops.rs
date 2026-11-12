//! Tab destinations retained independently of the visible width.

/// Tab positions with a separately managed visible column count.
pub(crate) struct Tabstops {
    /// Number of columns currently managed.
    cols: usize,
    /// Retained positions, including positions hidden by shrinking.
    stops: Vec<bool>,
}

impl Tabstops {
    /// Initialize interval positions for the supplied width.
    pub(crate) fn init(cols: usize, interval: usize) -> Self {
        let mut tabs = Self {
            cols: 0,
            stops: Vec::new(),
        };
        tabs.resize(cols);
        tabs.reset(interval);
        tabs
    }

    /// Change the managed width without discarding or inventing positions.
    pub(crate) fn resize(&mut self, cols: usize) {
        self.stops.resize(self.stops.len().max(cols), false);
        self.cols = cols;
    }

    /// Read a retained column's tab position.
    pub(crate) fn get(&self, col: usize) -> bool {
        self.stops[col]
    }

    /// Set a retained column's tab position.
    pub(crate) fn set(&mut self, col: usize) {
        self.stops[col] = true;
    }

    /// Clear a retained column's tab position, including an absent position.
    pub(crate) fn unset(&mut self, col: usize) {
        self.stops[col] = false;
    }

    /// Clear all positions and set interval stops before the final column.
    pub(crate) fn reset(&mut self, interval: usize) {
        self.stops.fill(false);
        if interval == 0 || self.cols <= 1 {
            return;
        }
        for col in (interval..self.cols - 1).step_by(interval) {
            self.set(col);
        }
    }
}

#[cfg(test)]
mod tests;
