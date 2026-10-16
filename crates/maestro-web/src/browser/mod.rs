//! Browser-owned attachment content effects.
pub mod attachment_content;

/// Cancel-on-drop browser timers.
pub mod timer;

/// Native sandbox values.
#[cfg(target_arch = "wasm32")]
pub mod sandbox_value;

/// Retained browser event subscriptions.
#[cfg(target_arch = "wasm32")]
pub(crate) mod event_subscription;

/// Append the selected pack's escaped custom-property declarations to native inline CSS.
#[cfg(target_arch = "wasm32")]
pub(crate) fn append_brand_properties(
    style: &mut String,
    presentation: &maestro_theme::BrandPresentation<'_>,
) {
    use std::fmt::Write as _;
    for (key, value) in presentation.css_properties() {
        let _ = write!(style, "{key}:{value};");
    }
}
