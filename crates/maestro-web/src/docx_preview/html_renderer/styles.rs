//! Paragraph style values the pages read, resolved through each style's base chain.
use maestro_app::browser::docx_preview::{IDomStyle, Numbering};
use std::collections::{HashMap, HashSet};

/// Resolved paragraph values of the last style with each nonempty identifier.
#[derive(Default)]
pub(crate) struct Styles {
    /// Resolved values by identifier.
    styles: HashMap<String, ParagraphStyle>,
}

/// Paragraph values a style gives its paragraphs.
pub(crate) struct ParagraphStyle {
    /// Numbering reference, its identifier and level each taken from the nearest style that
    /// sets it.
    pub(crate) numbering: Option<Numbering>,
    /// Whether paragraphs of the style start on a new page.
    pub(crate) page_break_before: bool,
}

impl Styles {
    /// Resolve the last style with each identifier against the unchanged definitions.
    pub(crate) fn process_styles(styles: &[IDomStyle]) -> Self {
        let style_map: HashMap<&str, &IDomStyle> = styles
            .iter()
            .filter_map(|style| Some((style.id.as_deref().filter(|id| !id.is_empty())?, style)))
            .collect();
        Self {
            styles: style_map
                .keys()
                .map(|&id| (id.to_owned(), resolve_style(id, &style_map)))
                .collect(),
        }
    }

    /// Borrow the resolved values for a style name.
    pub(crate) fn find_style(&self, style_name: Option<&str>) -> Option<&ParagraphStyle> {
        self.styles.get(style_name?)
    }
}

/// Resolve one style, each value taken from the first style along its base chain that sets it;
/// the chain stops at its first repeated style.
fn resolve_style(id: &str, style_map: &HashMap<&str, &IDomStyle>) -> ParagraphStyle {
    let mut numbering: Option<Numbering> = None;
    let mut page_break_before = None;
    let mut visited = HashSet::new();
    let mut current = Some(id);
    while let Some(style) = current
        .filter(|id| visited.insert(*id))
        .and_then(|id| style_map.get(id))
    {
        if let Some(props) = &style.paragraph_props {
            page_break_before = page_break_before.or(props.page_break_before);
            if let Some(base) = &props.numbering {
                let numbering = numbering.get_or_insert_default();
                numbering.id = numbering.id.take().or_else(|| base.id.clone());
                numbering.level = numbering.level.or(base.level);
            }
        }
        current = style.based_on.as_deref();
    }
    ParagraphStyle {
        numbering,
        page_break_before: page_break_before == Some(true),
    }
}
