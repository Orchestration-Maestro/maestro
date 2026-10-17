//! Typed failures shared by base and command context witnesses.
#![cfg(test)]
/// A caller error whose marker must survive forwarding.
#[derive(Debug)]
pub struct Marker<T>(pub T);
impl<T: std::fmt::Display> std::fmt::Display for Marker<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
impl<T: std::fmt::Display + std::fmt::Debug> std::error::Error for Marker<T> {}
