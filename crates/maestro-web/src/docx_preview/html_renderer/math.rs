//! Office math rendered as `MathML`.
use super::{HtmlRenderer, render_style_values};
use crate::docx_preview::html_element::{HtmlContent, HtmlElement, Namespace};
use maestro_app::browser::docx_preview::{
    DomType, MathProperties, OpenXmlElementTree, RunProperties,
};

impl<'a> HtmlRenderer<'a> {
    /// Render one equation element by kind; kinds that are not equation elements render
    /// nothing.
    pub(super) fn render_math(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        kind: DomType,
        props: Option<&MathProperties>,
    ) -> Option<HtmlElement> {
        let tag = match kind {
            DomType::MmlMath => "math",
            DomType::MmlMathParagraph => return Some(self.render_container(tree, index, "span")),
            DomType::MmlFraction => "mfrac",
            DomType::MmlBase
            | DomType::MmlNumerator
            | DomType::MmlDenominator
            | DomType::MmlFunction
            | DomType::MmlLimit
            | DomType::MmlBox => "mrow",
            DomType::MmlGroupChar => return Some(self.render_mml_group_char(tree, index, props)),
            DomType::MmlLimitLower => "munder",
            DomType::MmlMatrix => "mtable",
            DomType::MmlMatrixRow => return Some(self.render_mml_matrix_row(tree, index)),
            DomType::MmlRadical => return Some(self.render_mml_radical(tree, index, props)),
            DomType::MmlSuperscript => "msup",
            DomType::MmlSubscript => "msub",
            DomType::MmlDegree | DomType::MmlSuperArgument | DomType::MmlSubArgument => "mn",
            DomType::MmlFunctionName => "ms",
            DomType::MmlDelimiter => return Some(self.render_mml_delimiter(tree, index, props)),
            DomType::MmlNary => return Some(self.render_mml_nary(tree, index, props)),
            DomType::MmlPreSubSuper => return Some(self.render_mml_pre_sub_super(tree, index)),
            DomType::MmlBar => return Some(self.render_mml_bar(tree, index, props)),
            DomType::MmlEquationArray => return Some(self.render_mll_list(tree, index)),
            _ => return None,
        };
        Some(self.render_container_ns(tree, index, Namespace::MathMl, tag))
    }

    /// Render a matrix row, its bases as cells.
    fn render_mml_matrix_row(&mut self, tree: &'a OpenXmlElementTree, index: usize) -> HtmlElement {
        let mut row = mathml("mtr");
        for &child in &tree.nodes[index].base.children {
            if tree.nodes[child].dom_type() == DomType::MmlBase {
                let cell = self.render_container_ns(tree, child, Namespace::MathMl, "mtd");
                row.push_element(cell);
            } else {
                row.append(self.render_elements(tree, &[child]));
            }
        }
        row
    }

    /// Render a radical: its base under a square root when the degree is hidden or missing,
    /// otherwise with its degree.
    fn render_mml_radical(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        props: Option<&MathProperties>,
    ) -> HtmlElement {
        let hidden = props.and_then(|props| props.hide_degree) == Some(Some(true));
        let degree = child_of_kind(tree, index, DomType::MmlDegree).filter(|_| !hidden);
        let tag = if degree.is_some() { "mroot" } else { "msqrt" };
        let children: Vec<usize> = child_of_kind(tree, index, DomType::MmlBase)
            .into_iter()
            .chain(degree)
            .collect();
        mathml_with(tag, self.render_elements(tree, &children))
    }

    /// Render a delimiter between its opening and closing characters, `(` and `)` by default.
    fn render_mml_delimiter(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        props: Option<&MathProperties>,
    ) -> HtmlElement {
        let begin = props.and_then(|props| props.begin_char.as_ref().and_then(Option::as_deref));
        let end = props.and_then(|props| props.end_char.as_ref().and_then(Option::as_deref));
        let mut result = mathml("mrow");
        result.push_element(operator(begin.unwrap_or("(")));
        result.append(self.render_elements(tree, &tree.nodes[index].base.children));
        result.push_element(operator(end.unwrap_or(")")));
        result
    }

    /// Render an n-ary operator, `∫` by default, with the scripts it has, then its base's
    /// content.
    fn render_mml_nary(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        props: Option<&MathProperties>,
    ) -> HtmlElement {
        let sup = self.render_script(tree, index, DomType::MmlSuperArgument);
        let sub = self.render_script(tree, index, DomType::MmlSubArgument);
        let char = props.and_then(|props| props.char.as_ref().and_then(Option::as_deref));
        let char = operator(char.unwrap_or("\u{222B}"));
        let mut result = mathml("mrow");
        result.push_element(match (sub, sup) {
            (Some(sub), Some(sup)) => mathml_from("munderover", [char, sub, sup]),
            (None, Some(sup)) => mathml_from("mover", [char, sup]),
            (Some(sub), None) => mathml_from("munder", [char, sub]),
            (None, None) => char,
        });
        result.append(self.render_base_content(tree, index));
        result
    }

