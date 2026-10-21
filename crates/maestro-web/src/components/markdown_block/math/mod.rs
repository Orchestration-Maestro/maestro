//! HTML math adaptation at native typed registration seams.
use katex::dom_tree::SymbolNode;
use katex::parser::parse_node::NodeType;
use katex::tree::VirtualNode as _;
use katex::types::ParseErrorKind;
use katex::{KatexContext, ParseError, Settings};
use std::sync::{Arc, LazyLock, Mutex};
mod environment;
mod macro_expander;
mod sizing;
mod sqrt;
/// Immutable native callbacks before registration overrides.
static ORIGINAL: LazyLock<KatexContext> = LazyLock::new(KatexContext::default);

/// Register the typed HTML and environment adapters.
pub(super) fn context() -> KatexContext {
    let mut ctx = KatexContext::default();
    ctx.html_group_builders.insert(NodeType::Sqrt, sqrt::sqrt);
    ctx.html_group_builders
        .insert(NodeType::Sizing, sizing::sizing);
    for name in ["\\begin", "\\end"] {
        if let Some(spec) = ctx.functions.get_mut(name) {
            Arc::make_mut(spec).handler = Some(environment::environment);
        }
    }
    ctx
}

/// Serialize safe inline error markup through the native DOM owner.
fn render_error(
    error: &ParseError,
    expression: &str,
    settings: &Settings,
) -> Result<String, ParseError> {
    let mut node = katex::build_common::make_span(
        "katex-error",
        vec![SymbolNode::builder().text(expression).build().into()],
        None,
        None,
    );
    node.attributes
        .insert("title".to_owned(), error.to_string());
    node.attributes.insert(
        "style".to_owned(),
        format!("color: {}", settings.error_color),
    );
    node.to_markup()
}

/// Render HTML while preserving parse errors and caller fallback failures.
pub(super) fn render_to_string(
    ctx: &KatexContext,
    expression: &str,
    settings: &Settings,
) -> Result<String, ParseError> {
    let first_error = Arc::new(Mutex::new(None));
    let patched = macro_expander::settings(settings, &first_error);
    let parsed = katex::parse(ctx, expression, &patched);
    let error = first_error
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    let parsed = match (error, parsed) {
        (Some(error), _) => return render_error(&error, expression, settings),
        (None, Ok(tree)) => tree,
        (None, Err(error)) => {
            if matches!(
                *error.kind,
                ParseErrorKind::ExpectedNode {
                    node: NodeType::TextOrd
                }
            ) {
                return Err(error);
            }
            return render_error(&error, expression, settings);
        }
    };
    match katex::build_tree::build_tree(ctx, &parsed, expression, &patched) {
        Ok(tree) => tree.to_markup(),
        Err(error) => render_error(&error, expression, settings),
    }
}

#[cfg(test)]
mod tests;
