//! Node properties and strict shorthand tag suffix decoding.

use super::{FrontmatterError, parser::Parser};

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
}

impl<'a> Parser<'a> {
    /// Consume node properties in either legal order.
    pub fn properties(&mut self) -> Result<Properties, FrontmatterError> {
        let mut props = Properties::default();
        loop {
            match self.cursor.peek() {
                Some('&') if props.anchor.is_some() => {
                    return Err(self.cursor.error("Multiple anchors on one node"));
                }
                Some('!') if props.tag.is_some() => {
                    return Err(self.cursor.error("Multiple tags on one node"));
                }
                Some('&') => props.anchor = Some(self.anchor_name()?),
                Some('!') => props.tag = Some(self.tag()?),
                _ => break,
            }
            if self
                .cursor
                .peek()
                .is_some_and(|c| !matches!(c, ' ' | '\t' | '\n'))
            {
                return Err(self.cursor.error("Expected separation after node property"));
            }
            self.cursor.spaces();
        }
        Ok(props)
    }

    /// Require a nonempty anchor name after its marker.
    fn anchor_name(&mut self) -> Result<String, FrontmatterError> {
        self.cursor.next();
        let name = self.property_name();
        if name.is_empty() {
            return Err(self.cursor.error("Missing anchor name"));
        }
        Ok(name.into())
    }

    /// Consume a name without flow delimiters or lexical separation.
    pub fn property_name(&mut self) -> &'a str {
        let start = self.cursor.offset;
        while self
            .cursor
            .peek()
            .is_some_and(|c| !matches!(c, ' ' | '\t' | '\n' | ',' | '[' | ']' | '{' | '}'))
        {
            self.cursor.next();
        }
        &self.cursor.source[start..self.cursor.offset]
    }

    /// Decode standard shorthand suffixes, leaving verbatim and local names alone.
    fn tag(&mut self) -> Result<Tag, FrontmatterError> {
        self.cursor.next();
        if self.cursor.eat('<') {
            return self.verbatim_tag();
        }
        if self.cursor.eat('!') {
            let suffix = self.property_name();
            if !valid_percent_escapes(suffix) {
                return Err(self.cursor.error("Invalid tag URI escape"));
            }
            let decoded = percent_encoding::percent_decode_str(suffix)
                .decode_utf8()
                .map_err(|_| self.cursor.error("Invalid UTF-8 in tag URI"))?;
            return Ok(Tag::core(&decoded));
        }
        if self.property_name().contains('!') {
            return Err(self.cursor.error("Unbound tag handle"));
        }
        Ok(Tag::Text)
    }

    /// Read a verbatim URI without applying shorthand percent decoding.
    fn verbatim_tag(&mut self) -> Result<Tag, FrontmatterError> {
        let start = self.cursor.offset;
        while self
            .cursor
            .peek()
            .is_some_and(|c| !matches!(c, '>' | '\n' | ' ' | '\t'))
        {
            self.cursor.next();
        }
        let uri = &self.cursor.source[start..self.cursor.offset];
        if !self.cursor.eat('>') || matches!(uri, "" | "!" | "!!") {
            return Err(self.cursor.error("Invalid verbatim tag"));
        }
        Ok(uri
            .strip_prefix("tag:yaml.org,2002:")
            .map_or(Tag::Text, Tag::core))
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
