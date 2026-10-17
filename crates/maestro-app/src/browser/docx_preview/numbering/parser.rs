//! One borrowed reader for raw metadata and prepared levels.
use super::super::document::{paragraph::parse_paragraph_properties, run::parse_run_properties};
use super::super::{
    DefaultPropertiesOptions, IDomNumbering, XmlElement, XmlParser, parse_default_properties,
};
use super::{
    AbstractNumbering, Numbering, NumberingBulletPicture, NumberingLevel, NumberingLevelOverride,
    NumberingPart,
};
/// Encountered picture selector, including an explicitly missing attribute.
#[derive(Clone, Copy)]
pub(crate) enum PictureSelection {
    /// No selector child.
    Absent,
    /// Selected nullable numeric identifier.
    Id(Option<f64>),
}
/// First-child abstract reference, distinct from last-write raw metadata.
pub(crate) enum ReferenceKey {
    /// No reference child.
    Absent,
    /// First reference child's nullable value.
    Id(Option<String>),
}
/// Operation-local level presentation with deferred picture binding.
pub(crate) struct PreparedLevel {
    /// Owned presentation fields.
    pub(crate) dom: IDomNumbering,
    /// Encountered numeric picture selector.
    pub(crate) picture: PictureSelection,
}
/// Owned intermediate arrays aligned with their raw records.
#[derive(Default)]
pub(crate) struct Parsed {
    /// Raw results under construction.
    pub(crate) part: NumberingPart,
    /// First-child reference keys, retaining absent versus null.
    pub(crate) reference_keys: Vec<ReferenceKey>,
    /// Presentations by abstract-definition position.
    pub(crate) definitions: Vec<Vec<PreparedLevel>>,
    /// Replacement presentations by concrete and override position.
    pub(crate) replacements: Vec<Vec<Option<PreparedLevel>>>,
}
/// Borrowed queries and per-operation width policy.
pub(super) struct Reader<'doc, 'input> {
    /// Shared XML reader.
    pub(super) xml: &'doc XmlParser<'input>,
    /// Suppress both paragraph and run cell widths.
    pub(super) ignore_width: bool,
}
impl<'doc, 'input: 'doc> Reader<'doc, 'input> {
    /// Collect raw arrays and presentation slots in a single root traversal.
    pub(super) fn parse(&self, root: XmlElement<'doc, 'input>) -> Parsed {
        let mut result = Parsed::default();
        for child in self.xml.elements(root, None) {
            match child.local_name() {
                "num" => {
                    let (raw, replacements) = self.numbering(child);
                    result.reference_keys.push(self.reference_key(child));
                    result.part.numberings.push(raw);
                    result.replacements.push(replacements);
                }
                "abstractNum" => {
                    let (raw, levels) = self.abstract_numbering(child);
                    result.part.abstract_numberings.push(raw);
                    result.definitions.push(levels);
                }
                "numPicBullet" => result.part.bullet_pictures.push(self.picture(child)),
                _ => {}
            }
        }
        result
    }
    /// Preserve first-child lookup semantics separately from raw metadata.
    fn reference_key(&self, element: XmlElement<'doc, 'input>) -> ReferenceKey {
        self.xml
            .element(element, "abstractNumId")
            .map_or(ReferenceKey::Absent, |e| {
                ReferenceKey::Id(self.attr(e, "val"))
            })
    }
    /// Copy one nullable local attribute.
    fn attr(&self, element: XmlElement<'doc, 'input>, name: &str) -> Option<String> {
        self.xml.attr(element, name).map(str::to_owned)
    }
    /// Read one concrete reference with aligned override presentations.
    fn numbering(
        &self,
        element: XmlElement<'doc, 'input>,
    ) -> (Numbering, Vec<Option<PreparedLevel>>) {
        let mut raw = Numbering {
            id: self.attr(element, "numId"),
            ..Numbering::default()
        };
        let mut replacements = Vec::new();
        for child in self.xml.elements(element, None) {
            match child.local_name() {
                "abstractNumId" => raw.abstract_id = Some(self.attr(child, "val")),
                "lvlOverride" => {
                    let (over, prepared) = self.level_override(child);
                    raw.overrides.push(over);
                    replacements.push(prepared);
                }
                _ => {}
            }
        }
        (raw, replacements)
    }
    /// Read one definition without resolving references yet.
    fn abstract_numbering(
        &self,
        element: XmlElement<'doc, 'input>,
    ) -> (AbstractNumbering, Vec<PreparedLevel>) {
        let mut raw = AbstractNumbering {
            id: self.attr(element, "abstractNumId"),
            ..AbstractNumbering::default()
        };
        let mut levels = Vec::new();
        for child in self.xml.elements(element, None) {
            match child.local_name() {
                "name" => raw.name = Some(self.attr(child, "val")),
                "multiLevelType" => raw.multi_level_type = Some(self.attr(child, "val")),
                "numStyleLink" => raw.numbering_style_link = Some(self.attr(child, "val")),
                "styleLink" => raw.style_link = Some(self.attr(child, "val")),
                "lvl" => {
                    let (level, prepared) = self.level(child);
                    raw.levels.push(level);
                    levels.push(prepared);
                }
                _ => {}
            }
        }
        (raw, levels)
    }
    /// Keep the last nested level and start, irrespective of their child order.
    fn level_override(
        &self,
        element: XmlElement<'doc, 'input>,
    ) -> (NumberingLevelOverride, Option<PreparedLevel>) {
        let mut raw = NumberingLevelOverride {
            level: self.xml.int_attr(element, "ilvl", None),
            ..NumberingLevelOverride::default()
        };
        let mut prepared = None;
        for child in self.xml.elements(element, None) {
            match child.local_name() {
                "startOverride" => raw.start = Some(self.xml.int_attr(child, "val", None)),
                "lvl" => {
                    let (level, dom) = self.level(child);
                    raw.numbering_level = Some(level);
                    prepared = Some(dom);
                }
                _ => {}
            }
        }
        (raw, prepared)
    }
    /// Require the first complete immediate shape chain.
    fn picture(&self, element: XmlElement<'doc, 'input>) -> Option<NumberingBulletPicture> {
        let pict = self.xml.element(element, "pict")?;
        let shape = self.xml.element(pict, "shape")?;
        let image = self.xml.element(shape, "imagedata")?;
        Some(NumberingBulletPicture {
            id: self.attr(element, "numPicBulletId"),
            reference_id: self.attr(image, "id"),
            style: self.attr(shape, "style"),
        })
    }
    /// Collect raw metadata and prepare CSS together without retaining XML.
    fn level(&self, element: XmlElement<'doc, 'input>) -> (NumberingLevel, PreparedLevel) {
        let level = self.xml.int_attr(element, "ilvl", None);
        let mut raw = NumberingLevel {
            level,
            ..NumberingLevel::default()
        };
        let mut prepared = PreparedLevel {
            dom: IDomNumbering {
                id: None,
                level,
                start: Some(1.0),
                p_style_name: None,
                p_style: indexmap::IndexMap::new(),
                r_style: indexmap::IndexMap::new(),
                level_text: None,
                suff: Some("tab".into()),
                format: None,
                bullet: None,
            },
            picture: PictureSelection::Absent,
        };
        for child in self.xml.elements(element, None) {
            self.level_child(child, &mut raw, &mut prepared);
        }
        (raw, prepared)
    }
    /// Dispatch one raw field and its corresponding presentation update.
    fn level_child(
        &self,
        child: XmlElement<'doc, 'input>,
        raw: &mut NumberingLevel,
        prepared: &mut PreparedLevel,
    ) {
        let dom = &mut prepared.dom;
        match child.local_name() {
            "start" => {
                raw.start = Some(self.attr(child, "val"));
                dom.start = self.xml.int_attr(child, "val", None);
            }
            "lvlRestart" => raw.restart = Some(self.xml.int_attr(child, "val", None)),
            "numFmt" => {
                raw.format = Some(self.attr(child, "val"));
                dom.format.clone_from(&raw.format);
            }
            "lvlText" => {
                raw.text = Some(self.attr(child, "val"));
                dom.level_text.clone_from(&raw.text);
            }
            "lvlJc" => raw.justification = Some(self.attr(child, "val")),
            "lvlPicBulletId" => {
                raw.bullet_picture_id = Some(self.attr(child, "val"));
                prepared.picture = PictureSelection::Id(self.xml.int_attr(child, "val", None));
            }
            "pStyle" => {
                raw.paragraph_style = Some(self.attr(child, "val"));
                dom.p_style_name.clone_from(&raw.paragraph_style);
            }
            "suff" => dom.suff = self.attr(child, "val"),
            "pPr" => {
                raw.paragraph_props = Some(parse_paragraph_properties(self.xml, child));
                self.styles(child, &mut dom.p_style);
            }
            "rPr" => {
                raw.run_props = Some(parse_run_properties(self.xml, child));
                self.styles(child, &mut dom.r_style);
            }
            _ => {}
        }
    }
    /// Delegate both presentation maps to the shared CSS owner.
    fn styles(
        &self,
        element: XmlElement<'doc, 'input>,
        style: &mut indexmap::IndexMap<String, Option<String>>,
    ) {
        parse_default_properties(
            self.xml,
            element,
            style,
            None,
            DefaultPropertiesOptions {
                ignore_width: self.ignore_width,
                handler: None,
            },
        );
    }
}
