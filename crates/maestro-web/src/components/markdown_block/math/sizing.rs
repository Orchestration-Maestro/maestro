//! Explicit-size HTML fragments.
use katex::dom_tree::HtmlDomNode;
use katex::options::Options;
use katex::parser::parse_node::{NodeType, ParseNode};
use katex::types::ParseErrorKind;
use katex::{KatexContext, ParseError};
/// Preserve the current display style while changing explicit size.
pub(super) fn sizing(
    node: &ParseNode,
    options: &Options,
    ctx: &KatexContext,
) -> Result<HtmlDomNode, ParseError> {
    let ParseNode::Sizing(sizing) = node else {
        return Err(ParseError::new(ParseErrorKind::ExpectedNode {
            node: NodeType::Sizing,
        }));
    };
    let mut adjusted = options.having_size(sizing.size);
    adjusted.style = options.style.text();
    sizing_group(ctx, &sizing.body, &adjusted, options)
}

/// Rescale the native expression fragment into its containing size.
fn sizing_group(
    ctx: &KatexContext,
    nodes: &[ParseNode],
    options: &Options,
    base: &Options,
) -> Result<HtmlDomNode, ParseError> {
    let mut inner = katex::build_html::build_expression(
        ctx,
        nodes,
        options,
        katex::build_html::GroupType::False,
        (None, None),
    )?;
    let multiplier = options.size_multiplier / base.size_multiplier;
    for item in &mut inner {
        #[cfg(test)]
        super::tests::record_size_visit();
        sizing_classes(item, options, base);
        if let Some(height) = item.height_mut() {
            *height *= multiplier;
        }
        if let Some(depth) = item.depth_mut() {
            *depth *= multiplier;
        }
    }
    Ok(katex::build_common::make_fragment(inner).into())
}

/// Keep a nested size reset relative to the containing expression.
fn sizing_classes(item: &mut HtmlDomNode, options: &Options, base: &Options) {
    let Some(classes) = item.classes_mut() else {
        return;
    };
    let Some(index) = classes.iter().position(|name| name == "katex-sizing") else {
        classes.extend(options.sizing_classes(base));
        return;
    };
    let reset = format!("reset-size{}", options.size);
    if let Some(class) = classes
        .get_mut(index + 1)
        .filter(|class| **class == reset.as_str())
    {
        *class = format!("reset-size{}", base.size).into();
    }
}
