//! Radical padding through the registered HTML builder.
use super::ORIGINAL;
use katex::dom_tree::{DomSpan, HtmlDomNode};
use katex::options::Options;
use katex::parser::parse_node::{NodeType, ParseNode};
use katex::types::{CssProperty, ParseErrorKind};
use katex::{KatexContext, ParseError};

/// Locate a span through the native builder’s constant-depth child path.
fn span_at<'a>(node: &'a mut HtmlDomNode, path: &[usize]) -> Option<&'a mut DomSpan> {
    let HtmlDomNode::DomSpan(span) = node else {
        return None;
    };
    if let Some((head, tail)) = path.split_first() {
        span_at(span.children.get_mut(*head)?, tail)
    } else {
        Some(span)
    }
}

/// Add missing bare-symbol advance width to the native radical.
pub(super) fn sqrt(
    node: &ParseNode,
    options: &Options,
    ctx: &KatexContext,
) -> Result<HtmlDomNode, ParseError> {
    let original = ORIGINAL
        .html_group_builders
        .get(&NodeType::Sqrt)
        .ok_or_else(|| {
            ParseError::new(ParseErrorKind::ExpectedNode {
                node: NodeType::Sqrt,
            })
        })?;
    let mut tree = original(node, options, ctx)?;
    let HtmlDomNode::DomSpan(ref mut root) = tree else {
        return Ok(tree);
    };
    let Some(body) = root.children.last_mut() else {
        return Ok(tree);
    };
    let Some(aligned) = span_at(body, &[0, 0, 0]) else {
        return Ok(tree);
    };
    let Some(inner) = aligned.children.last_mut() else {
        return Ok(tree);
    };
    if inner
        .style()
        .is_some_and(|s| !s.contains_key(CssProperty::PaddingLeft))
    {
        let theta = options.font_metrics().default_rule_thickness;
        let phi = if options.style.id < katex::style::TEXT.id {
            options.font_metrics().x_height
        } else {
            theta
        };
        let min_height = inner.height() + inner.depth() + theta + phi / 4.0 + theta;
        let image = katex::delimiter::make_sqrt_image(ctx, min_height, options)?;
        if let Some(style) = inner.style_mut() {
            style.insert(
                CssProperty::PaddingLeft,
                katex::units::make_em(image.advance_width),
            );
        }
    }
    Ok(tree)
}
