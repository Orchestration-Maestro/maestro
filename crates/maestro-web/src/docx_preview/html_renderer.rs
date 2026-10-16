//! Retained styles and ordered document stylesheet output.
use super::utils::escape_class_name;
use indexmap::IndexMap;
use maestro_app::browser::docx_preview::{
    CommonProperties, IDomStyle, Numbering, ParagraphProperties,
};
use maestro_app::markup::{EscapeMode, escape};
use std::collections::HashSet;
use std::fmt::Write;

/// Prepared document stylesheet state.
pub(crate) struct HtmlRenderer {
    /// Namespace supplied by the caller.
    class_name: String,
    /// Owned occurrences shared by lookup and stylesheet consumers.
    pub(super) styles: Vec<IDomStyle>,
    /// Last occurrence of each nonnull identifier.
    style_map: IndexMap<String, usize>,
}
impl HtmlRenderer {
    /// Retain the authored namespace without conversion.
    pub(crate) fn new(class_name: String) -> Self {
        Self {
            class_name,
            styles: Vec::new(),
            style_map: IndexMap::new(),
        }
    }

    /// Replace retained style occurrences and index their identifiers.
    pub(crate) fn process_styles(&mut self, styles: Vec<IDomStyle>) {
        self.style_map = styles
            .iter()
            .enumerate()
            .filter_map(|(index, style)| style.id.as_ref().map(|id| (id.clone(), index)))
            .collect();
        self.styles = styles
            .iter()
            .enumerate()
            .map(|(index, _)| resolve_style(&styles, index, &self.style_map))
            .collect();
        drop(styles);
    }

    /// Borrow the retained last occurrence for a nonempty exact identifier.
    pub(crate) fn find_style(&self, style_name: Option<&str>) -> Option<&IDomStyle> {
        let name = style_name.filter(|name| !name.is_empty())?;
        self.style_map.get(name).map(|&index| &self.styles[index])
    }

    /// Emit each occurrence's own declarations followed by its resolved link.
    pub(crate) fn render_styles(&self) -> String {
        let defaults: IndexMap<_, _> = self
            .styles
            .iter()
            .enumerate()
            .filter(|(_, style)| style.is_default == Some(Some(true)))
            .map(|(index, style)| (style.target.as_deref(), index))
            .collect();
        self.styles
            .iter()
            .enumerate()
            .map(|(index, style)| {
                self.render_style(
                    style,
                    defaults.get(&style.target.as_deref()) == Some(&index),
                )
            })
            .collect()
    }

    /// Emit one occurrence's own declarations followed by its resolved link.
    fn render_style(&self, style: &IDomStyle, is_default: bool) -> String {
        let mut result = String::new();
        let linked = self.find_style(style.linked.as_ref().and_then(|v| v.as_deref()));
        for substyle in style
            .styles
            .iter()
            .chain(linked.into_iter().flat_map(|s| &s.styles))
        {
            let mut selector = format!(
                "{}.{}",
                style.target.as_deref().unwrap_or_default(),
                self.process_style_name(style.id.as_deref())
            );
            if style.target.as_deref() != Some(&substyle.target) {
                selector.push(' ');
                selector.push_str(&substyle.target);
            }
            if is_default {
                selector = format!(
                    ".{} {}, {selector}",
                    self.class_name,
                    style.target.as_deref().unwrap_or("null")
                );
            }
            result.push_str(&Self::style_to_string(&selector, &substyle.values, None));
        }
        result
    }

    /// Enclose trusted rendered children, escaping only the literal class attribute.
    pub(crate) fn render_wrapper(&self, children_html: &str) -> String {
        format!(
            "<div class=\"{}-wrapper\">{children_html}</div>",
            escape(&self.class_name, EscapeMode::HtmlDecimal)
        )
    }

