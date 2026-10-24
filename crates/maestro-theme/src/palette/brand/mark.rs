//! Native selection and reading of mark templates.
use super::{BrandMode, BrandPack};
use crate::palette::ThemeError;
use crate::palette::loading::read_text;

impl BrandPack {
    /// Read the default template (`None`) or the named variant and fill in `mode`'s colors.
    ///
    /// Substitution follows [`maestro_brand::BrandPresentation::fill_mark`].
    ///
    /// # Errors
    /// Returns `Unknown brand mark variant` before any read, the failures of [`BrandPack::presentation`], the
    /// template read failure, `Missing brand color role` or `Unterminated brand template color`.
    pub fn mark_svg(&self, mode: BrandMode, variant: Option<&str>) -> Result<String, ThemeError> {
        let mark = &self.data.mark;
        let path = match variant {
            None => &mark.template,
            Some(name) => mark.variants.get(name).ok_or_else(|| {
                ThemeError::message(format!("Unknown brand mark variant: {name}"))
            })?,
        };
        let presentation = self.presentation(mode)?;
        let template = read_text(path).map_err(ThemeError::io)?;
        presentation.fill_mark(&template).map_err(ThemeError::from)
    }
}
