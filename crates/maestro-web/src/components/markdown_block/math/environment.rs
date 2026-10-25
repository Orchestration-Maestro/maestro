//! Typed malformed environment-name propagation.
use super::ORIGINAL;
use katex::ParseError;
use katex::parser::parse_node::{NodeType, ParseNode};
use katex::types::ParseErrorKind;

/// Delegate valid names and retain malformed-name failures for the caller.
pub(super) fn environment(
    context: katex::define_function::FunctionContext<'_, '_, '_>,
    args: Vec<ParseNode>,
    optional: Vec<Option<ParseNode>>,
) -> Result<ParseNode, ParseError> {
    if let Some(ParseNode::OrdGroup(group)) = args.first()
        && group
            .body
            .iter()
            .any(|item| !matches!(item, ParseNode::TextOrd(_)))
    {
        return Err(ParseError::new(ParseErrorKind::ExpectedNode {
            node: NodeType::TextOrd,
        }));
    }
    let handler = ORIGINAL
        .functions
        .get(context.func_name)
        .and_then(|s| s.handler)
        .ok_or_else(|| {
            ParseError::new(ParseErrorKind::ExpectedNode {
                node: NodeType::Environment,
            })
        })?;
    handler(context, args, optional)
}