    /// Emit wrapper, page and document CSS with the supplied print policy.
    pub(crate) fn render_default_style(&self, hide_wrapper_on_print: bool) -> String {
        let c = &self.class_name;
        let mut wrapper = format!(
            r"
.{c}-wrapper {{ background: gray; padding: 30px; padding-bottom: 0px; display: flex; flex-flow: column; align-items: center; }} 
.{c}-wrapper>section.{c} {{ background: white; box-shadow: 0 0 10px rgba(0, 0, 0, 0.5); margin-bottom: 30px; }}"
        );
        if hide_wrapper_on_print {
            wrapper = format!("@media not print {{ {wrapper} }}");
        }
        format!(
            r"{wrapper}
.{c} {{ color: black; hyphens: auto; text-underline-position: from-font; }}
section.{c} {{ box-sizing: border-box; display: flex; flex-flow: column nowrap; position: relative; overflow: hidden; }}
section.{c}>article {{ margin-bottom: auto; z-index: 1; }}
section.{c}>footer {{ z-index: 1; }}
.{c} table {{ border-collapse: collapse; }}
.{c} table td, .{c} table th {{ vertical-align: top; }}
.{c} p {{ margin: 0pt; min-height: 1em; }}
.{c} span {{ white-space: pre-wrap; overflow-wrap: break-word; }}
.{c} a {{ color: inherit; text-decoration: inherit; }}
.{c} svg {{ fill: transparent; }}
"
        )
    }

    /// Fill absent destination declarations in source or requested order.
    pub(crate) fn copy_style_properties(
        input: Option<&IndexMap<String, Option<String>>>,
        output: &mut Option<IndexMap<String, Option<String>>>,
        attrs: Option<&[&str]>,
    ) {
        let Some(input) = input else { return };
        let output = output.get_or_insert_default();
        let keys = attrs.map_or_else(
            || input.keys().map(String::as_str).collect::<Vec<_>>(),
            <[_]>::to_vec,
        );
        for key in keys {
            if let Some(value) = input.get(key) {
                output
                    .entry(key.to_owned())
                    .or_insert_with(|| value.clone());
            }
        }
    }

    /// Emit one CSS rule with ordered declarations and a literal suffix.
    pub(crate) fn style_to_string(
        selectors: &str,
        values: &IndexMap<String, Option<String>>,
        css_text: Option<&str>,
    ) -> String {
        let mut result = format!("{selectors} {{\r\n");
        for (key, value) in values {
            if !key.starts_with('$') {
                let _ = write!(
                    result,
                    "  {key}: {};\r\n",
                    value.as_deref().unwrap_or("null")
                );
            }
        }
        result.push_str(css_text.unwrap_or_default());
        result.push_str("}\r\n");
        result
    }

    /// Prefix a nonempty style identifier with the retained namespace.
    pub(crate) fn process_style_name(&self, class_name: Option<&str>) -> String {
        class_name.filter(|name| !name.is_empty()).map_or_else(
            || self.class_name.clone(),
            |name| format!("{}_{}", self.class_name, escape_class_name(name)),
        )
    }
}

/// Resolve one occurrence from unchanged definitions, retaining each first-visited ancestor.
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

/// Fill a scalar only when the child has no encountered value.
fn inherit<T: Clone>(child: &mut Option<T>, base: Option<&T>) {
    if child.is_none() {
        *child = base.cloned();
    }
}

/// Copy an absent owned group or fill its missing nested fields.
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

/// Fill CSS targets and typed metadata from one unchanged ancestor.
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
            .find(|s| s.target == inherited.target)
        {
            let mut values = Some(std::mem::take(&mut target.values));
            HtmlRenderer::copy_style_properties(Some(&inherited.values), &mut values, None);
            target.values = values.unwrap_or_default();
        } else {
            child.styles.push(inherited.clone());
        }
    }
}

/// Fill shared color and size without overwriting explicit nulls.
fn inherit_common(child: &mut CommonProperties, base: &CommonProperties) {
    inherit(&mut child.color, base.color.as_ref());
    inherit(&mut child.font_size, base.font_size.as_ref());
}

/// Fill independent numbering fields.
fn inherit_numbering(child: &mut Numbering, base: &Numbering) {
    inherit(&mut child.id, base.id.as_ref());
    inherit(&mut child.level, base.level.as_ref());
}

/// Fill paragraph metadata by the same child-first rule as CSS.
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
