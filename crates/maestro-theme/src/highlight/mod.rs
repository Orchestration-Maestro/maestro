//! Surface-specific native language recognition.
mod core;
mod languages;

/// The ordered language policy of an HTML surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightProfile {
    /// Offline exported sessions.
    ExportViewer,
    /// Browser code blocks.
    CodeBlock,
}

#[cfg(test)]
mod tests;

/// Resolve a label before asking the detector to select from the surface registry.
pub(crate) fn select(
    code: &str,
    lang: Option<&str>,
    profile: HighlightProfile,
) -> Result<Option<&'static str>, core::Failure> {
    if let Some(language) =
        lang.and_then(|label| languages::lookup(languages::registered(profile), label))
    {
        return Ok(Some(language.name));
    }
    Ok(core::detect(profile, code)?
        .first()
        .and_then(|score| score.language))
}
