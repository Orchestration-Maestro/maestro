//! Fractional native reader-follow transitions.
use crate::browser::attachment_content::ReadingScroll;

/// Follow preference with the last admitted native scroll observation.
pub(super) struct FollowState {
    /// Follow later native content growth.
    pub(super) enabled: bool,
    /// Previous admitted vertical offset.
    last_top: f64,
    /// Previous observed viewport height.
    last_height: f64,
}
impl Default for FollowState {
    fn default() -> Self {
        Self {
            enabled: true,
            last_top: 0.,
            last_height: 0.,
        }
    }
}
impl FollowState {
    /// Apply a real native scroll, retaining the previous top across viewport shrink.
    pub(super) fn observe(&mut self, current: Option<ReadingScroll>) {
        let Some(current) = current else { return };
        if current.client_height < self.last_height {
            self.last_height = current.client_height;
            return;
        }
        let distance = current.height - current.top - current.client_height;
        if current.top != 0. && current.top < self.last_top && distance > 50. {
            self.enabled = false;
        } else if distance < 10. {
            self.enabled = true;
        }
        self.last_top = current.top;
        self.last_height = current.client_height;
    }
}
#[cfg(test)]
mod tests;
