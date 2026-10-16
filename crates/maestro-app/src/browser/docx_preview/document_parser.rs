//! Word body framing and selected CSS property parsing.
use super::{
    DocxError, DomType, OpenXmlElement, OpenXmlElementBase, OpenXmlElementData as Data,
    OpenXmlElementTree, OwnedXmlElement, UnparsedKind, XmlElement, XmlParser, parse_xml_string,
};

/// Parse the immediate body elements of an owned XML selection.
///
/// # Errors
/// Returns the existing XML admission error if the source cannot be parsed.
pub fn parse_body_elements(input: &OwnedXmlElement) -> Result<OpenXmlElementTree, DocxError> {
    parse(input, Framing::Body)
}
/// Parse a paragraph with distinct run and parent identities.
///
/// # Errors
/// Returns the existing XML admission error if the source cannot be parsed.
pub fn parse_paragraph(input: &OwnedXmlElement) -> Result<OpenXmlElementTree, DocxError> {
    parse(input, Framing::Paragraph)
}
/// Parse one run, retaining ordered inline content.
///
/// # Errors
/// Returns the existing XML admission error if the source cannot be parsed.
pub fn parse_run(input: &OwnedXmlElement) -> Result<OpenXmlElementTree, DocxError> {
    parse(input, Framing::Run)
}
/// Selected immediate-child framing.
#[derive(Clone, Copy)]
pub(super) enum Framing {
    /// Body children.
    Body,
    /// One paragraph.
    Paragraph,
    /// One run.
    Run,
}
/// Reparse and select the input once for one public operation.
fn parse(input: &OwnedXmlElement, framing: Framing) -> Result<OpenXmlElementTree, DocxError> {
    let xml = parse_xml_string(input.source(), false)?;
    let selected = xml.selected(input)?;
    Ok(parse_selected(&xml, selected, input, framing))
}
/// Construct one arena from an already admitted selection.
pub(super) fn parse_selected<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    input: &'doc OwnedXmlElement,
    framing: Framing,
) -> OpenXmlElementTree {
    let mut parser = DocumentParser {
        xml,
        input,
        tree: OpenXmlElementTree::default(),
    };
    parser.tree.roots = match framing {
        Framing::Body => parser.body(element),
        Framing::Paragraph => vec![parser.paragraph(element)],
        Framing::Run => vec![parser.run(element, None)],
    };
    parser.tree
}
/// One parse's borrowed XML view and owned result arena.
struct DocumentParser<'doc, 'input> {
    /// Reparsed complete XML input.
    xml: &'doc XmlParser<'input>,
    /// Shared source for retained child selections.
    input: &'doc OwnedXmlElement,
    /// Output arena under construction.
    tree: OpenXmlElementTree,
}
impl<'input> DocumentParser<'_, 'input> {
    /// Allocate one node and return its arena position.
    fn push(&mut self, data: Data) -> usize {
        let id = self.tree.nodes.len();
        self.tree.nodes.push(OpenXmlElement {
            base: OpenXmlElementBase::default(),
            data,
        });
        id
    }
    /// Keep a foreign branch with its shared source selection.
    fn retain(&mut self, element: XmlElement<'_, 'input>, kind: UnparsedKind) -> usize {
        self.push(Data::Unparsed {
            kind,
            element: self.input.slice(element),
        })
    }
    /// Parse immediate body children.
    fn body(&mut self, element: XmlElement<'_, 'input>) -> Vec<usize> {
        let mut children = Vec::new();
        for child in self.xml.elements(element, None) {
            match child.local_name() {
                "p" => children.push(self.paragraph(child)),
                "tbl" => children.push(self.retain(child, UnparsedKind::Element(DomType::Table))),
                "sdt" => children.extend(
                    self.xml
                        .element(child, "sdtContent")
                        .map(|c| self.body(c))
                        .unwrap_or_default(),
                ),
                "altChunk" => children.push(self.push(Data::AltChunk {
                    id: self.attr(child, "id"),
                })),
                _ => {}
            }
        }
        children
    }
    /// Build a paragraph whose run parents keep this arena identity.
    fn paragraph(&mut self, element: XmlElement<'_, 'input>) -> usize {
        let id = self.push(Data::Paragraph {
            properties: Box::default(),
        });
        self.tree.nodes[id].base.children = self.paragraph_children(element, id);
        id
    }
    /// Recognize paragraph children using the supplied parent identity.
    fn paragraph_children(&mut self, element: XmlElement<'_, 'input>, parent: usize) -> Vec<usize> {
        let mut children = Vec::new();
        for child in self.xml.elements(element, None) {
            match child.local_name() {
                "r" => children.push(self.run(child, Some(parent))),
                "fldSimple" => {
                    let field = self.push(self.field(child));
                    self.tree.nodes[field].base.children = self.paragraph_children(child, field);
                    children.push(field);
                }
                "pPr" => {
                    properties::parse_paragraph_properties(
                        self.xml,
                        child,
                        &mut self.tree.nodes[parent].data,
                    );
                }
                "sdt" => children.extend(
                    self.xml
                        .element(child, "sdtContent")
                        .map(|c| self.hidden_children(c))
                        .unwrap_or_default(),
                ),
                "ins" | "del" => children.push(self.change(child)),
                name => {
                    let kind = match name {
                        "hyperlink" => DomType::Hyperlink,
                        "smartTag" => DomType::SmartTag,
                        "bookmarkStart" => DomType::BookmarkStart,
                        "bookmarkEnd" => DomType::BookmarkEnd,
                        "commentRangeStart" => DomType::CommentRangeStart,
                        "commentRangeEnd" => DomType::CommentRangeEnd,
                        "oMath" => DomType::MmlMath,
                        "oMathPara" => DomType::MmlMathParagraph,
                        _ => continue,
                    };
                    children.push(self.retain(child, UnparsedKind::Element(kind)));
                }
            }
        }
        children
    }
    /// Build a change wrapper while retaining its hidden paragraph parents.
    fn change(&mut self, element: XmlElement<'_, 'input>) -> usize {
        let children = self.hidden_children(element);
        let data = if element.local_name() == "ins" {
            Data::Inserted
        } else {
            Data::Deleted
        };
        let id = self.push(data);
        self.tree.nodes[id].base.children = children;
        id
    }
    /// Retain a hidden paragraph identity while flattening its child positions.
    fn hidden_children(&mut self, element: XmlElement<'_, 'input>) -> Vec<usize> {
        let id = self.paragraph(element);
        self.tree.nodes[id].base.children.clone()
    }
    /// Copy one nullable decoded attribute into its payload.
    fn attr(&self, element: XmlElement<'_, 'input>, name: &str) -> Option<String> {
        self.xml.attr(element, name).map(str::to_owned)
    }
    /// Read shared field flags with the kind-specific open metadata.
    fn field(&self, element: XmlElement<'_, 'input>) -> Data {
        let lock_name = self
            .xml
            .attr(element, "fldLock")
            .map_or("lock", |_| "fldLock");
        let lock = self
            .xml
            .bool_attr(element, lock_name, Some(false))
            .unwrap_or(false);
        let dirty = self
            .xml
            .bool_attr(element, "dirty", Some(false))
            .unwrap_or(false);
        if element.local_name() == "fldChar" {
            Data::ComplexField {
                char_type: self.attr(element, "fldCharType"),
                lock,
                dirty,
            }
        } else {
            Data::SimpleField {
                instruction: self.attr(element, "instr"),
                lock,
                dirty,
            }
        }
    }
    /// Build a run and its ordered inline text.
    fn run(&mut self, element: XmlElement<'_, 'input>, parent: Option<usize>) -> usize {
        let id = self.push(Data::Run {
            field_run: false,
            properties: super::RunProperties::default(),
        });
        self.tree.nodes[id].base.parent = parent;
        for child in self.xml.elements(element, None) {
            if child.local_name() == "rPr" {
                properties::parse_run_properties(self.xml, child, &mut self.tree.nodes[id].data);
                continue;
            }
            let Some(data) = self.run_child(child) else {
                continue;
            };
            if matches!(data, Data::ComplexField { .. } | Data::Instruction { .. })
                && let Data::Run { field_run, .. } = &mut self.tree.nodes[id].data
            {
                *field_run = true;
            }
            let child = self.push(data);
            self.tree.nodes[id].base.children.push(child);
        }
        id
    }
    /// Construct one recognized inline payload.
    fn run_child(&self, child: XmlElement<'_, 'input>) -> Option<Data> {
        Some(match child.local_name() {
            "t" => Data::Text {
                text: child.text_content(),
            },
            "instrText" => Data::Instruction {
                text: child.text_content(),
            },
            "fldSimple" | "fldChar" => self.field(child),
            "noBreakHyphen" => Data::NoBreakHyphen,
            "tab" => Data::Tab,
            "AlternateContent" | "commentReference" | "drawing" | "pict" => self.raw_run(child),
            "sym" => Data::Symbol {
                font: self
                    .xml
                    .attr(child, "font")
                    .map(super::utils::enclose_font_family),
                char: self.attr(child, "char"),
            },
            "lastRenderedPageBreak" => Data::Break {
                r#break: "lastRenderedPageBreak".into(),
            },
            "footnoteReference" => Data::FootnoteReference {
                id: self.attr(child, "id"),
            },
            "endnoteReference" => Data::EndnoteReference {
                id: self.attr(child, "id"),
            },
            "delText" => Data::DeletedText {
                text: child.text_content(),
            },
            "br" => Data::Break {
                r#break: self
                    .xml
                    .attr(child, "type")
                    .filter(|s| !s.is_empty())
                    .unwrap_or("textWrapping")
                    .into(),
            },
            _ => return None,
        })
    }

    /// Retain an inline foreign branch without selecting alternate content.
    fn raw_run(&self, child: XmlElement<'_, 'input>) -> Data {
        let kind = match child.local_name() {
            "commentReference" => UnparsedKind::Element(DomType::CommentReference),
            "drawing" => UnparsedKind::Element(DomType::Drawing),
            "pict" => UnparsedKind::Element(DomType::VmlPicture),
            _ => UnparsedKind::AlternateContent,
        };
        Data::Unparsed {
            kind,
            element: self.input.slice(child),
        }
    }
}

mod values;
mod xml_util;

/// Update physical border sides from immediate children, retaining existing slots.
pub fn parse_border_properties<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
    output: &mut indexmap::IndexMap<String, Option<String>>,
) {
    for child in xml.elements(element, None) {
        let side = match child.local_name() {
            "start" | "left" => "border-left",
            "end" | "right" => "border-right",
            "top" => "border-top",
            "bottom" => "border-bottom",
            _ => continue,
        };
        output.insert(side.into(), Some(values::value_of_border(xml, child)));
    }
}

/// Return the selected element's background color when present.
#[must_use]
pub fn parse_background<'doc, 'input: 'doc>(
    xml: &'doc XmlParser<'input>,
    element: XmlElement<'doc, 'input>,
) -> indexmap::IndexMap<String, Option<String>> {
    xml_util::color_attr(xml, element, "color", "black")
        .map(|color| ("background-color".into(), Some(color)))
        .into_iter()
        .collect()
}

mod default_properties;
pub use default_properties::{DefaultPropertiesOptions, parse_default_properties};

mod formatting;

mod table_style;
pub use table_style::{TableStyle, parse_table_style};

mod properties;
pub(super) mod styles;