    /// Render scripts before their base, an empty operator standing for each missing one, then
    /// the base's content.
    fn render_mml_pre_sub_super(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
    ) -> HtmlElement {
        let sup = self.render_script(tree, index, DomType::MmlSuperArgument);
        let sub = self.render_script(tree, index, DomType::MmlSubArgument);
        let scripts = [
            mathml("mo"),
            sub.unwrap_or_else(|| mathml("mo")),
            sup.unwrap_or_else(|| mathml("mo")),
        ];
        let mut result = mathml("mrow");
        result.push_element(mathml_from("msubsup", scripts));
        result.append(self.render_base_content(tree, index));
        result
    }

    /// Render a grouping character over its content when justified to the bottom, otherwise
    /// under it.
    fn render_mml_group_char(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        props: Option<&MathProperties>,
    ) -> HtmlElement {
        let bottom = props.and_then(|props| {
            props
                .vertical_justification
                .as_ref()
                .and_then(Option::as_deref)
        });
        let tag = if bottom == Some("bot") {
            "mover"
        } else {
            "munder"
        };
        let mut result = self.render_container_ns(tree, index, Namespace::MathMl, tag);
        let char = props.and_then(|props| props.char.as_ref().and_then(Option::as_deref));
        if let Some(char) = char.filter(|char| !char.is_empty()) {
            result.push_element(operator(char));
        }
        result
    }

    /// Render a bar over its content at the top and under it at the bottom.
    fn render_mml_bar(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        props: Option<&MathProperties>,
    ) -> HtmlElement {
        let mut result = self.render_container_ns(tree, index, Namespace::MathMl, "mrow");
        let position = props.and_then(|props| props.position.as_ref().and_then(Option::as_deref));
        match position {
            Some("top") => result.set_style("text-decoration", Some("overline")),
            Some("bottom" | "bot") => result.set_style("text-decoration", Some("underline")),
            _ => {}
        }
        result
    }

    /// Render a math run with its class and declarations.
    pub(super) fn render_mml_run(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        properties: &RunProperties,
    ) -> HtmlElement {
        let mut result = self.render_container_ns(tree, index, Namespace::MathMl, "ms");
        let style_name = properties.style_name.as_ref().and_then(Option::as_deref);
        self.render_class(None, style_name, &mut result);
        render_style_values(properties.css_style.as_ref(), &mut result);
        result
    }

    /// Render an equation array as a table, one row per rendered child.
    fn render_mll_list(&mut self, tree: &'a OpenXmlElementTree, index: usize) -> HtmlElement {
        let mut result = mathml("mtable");
        for child in self.render_elements(tree, &tree.nodes[index].base.children) {
            let mut cell = mathml("mtd");
            cell.append(vec![child]);
            result.push_element(mathml_from("mtr", [cell]));
        }
        result
    }

    /// Render the last script of a kind inside an operator.
    fn render_script(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
        kind: DomType,
    ) -> Option<HtmlElement> {
        let script = last_child_of_kind(tree, index, kind)?;
        Some(mathml_with("mo", self.render_elements(tree, &[script])))
    }

    /// Render the content of the last base, or nothing without one.
    fn render_base_content(
        &mut self,
        tree: &'a OpenXmlElementTree,
        index: usize,
    ) -> Vec<HtmlContent> {
        match last_child_of_kind(tree, index, DomType::MmlBase) {
            Some(base) => self.render_elements(tree, &tree.nodes[base].base.children),
            None => Vec::new(),
        }
    }
}

/// Find the first child of a kind.
fn child_of_kind(tree: &OpenXmlElementTree, index: usize, kind: DomType) -> Option<usize> {
    tree.nodes[index]
        .base
        .children
        .iter()
        .copied()
        .find(|&child| tree.nodes[child].dom_type() == kind)
}

/// Find the last child of a kind, as a lookup keyed by kind keeps it.
fn last_child_of_kind(tree: &OpenXmlElementTree, index: usize, kind: DomType) -> Option<usize> {
    tree.nodes[index]
        .base
        .children
        .iter()
        .copied()
        .rfind(|&child| tree.nodes[child].dom_type() == kind)
}

/// Start a `MathML` element.
fn mathml(tag: &'static str) -> HtmlElement {
    HtmlElement::new_ns(Namespace::MathMl, tag)
}

/// Build a `MathML` element around rendered children.
fn mathml_with(tag: &'static str, children: Vec<HtmlContent>) -> HtmlElement {
    let mut element = mathml(tag);
    element.append(children);
    element
}

/// Build a `MathML` element around child elements.
fn mathml_from(tag: &'static str, children: impl IntoIterator<Item = HtmlElement>) -> HtmlElement {
    let mut element = mathml(tag);
    element.extend(children);
    element
}

/// Build an operator holding text.
fn operator(text: &str) -> HtmlElement {
    let mut element = mathml("mo");
    element.push_text(text);
    element
}
