//! Resolved styles: each occurrence merged with its base chain, looked up by identifier.
use super::{CssStyle, copy_style_properties};
use indexmap::IndexMap;
use maestro_app::browser::docx_preview::{
    CommonProperties, IDomStyle, Numbering, ParagraphProperties,
};
use std::collections::HashSet;

/// Resolved style occurrences and the last occurrence of each identifier.
#[derive(Default)]
pub(crate) struct Styles {
    /// Resolved occurrences in source order, repeated identifiers included.
    pub(crate) styles: Vec<IDomStyle>,
    /// Last occurrence of each identifier.
    style_map: IndexMap<String, usize>,
}

impl Styles {
    /// Resolve every occurrence against the unchanged definitions and index identifiers.
    pub(crate) fn process_styles(styles: &[IDomStyle]) -> Self {
        let style_map: IndexMap<String, usize> = styles
            .iter()
            .enumerate()
            .filter_map(|(index, style)| style.id.as_ref().map(|id| (id.clone(), index)))
            .collect();
        let resolved = (0..styles.len())
            .map(|index| resolve_style(styles, index, &style_map))
            .collect();
        Self {
            styles: resolved,
            style_map,
        }
    }

    /// Borrow the last occurrence of a nonempty identifier.
    pub(crate) fn find_style(&self, style_name: Option<&str>) -> Option<&IDomStyle> {
        let name = style_name.filter(|name| !name.is_empty())?;
        self.style_map
            .get(name)
            .and_then(|&index| self.styles.get(index))
    }
}

/// Resolve one occurrence, keeping each ancestor's contribution the first time it is visited.
fn resolve_style(
    styles: &[IDomStyle],
    index: usize,
    style_map: &IndexMap<String, usize>,
) -> IDomStyle {
    let mut result = styles[index].clone();
    let mut visited = HashSet::from([index]);
    let mut current = &styles[index];
    while let Some(&base) = current
        .based_on
        .as_deref()
        .filter(|id| !id.is_empty())
        .and_then(|id| style_map.get(id))
    {
        if !visited.insert(base) {
            break;
        }
        current = &styles[base];
        inherit_style(&mut result, current);
    }
    result
}

/// Fill a value only when the child has none.
fn inherit<T: Clone>(child: &mut Option<T>, base: Option<&T>) {
    if child.is_none() {
        *child = base.cloned();
    }
}

/// Copy an absent group or fill the missing fields of a present one.
fn inherit_group<T: Clone>(
    child: &mut Option<T>,
    base: Option<&T>,
    merge: impl FnOnce(&mut T, &T),
) {
    if let (Some(child), Some(base)) = (child.as_mut(), base) {
        merge(child, base);
    } else {
        inherit(child, base);
    }
}

/// Fill declarations and metadata from one ancestor, the child winning.
fn inherit_style(child: &mut IDomStyle, base: &IDomStyle) {
    inherit_group(
        &mut child.paragraph_props,
        base.paragraph_props.as_ref(),
        inherit_paragraph,
    );
    inherit_group(
        &mut child.run_props,
        base.run_props.as_ref(),
        inherit_common,
    );
    for inherited in &base.styles {
        if let Some(target) = child
            .styles
            .iter_mut()
            .find(|style| style.target == inherited.target)
        {
            let mut values: Option<CssStyle> = Some(std::mem::take(&mut target.values));
            copy_style_properties(Some(&inherited.values), &mut values, None);
            target.values = values.unwrap_or_default();
        } else {
            child.styles.push(inherited.clone());
        }
    }
}

/// Fill shared color and size without replacing explicit nulls.
fn inherit_common(child: &mut CommonProperties, base: &CommonProperties) {
    inherit(&mut child.color, base.color.as_ref());
    inherit(&mut child.font_size, base.font_size.as_ref());
}

/// Fill numbering identifier and level independently.
fn inherit_numbering(child: &mut Numbering, base: &Numbering) {
    inherit(&mut child.id, base.id.as_ref());
    inherit(&mut child.level, base.level.as_ref());
}

/// Fill paragraph metadata by the same child-first rule as declarations.
fn inherit_paragraph(child: &mut ParagraphProperties, base: &ParagraphProperties) {
    inherit_common(&mut child.common, &base.common);
    inherit(&mut child.css_style, base.css_style.as_ref());
    inherit(&mut child.tabs, base.tabs.as_ref());
    inherit_group(
        &mut child.numbering,
        base.numbering.as_ref(),
        inherit_numbering,
    );
    inherit(&mut child.line_spacing, base.line_spacing.as_ref());
    inherit(&mut child.text_alignment, base.text_alignment.as_ref());
    inherit(&mut child.keep_lines, base.keep_lines.as_ref());
    inherit(&mut child.keep_next, base.keep_next.as_ref());
    inherit(
        &mut child.page_break_before,
        base.page_break_before.as_ref(),
    );
    inherit(&mut child.outline_level, base.outline_level.as_ref());
    inherit(&mut child.style_name, base.style_name.as_ref());
    inherit_group(
        &mut child.run_props,
        base.run_props.as_ref(),
        inherit_common,
    );
    inherit(&mut child.class_name, base.class_name.as_ref());
}
