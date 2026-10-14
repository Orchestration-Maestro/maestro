//! Application root discoverability of shared diff rendering.
#![cfg(test)]

#[test]
fn application_diff_exports_preserve_theme_errors() {
    let options = maestro_app::RenderDiffOptions {
        file_path: Some("file.rs".into()),
    };
    let theme = unpublished();
    let error = maestro_app::render_diff("-1 old\n+1 new", &options, &theme).unwrap_err();
    assert_eq!(
        error.to_string(),
        "Theme not initialized. Call initTheme() first."
    );
}

/// Supply the unpublished live-theme slot inferred by the renderer.
fn unpublished<T: Default>() -> T {
    T::default()
}
