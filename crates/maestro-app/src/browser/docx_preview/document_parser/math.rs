//! Typed equation trees sharing the ordinary run owner.
use super::{Data, DocumentParser, DocxError, DomType, XmlElement};

/// Partial equation properties, distinguishing absent fields from missing values.
#[derive(Debug, Default)]
pub struct MathProperties {
    /// Optional nullable operator character.
    pub char: Option<Option<String>>,
    /// Optional nullable vertical justification.
    pub vertical_justification: Option<Option<String>>,
    /// Optional nullable position.
    pub position: Option<Option<String>>,
    /// Optional nullable degree visibility flag.
    pub hide_degree: Option<Option<bool>>,
    /// Optional nullable delimiter opening.
    pub begin_char: Option<Option<String>>,
    /// Optional nullable delimiter closing.
    pub end_char: Option<Option<String>>,
}
/// Select a recognized equation kind by local XML name.
fn math_kind(name: &str) -> Option<DomType> {
    Some(match name {
        "oMath" => DomType::MmlMath,
        "oMathPara" => DomType::MmlMathParagraph,
        "f" => DomType::MmlFraction,
        "func" => DomType::MmlFunction,
        "fName" => DomType::MmlFunctionName,
        "num" => DomType::MmlNumerator,
        "den" => DomType::MmlDenominator,
        "rad" => DomType::MmlRadical,
        "deg" => DomType::MmlDegree,
        "e" => DomType::MmlBase,
        "sSup" => DomType::MmlSuperscript,
        "sSub" => DomType::MmlSubscript,
        "sPre" => DomType::MmlPreSubSuper,
        "sup" => DomType::MmlSuperArgument,
        "sub" => DomType::MmlSubArgument,
        "d" => DomType::MmlDelimiter,
        "nary" => DomType::MmlNary,
        "eqArr" => DomType::MmlEquationArray,
        "lim" => DomType::MmlLimit,
        "limLow" => DomType::MmlLimitLower,
        "m" => DomType::MmlMatrix,
        "mr" => DomType::MmlMatrixRow,
        "box" => DomType::MmlBox,
        "bar" => DomType::MmlBar,
        "groupChr" => DomType::MmlGroupChar,
        _ => return None,
    })
}
impl<'input> DocumentParser<'_, 'input> {
    /// Read immediate property fields, preserving absence and last assignment.
    fn math_properties(&self, element: XmlElement<'_, 'input>) -> MathProperties {
        let mut result = MathProperties::default();
        for child in self.xml.elements(element, None) {
            match child.local_name() {
                "chr" => result.char = Some(self.attr(child, "val")),
                "vertJc" => result.vertical_justification = Some(self.attr(child, "val")),
                "pos" => result.position = Some(self.attr(child, "val")),
                "degHide" => result.hide_degree = Some(self.xml.bool_attr(child, "val", None)),
                "begChr" => result.begin_char = Some(self.attr(child, "val")),
                "endChr" => result.end_char = Some(self.attr(child, "val")),
                _ => {}
            }
        }
        result
    }

    /// Retag a completed shared run without changing its properties or parent.
    fn math_run(&mut self, element: XmlElement<'_, 'input>) -> Result<usize, DocxError> {
        let run = self.run(element, None)?;
        if let Data::Run { r#type, .. } = &mut self.tree.nodes[run].data {
            *r#type = DomType::MmlRun;
        }
        Ok(run)
    }
    /// Recursively parse recognized math children and detached shared runs.
    pub(super) fn math(&mut self, element: XmlElement<'_, 'input>) -> Result<usize, DocxError> {
        let kind = math_kind(element.local_name()).unwrap_or(DomType::MmlMath);
        let id = self.push(Data::Math {
            r#type: kind,
            props: None,
        });
        let mut children = Vec::new();
        let mut props = None;
        let props_tag = format!("{}Pr", element.local_name());
        for child in self.xml.elements(element, None) {
            if math_kind(child.local_name()).is_some() {
                children.push(self.math(child)?);
            } else if child.local_name() == "r" {
                children.push(self.math_run(child)?);
            } else if child.local_name() == props_tag {
                props = Some(self.math_properties(child));
            }
        }
        self.tree.nodes[id].data = Data::Math {
            r#type: kind,
            props,
        };
        self.tree.nodes[id].base.children = children;
        Ok(id)
    }
}
