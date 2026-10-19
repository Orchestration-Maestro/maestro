//! Owned raw terminal metadata with borrowed reads.

use super::Terminal;

impl Terminal {
    /// Copy title bytes without decoding or ingress limits; empty bytes unset it.
    pub fn set_title(&mut self, title: &[u8]) {
        title.clone_into(&mut self.title);
    }

    /// Borrow the nonempty title bytes until the next mutable terminal operation.
    #[must_use]
    pub fn get_title(&self) -> Option<&[u8]> {
        (!self.title.is_empty()).then_some(self.title.as_slice())
    }

    /// Copy directory bytes without decoding or ingress limits; empty bytes unset it.
    pub fn set_pwd(&mut self, pwd: &[u8]) {
        pwd.clone_into(&mut self.pwd);
    }

    /// Borrow the nonempty directory bytes until the next mutable terminal operation.
    #[must_use]
    pub fn get_pwd(&self) -> Option<&[u8]> {
        (!self.pwd.is_empty()).then_some(self.pwd.as_slice())
    }
}
