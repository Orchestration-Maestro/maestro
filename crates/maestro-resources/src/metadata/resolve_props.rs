//! Validate property and end token lists, then decode authored tags.

use super::{
    FrontmatterError,
    composer::Composer,
    cst::{Node, SourceToken, TokenKind as K},
};

/// Scalar resolution selected by a tag; collections keep their data shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tag {
    /// Integer resolution.
    Int,
    /// Floating-point resolution.
    Float,
    /// Boolean resolution.
    Bool,
    /// Null resolution.
    Null,
    /// Core data tag whose scalar keeps its untagged resolution.
    Data,
    /// A text, local, unknown or non-specific tag.
    Text,
}

impl Tag {
    /// Select the resolution named by a standard schema suffix.
    fn core(suffix: &str) -> Self {
        match suffix {
            "int" => Self::Int,
            "float" => Self::Float,
            "bool" => Self::Bool,
            "null" => Self::Null,
            "binary" | "set" | "timestamp" | "omap" | "pairs" => Self::Data,
            _ => Self::Text,
        }
    }
}

/// Properties attached to the following complete node.
#[derive(Default)]
pub(super) struct Properties {
    /// Optional anchor name to publish on completion.
    pub anchor: Option<String>,
    /// Optional explicit scalar resolution.
    pub tag: Option<Tag>,
    /// Whether the requested collection indicator was present.
    pub found: Option<SourceToken>,
    /// Whether a comma was present.
    pub comma: Option<SourceToken>,
    /// Whether the token list includes a newline.
    pub newline: bool,
    /// Whether a newline follows the last property.
    pub newline_after_prop: bool,
    /// First property or content position.
    pub start: usize,
    /// Byte after the final property token.
    pub end: usize,
}

/// Context for validation of a collection's property list.
#[derive(Clone, Copy)]
pub(super) struct PropertyContext {
    /// Accepted structural indicator.
    pub indicator: K,
    /// Whether commas and flow indentation are allowed.
    pub flow: bool,
    /// Whether the list starts at a line boundary.
    pub newline: bool,
    /// Parent collection indentation.
    pub indent: usize,
}

/// Separation and indentation while traversing a property list.
struct PropertyState {
    /// Whether separation precedes the next comment.
    space: bool,
    /// Whether a line boundary precedes the next token.
    at_newline: bool,
    /// Whether the preceding property requires separation.
    require_space: bool,
    /// Unsettled indentation tab and its indentation.
    tab: Option<(usize, usize)>,
}

