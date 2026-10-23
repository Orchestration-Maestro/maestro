//! Style definitions resolved through their base chains: the paragraph values pages read and
//! the substyles the stylesheet writes.
use maestro_app::browser::docx_preview::{IDomNumbering, IDomStyle, IDomSubStyle, Numbering};
use std::collections::{HashMap, HashSet};

/// Every style definition in document order, each resolved through its base chain.
#[derive(Default)]
pub(crate) struct Styles {
    /// Resolved definitions in document order.
    pub(super) styles: Vec<Style>,
    /// Position of the last definition with each nonempty identifier.
    pub(super) by_id: HashMap<String, usize>,
}

/// One definition with the values it resolves to.
pub(super) struct Style {
    /// Raw identifier.
    pub(super) id: Option<String>,
    /// CSS element target of the style kind.
    pub(super) target: Option<String>,
    /// Identifier of the linked style.
    pub(super) linked: Option<String>,
    /// Whether the definition is a default of its kind.
    pub(super) is_default: bool,
    /// Substyles, each base substyle filling the declarations its target lacks or appended
    /// when the definition has no substyle for its target.
    pub(super) substyles: Vec<IDomSubStyle>,
    /// Paragraph values the style gives its paragraphs.
    pub(super) paragraph: ParagraphStyle,
}

/// Paragraph values a style gives its paragraphs.
pub(crate) struct ParagraphStyle {
    /// Numbering reference, its identifier and level each taken from the nearest style that
    /// sets it.
    pub(crate) numbering: Option<Numbering>,
    /// Whether paragraphs of the style start on a new page.
    pub(crate) page_break_before: bool,
}

/// Values one definition sets or inherits.
struct Inherited {
    /// Substyles in definition order, inherited ones appended.
    substyles: Vec<IDomSubStyle>,
    /// Numbering reference.
    numbering: Option<Numbering>,
    /// Page-break-before flag.
    page_break_before: Option<bool>,
}

impl Styles {
    /// Resolve every definition from the unchanged definitions, child first; the last
    /// definition with an identifier is the one a base, a link or a paragraph names.
    pub(crate) fn process_styles(styles: &[IDomStyle]) -> Self {
        let by_id: HashMap<String, usize> = styles
            .iter()
            .enumerate()
            .filter_map(|(index, style)| {
                Some((style.id.clone().filter(|id| !id.is_empty())?, index))
            })
            .collect();
        let bases: Vec<Option<usize>> = styles
            .iter()
            .map(|style| {
                let based_on = style.based_on.as_deref().filter(|id| !id.is_empty())?;
                by_id.get(based_on).copied()
            })
            .collect();
        let styles = styles
            .iter()
            .zip(resolve(styles, &bases))
            .map(|(style, inherited)| Style {
                id: style.id.clone(),
                target: style.target.clone(),
                linked: style.linked.clone().flatten(),
                is_default: style.is_default == Some(Some(true)),
                substyles: inherited.substyles,
                paragraph: ParagraphStyle {
                    numbering: inherited.numbering,
                    page_break_before: inherited.page_break_before == Some(true),
                },
            })
            .collect();
        Self { styles, by_id }
    }

    /// Borrow the resolved values for a style name.
    pub(crate) fn find_style(&self, style_name: Option<&str>) -> Option<&ParagraphStyle> {
        self.named(style_name).map(|style| &style.paragraph)
    }

    /// Borrow the last definition with a nonempty identifier.
    pub(super) fn named(&self, style_name: Option<&str>) -> Option<&Style> {
        self.by_id
            .get(style_name?)
            .map(|&index| &self.styles[index])
    }

    /// Give a numbered paragraph style the level of each numbering level that names it.
    pub(crate) fn prodess_numberings(&mut self, numberings: &[IDomNumbering]) {
        for num in numberings {
            let Some(&index) = num
                .p_style_name
                .as_ref()
                .and_then(Option::as_deref)
                .and_then(|name| self.by_id.get(name))
            else {
                continue;
            };
            if let Some(numbering) = &mut self.styles[index].paragraph.numbering {
                numbering.level = Some(num.level);
            }
        }
    }
}

impl Inherited {
    /// Take the values a definition sets itself.
    fn own(style: &IDomStyle) -> Self {
        let props = style.paragraph_props.as_ref();
        Self {
            substyles: style.styles.clone(),
            numbering: props.and_then(|props| props.numbering.clone()),
            page_break_before: props.and_then(|props| props.page_break_before),
        }
    }

    /// Fill what the definition lacks from its base: page values field by field, declarations
    /// by substyle target.
    fn inherit(&mut self, base: &Self) {
        self.page_break_before = self.page_break_before.or(base.page_break_before);
        if let Some(base) = &base.numbering {
            let numbering = self.numbering.get_or_insert_default();
            if numbering.id.is_none() {
                numbering.id.clone_from(&base.id);
            }
            numbering.level = numbering.level.or(base.level);
        }
        for inherited in &base.substyles {
            self.merge_substyle(inherited);
        }
    }

    /// Fill what the definition lacks from a base substyle of the same target, appending the
    /// substyle when the definition has none for that target.
    fn merge_substyle(&mut self, inherited: &IDomSubStyle) {
        let Some(substyle) = self
            .substyles
            .iter_mut()
            .find(|substyle| substyle.target == inherited.target)
        else {
            self.substyles.push(inherited.clone());
            return;
        };
        for (key, value) in &inherited.values {
            substyle
                .values
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
    }
}

/// Resolve each definition once: off a base cycle from its base's resolved values, on a cycle
/// by walking its own chain to the first repeat.
fn resolve(styles: &[IDomStyle], bases: &[Option<usize>]) -> Vec<Inherited> {
    let mut resolved: Vec<Option<Inherited>> = styles.iter().map(|_| None).collect();
    let mut on_chain = vec![false; styles.len()];
    for start in 0..styles.len() {
        let mut chain = Vec::new();
        let mut current = Some(start);
        while let Some(index) = current.filter(|&index| resolved[index].is_none()) {
            if on_chain[index] {
                resolve_cycle(&mut chain, index, styles, bases, &mut resolved);
                break;
            }
            on_chain[index] = true;
            chain.push(index);
            current = bases[index];
        }
        for &index in chain.iter().rev() {
            let mut inherited = Inherited::own(&styles[index]);
            if let Some(base) = bases[index].and_then(|base| resolved[base].as_ref()) {
                inherited.inherit(base);
            }
            resolved[index] = Some(inherited);
        }
    }
    resolved.into_iter().flatten().collect()
}

/// Resolve every member of a base cycle by walking its own chain, and keep the members that
/// precede the cycle for the ordinary pass.
fn resolve_cycle(
    chain: &mut Vec<usize>,
    repeat: usize,
    styles: &[IDomStyle],
    bases: &[Option<usize>],
    resolved: &mut [Option<Inherited>],
) {
    let cycle = chain
        .iter()
        .position(|&member| member == repeat)
        .unwrap_or(0);
    for &member in &chain[cycle..] {
        resolved[member] = Some(walk(member, styles, bases));
    }
    chain.truncate(cycle);
}

/// Resolve a definition by walking its base chain until the first repeated definition.
fn walk(start: usize, styles: &[IDomStyle], bases: &[Option<usize>]) -> Inherited {
    let mut inherited = Inherited::own(&styles[start]);
    let mut visited = HashSet::from([start]);
    let mut current = bases[start];
    while let Some(index) = current.filter(|&index| visited.insert(index)) {
        inherited.inherit(&Inherited::own(&styles[index]));
        current = bases[index];
    }
    inherited
}
