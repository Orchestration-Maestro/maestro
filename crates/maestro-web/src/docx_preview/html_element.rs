//! Owned rendered elements and their escaped HTML text.
use indexmap::IndexMap;
use maestro_app::markup::{EscapeMode, escape_into};
use std::collections::BTreeMap;

/// One rendered element with ordered attributes, inline declarations and children.
#[derive(Debug)]
pub struct HtmlElement {
    /// Tag name.
    pub(super) tag: &'static str,
    /// Attributes in assignment order; inline declarations are kept apart.
    pub(super) attributes: IndexMap<String, String>,
    /// Inline declarations by CSS property name.
    pub(super) style: BTreeMap<String, String>,
    /// Child elements and text in document order.
    pub(super) children: Vec<HtmlContent>,
}

/// One child of a rendered element.
#[derive(Debug)]
pub(crate) enum HtmlContent {
    /// A nested element.
    Element(HtmlElement),
    /// Literal text, escaped when serialized.
    Text(String),
}

impl HtmlElement {
    /// Start an element without attributes or children.
    pub(crate) fn new(tag: &'static str) -> Self {
        Self {
            tag,
            attributes: IndexMap::new(),
            style: BTreeMap::new(),
            children: Vec::new(),
        }
    }

    /// Serialize the element and its children as HTML text, escaping every value once.
    #[must_use]
    pub fn to_html(&self) -> String {
        let mut out = String::new();
        self.write_html(&mut out);
        out
    }

    /// Append the opening tag, the content and the closing tag.
    fn write_html(&self, out: &mut String) {
        out.push('<');
        out.push_str(self.tag);
        for (name, value) in &self.attributes {
            write_attribute(name, value, out);
        }
        if !self.style.is_empty() {
            let declarations: Vec<_> = self
                .style
                .iter()
                .map(|(property, value)| format!("{property}:{value}"))
                .collect();
            write_attribute("style", &declarations.join(";"), out);
        }
        out.push('>');
        if matches!(self.tag, "br" | "col") {
            return;
        }
        for child in &self.children {
            match child {
                HtmlContent::Element(element) => element.write_html(out),
                HtmlContent::Text(text) => escape_into(text, out, EscapeMode::HtmlDecimal),
            }
        }
        out.push_str("</");
        out.push_str(self.tag);
        out.push('>');
    }

    /// Assign one attribute, replacing an earlier value in place.
    pub(crate) fn set_attribute(&mut self, name: &str, value: impl Into<String>) {
        self.attributes.insert(name.to_owned(), value.into());
    }

    /// Assign one declaration as a style property assignment does: an absent or empty value
    /// removes the property, and a value that would leave its own declaration is rejected.
    pub(crate) fn set_style(&mut self, property: &str, value: Option<impl Into<String>>) {
        match value.map(Into::into).filter(|value| !value.is_empty()) {
            None => {
                self.style.remove(property);
            }
            Some(value) if contained(&value) => {
                self.style.insert(property.to_owned(), value);
            }
            Some(_) => {}
        }
    }

    /// Add one class token unless the class list already holds it.
    pub(crate) fn add_class(&mut self, token: &str) {
        let current = self.attributes.get("class").map_or("", String::as_str);
        let mut tokens: Vec<&str> = Vec::new();
        for existing in current.split_ascii_whitespace().chain([token]) {
            if !tokens.contains(&existing) {
                tokens.push(existing);
            }
        }
        let joined = tokens.join(" ");
        self.set_attribute("class", joined);
    }

    /// Append one child element.
    pub(crate) fn push_element(&mut self, child: Self) {
        self.children.push(HtmlContent::Element(child));
    }

    /// Append child elements in order.
    pub(crate) fn extend(&mut self, children: impl IntoIterator<Item = Self>) {
        self.children
            .extend(children.into_iter().map(HtmlContent::Element));
    }

    /// Append literal text.
    pub(crate) fn push_text(&mut self, text: impl Into<String>) {
        self.children.push(HtmlContent::Text(text.into()));
    }

    /// Append already rendered children in order.
    pub(crate) fn append(&mut self, children: Vec<HtmlContent>) {
        self.children.extend(children);
    }

    /// Borrow a child element by its position among all children.
    pub(crate) fn child_mut(&mut self, index: usize) -> Option<&mut Self> {
        match self.children.get_mut(index) {
            Some(HtmlContent::Element(element)) => Some(element),
            _ => None,
        }
    }
}

/// Append one name and value pair, escaping both.
fn write_attribute(name: &str, value: &str, out: &mut String) {
    out.push(' ');
    escape_into(name, out, EscapeMode::HtmlDecimal);
    out.push_str("=\"");
    escape_into(value, out, EscapeMode::HtmlDecimal);
    out.push('"');
}

/// Whether a declaration value ends where it starts: strings and brackets close, and no
/// separator, block, comment, priority or escape appears outside a string.
///
/// A style property assignment parses each value on its own and rejects one that is not a
/// single value; written into one style attribute, such a value would instead end its
/// declaration early or swallow the declarations after it.
fn contained(value: &str) -> bool {
    let mut quote = None;
    let mut depth = 0_usize;
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        match (quote, character) {
            (Some(_), '\\') => {
                characters.next();
            }
            (Some(_), '\n' | '\r' | '\u{c}') | (None, ';' | '{' | '}' | '!' | '\\') => {
                return false;
            }
            (None, '/') if characters.peek() == Some(&'*') => return false,
            (Some(open), _) if character == open => quote = None,
            (None, '"' | '\'') => quote = Some(character),
            (None, '(' | '[') => depth += 1,
            (None, ')' | ']') => match depth.checked_sub(1) {
                Some(next) => depth = next,
                None => return false,
            },
            _ => {}
        }
    }
    quote.is_none() && depth == 0
}
