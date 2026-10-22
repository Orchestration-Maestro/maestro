//! Header and footer selection and placement on a page.
use super::HtmlRenderer;
use crate::docx_preview::html_element::HtmlElement;
use maestro_app::browser::docx_preview::{FooterHeaderReference, PartKind, SectionProperties};

impl<'a> HtmlRenderer<'a> {
    /// Render the header or footer a page uses: the first-page reference on the first page of
    /// a section with a title page, the even reference on even pages, otherwise the default.
    pub(super) fn render_header_footer(
        &mut self,
        refs: Option<&'a [FooterHeaderReference]>,
        props: &'a SectionProperties,
        page: usize,
        first_of_section: bool,
    ) -> Option<HtmlElement> {
        let refs = refs?;
        let find = |kind: &str| refs.iter().find(|r| r.r#type.as_deref() == Some(kind));
        let reference = (props.title_page == Some(true) && first_of_section)
            .then(|| find("first"))
            .flatten()
            .or_else(|| (page % 2 == 1).then(|| find("even")).flatten())
            .or_else(|| find("default"))?;
        let (kind, tree) = self.preview.header_footer(reference.id.as_deref()?)?;
        let header = kind == PartKind::Header;
        let mut element = HtmlElement::new(if header { "header" } else { "footer" });
        element.append(self.render_elements(tree, &tree.roots));
        if let Some(margins) = &props.page_margins {
            let (edge, offset, margin) = if header {
                (&margins.top, &margins.header, "margin-top")
            } else {
                (&margins.bottom, &margins.footer, "margin-bottom")
            };
            // An offset with a missing operand is invalid CSS, which a page never applies.
            if let (Some(edge), Some(offset)) = (edge, offset) {
                element.set_style(margin, Some(&format!("calc({offset} - {edge})")));
                element.set_style("min-height", Some(&format!("calc({edge} - {offset})")));
            }
        }
        Some(element)
    }
}
