//! Shared formatting and source delegation for retained operation errors.
/// Implement the common error shape with its owner's presentation source policy.
macro_rules! impl_ui_error {
    ($kind:ty, $presentation:pat => $source:expr) => {
        impl std::fmt::Display for $kind {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    Self::I18n(error) => std::fmt::Display::fmt(error, f),
                    Self::Control(error) => std::fmt::Display::fmt(error, f),
                    Self::Host(error) => std::fmt::Display::fmt(error, f),
                    Self::Presentation(error) => std::fmt::Display::fmt(error, f),
                    Self::BorrowConflict => f.write_str("Attachment is already borrowed"),
                }
            }
        }
        impl std::error::Error for $kind {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                match self {
                    Self::I18n(error) => Some(error),
                    Self::Control(error) => std::error::Error::source(error),
                    Self::Host(error) => Some(error),
                    Self::Presentation($presentation) => $source,
                    Self::BorrowConflict => None,
                }
            }
        }
    };
}
pub(crate) use impl_ui_error;
