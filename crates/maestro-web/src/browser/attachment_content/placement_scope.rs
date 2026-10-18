//! Non-cloneable native placement and activation lifetimes.
/// An owned registry operation revoked when its placement lifetime ends.
#[derive(Default)]
pub struct PlacementScope {
    /// Detach owned placements or restore a nested activation on drop.
    release: Option<Box<dyn FnOnce()>>,
}
impl PlacementScope {
    /// Retain a registry's placement-lifetime revocation.
    #[must_use]
    pub fn new(release: impl FnOnce() + 'static) -> Self {
        Self {
            release: Some(Box::new(release)),
        }
    }
}
impl Drop for PlacementScope {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            release();
        }
    }
}