impl Composer<'_> {
    /// Validate one property list without searching beyond any token.
    pub fn resolve_props(
        &self,
        tokens: &[SourceToken],
        context: PropertyContext,
        next: Option<&Node>,
    ) -> Result<Properties, FrontmatterError> {
        let context = PropertyContext {
            flow: context.flow
                || context.indicator == K::DocStart
                    && next.is_some_and(|node| {
                        matches!(node.kind, super::cst::NodeKind::FlowCollection { .. })
                    }),
            ..context
        };
        let mut props = Properties::default();
        let mut state = PropertyState {
            space: context.newline,
            at_newline: context.newline,
            require_space: false,
            tab: None,
        };
        for token in tokens {
            self.resolve_prop_token(token, &mut props, &mut state, context)?;
        }
        props.end = tokens.last().map_or(0, |token| token.end);
        if props.anchor.is_none() && props.tag.is_none() {
            props.start = props.end;
        }
        if state.require_space && next.is_some() {
            return Err(self.error_at(
                next.map_or(props.end, |node| node.offset),
                "Expected separation after node property",
            ));
        }
        if let Some((offset, indent)) = state.tab
            && (state.at_newline && indent <= context.indent
                || next.is_some_and(|node| {
                    matches!(
                        node.kind,
                        super::cst::NodeKind::BlockMap(_) | super::cst::NodeKind::BlockSequence(_)
                    )
                }))
        {
            return Err(self.error_at(offset, "Tabs cannot indent block nodes"));
        }
        Ok(props)
    }

    /// Validate and classify one property token.
    fn resolve_prop_token(
        &self,
        token: &SourceToken,
        props: &mut Properties,
        state: &mut PropertyState,
        context: PropertyContext,
    ) -> Result<(), FrontmatterError> {
        if state.require_space && !matches!(token.kind, K::Space | K::Newline | K::Comma) {
            return Err(self.error_at(token.offset, "Expected separation after node property"));
        }
        state.require_space = false;
        if let Some((offset, _)) = state.tab.take()
            && state.at_newline
            && !matches!(token.kind, K::Comment | K::Newline)
        {
            return Err(self.error_at(offset, "Tabs cannot indent block nodes"));
        }
        match token.kind {
            K::Space => {
                if !context.flow && self.text(token).contains('\t') {
                    state.tab = Some((token.offset, token.indent));
                }
                state.space = true;
            }
            K::Comment => {
                if !state.space {
                    return Err(self.error_at(token.offset, "Expected separation before comment"));
                }
                state.at_newline = false;
            }
            K::Newline => {
                state.at_newline = true;
                props.newline = true;
                state.space = true;
                props.newline_after_prop = props.anchor.is_some() || props.tag.is_some();
            }
            K::Anchor | K::Tag => {
                if props.anchor.is_none() && props.tag.is_none() {
                    props.start = token.offset;
                }
                self.resolve_property(token, props)?;
                state.at_newline = false;
                state.space = false;
                state.require_space = true;
                props.newline_after_prop = false;
            }
            kind if kind == context.indicator => {
                if props.anchor.is_some() || props.tag.is_some() || props.found.is_some() {
                    return Err(self.error_at(token.offset, "Unexpected collection indicator"));
                }
                props.found = Some(token.clone());
                state.at_newline = matches!(kind, K::SeqItemInd | K::ExplicitKeyInd);
                state.space = false;
            }
            K::Comma if context.flow && props.comma.is_none() => {
                props.comma = Some(token.clone());
                state.at_newline = false;
                state.space = false;
            }
            _ => return Err(self.error_at(token.offset, "Unexpected property token")),
        }
        Ok(())
    }

    /// Resolve exactly one anchor or tag token.
    fn resolve_property(
        &self,
        token: &SourceToken,
        props: &mut Properties,
    ) -> Result<(), FrontmatterError> {
        if token.kind == K::Anchor {
            if props.anchor.is_some() {
                return Err(self.error_at(token.offset, "Multiple anchors on one node"));
            }
            let name = self.text(token).strip_prefix('&').unwrap_or_default();
            if name.is_empty() {
                return Err(self.error_at(token.offset, "Missing anchor name"));
            }
            props.anchor = Some(name.into());
        } else {
            if props.tag.is_some() {
                return Err(self.error_at(token.offset, "Multiple tags on one node"));
            }
            props.tag = Some(self.tag(token)?);
        }
        Ok(())
    }

    /// Validate trailing trivia and required comment separation.
    pub fn resolve_end(
        &self,
        tokens: &[SourceToken],
        initial_space: bool,
    ) -> Result<(), FrontmatterError> {
        let mut space = initial_space;
        for token in tokens {
            match token.kind {
                K::Space | K::Newline => space = true,
                K::Comment if space => (),
                K::Comment => {
                    return Err(self.error_at(token.offset, "Expected separation before comment"));
                }
                _ => return Err(self.error_at(token.offset, "Unexpected content after YAML value")),
            }
        }
        Ok(())
    }

    /// Decode shorthand tag suffixes as strict UTF-8.
    fn tag(&self, token: &SourceToken) -> Result<Tag, FrontmatterError> {
        let text = self.text(token);
        if let Some(uri) = text.strip_prefix("!<") {
            let uri = uri
                .strip_suffix('>')
                .filter(|uri| !matches!(*uri, "" | "!" | "!!"))
                .ok_or_else(|| self.error_at(token.offset, "Invalid verbatim tag"))?;
            return Ok(uri
                .strip_prefix("tag:yaml.org,2002:")
                .map_or(Tag::Text, Tag::core));
        }
        if let Some(suffix) = text.strip_prefix("!!") {
            if !valid_percent_escapes(suffix) {
                return Err(self.error_at(token.offset, "Invalid tag URI escape"));
            }
            let decoded = percent_encoding::percent_decode_str(suffix)
                .decode_utf8()
                .map_err(|_| self.error_at(token.offset, "Invalid UTF-8 in tag URI"))?;
            return Ok(Tag::core(&decoded));
        }
        if text.strip_prefix('!').unwrap_or_default().contains('!') {
            return Err(self.error_at(token.offset, "Unbound tag handle"));
        }
        Ok(Tag::Text)
    }
}

/// Require complete hexadecimal triplets before invoking the URI decoder.
fn valid_percent_escapes(suffix: &str) -> bool {
    let mut bytes = suffix.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%'
            && !(bytes.next().is_some_and(|b| b.is_ascii_hexdigit())
                && bytes.next().is_some_and(|b| b.is_ascii_hexdigit()))
        {
            return false;
        }
    }
    true
}
