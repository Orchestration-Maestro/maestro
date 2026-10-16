//! Ordered, cached discovery of supported Word package parts.
use super::{
    DocxError, Options, Part, PartData, PartKind, Relationship,
    common::open_xml_package::OpenXmlPackage, utils,
};
use std::collections::HashMap;

/// Open Word package with eagerly checked XML and demand-loaded binary resources.
pub struct WordDocument {
    /// Archive backing resource reads.
    package: OpenXmlPackage,
    /// Retained root relationships.
    rels: Option<Vec<Relationship>>,
    /// Reservation order of recognized XML parts.
    parts: Vec<Part>,
    /// Resolved relationship key to stored part.
    index: HashMap<String, usize>,
    /// Latest designated main document.
    document: Option<usize>,
    /// Latest designated core properties.
    core_props: Option<usize>,
    /// Latest designated extended properties.
    extended_props: Option<usize>,
    /// Latest designated comments.
    comments: Option<usize>,
    /// Latest designated extended comments.
    comments_extended: Option<usize>,
}

impl WordDocument {
    /// Open bytes and validate supported XML parts; missing and unknown parts are tolerated.
    ///
    /// # Errors
    /// Returns archive-read failures or XML errors identifying the failing part.
    pub fn load(data: &[u8], options: Options) -> Result<Self, DocxError> {
        let package = OpenXmlPackage::open(data, options)?;
        let rels = package.relationships(None)?;
        let mut document = Self {
            package,
            rels,
            parts: Vec::new(),
            index: HashMap::new(),
            document: None,
            core_props: None,
            extended_props: None,
            comments: None,
            comments_extended: None,
        };
        let mut roots = Vec::new();
        for (kind, fallback) in [
            (PartKind::Document, "word/document.xml"),
            (PartKind::ExtendedProps, "docProps/app.xml"),
            (PartKind::CoreProps, "docProps/core.xml"),
            (PartKind::CustomProps, "docProps/custom.xml"),
        ] {
            let target = document
                .rels()
                .unwrap_or_default()
                .iter()
                .find(|r| r.r#type.as_deref().and_then(PartKind::from_type) == Some(kind))
                .map_or(Some(fallback), |r| r.target.as_deref())
                .map(str::to_owned);
            if let Some(index) = target.and_then(|path| document.reserve(path, kind)) {
                roots.push(index);
            }
        }
        let mut pending: Vec<_> = roots.into_iter().rev().collect();
        while let Some(index) = pending.pop() {
            let children = document.load_part(index)?;
            pending.extend(
                children
                    .into_iter()
                    .filter_map(|(path, kind)| document.reserve(path, kind))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev(),
            );
        }
        Ok(document)
    }
    /// Admit relationships and XML, prepare semantics, then return descendant reservations.
    fn load_part(&mut self, index: usize) -> Result<Vec<(String, PartKind)>, DocxError> {
        let part = &mut self.parts[index];
        let path = &part.path;
        let rels = self.package.relationships(Some(path))?;
        let source = self.package.text(path)?.unwrap_or_default();
        let xml = self.package.parse(&source, path)?;
        let data = PartData::parse_xml(part.kind, &xml).map_err(|mut error| {
            if let DocxError::Xml { path: context, .. } = &mut error {
                *context = Some(path.clone());
            }
            error
        })?;
        let folder = utils::split_path(path).0;
        let children = rels
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|r| {
                Some((
                    utils::resolve_path(r.target.as_deref()?, folder),
                    PartKind::from_type(r.r#type.as_deref()?)?,
                ))
            })
            .collect();
        part.rels = rels;
        part.xml = source;
        part.data = data;
        Ok(children)
    }
    /// Reserve a fresh supported key before walking descendants, terminating cached cycles.
    fn reserve(&mut self, path: String, kind: PartKind) -> Option<usize> {
        if self.index.contains_key(&path) || !self.package.contains(&path) {
            return None;
        }
        let index = self.parts.len();
        self.index.insert(path.clone(), index);
        self.parts.push(Part {
            path,
            kind,
            rels: None,
            xml: String::new(),
            data: None,
        });
        match kind {
            PartKind::Document => self.document = Some(index),
            PartKind::CoreProps => self.core_props = Some(index),
            PartKind::ExtendedProps => self.extended_props = Some(index),
            PartKind::Comments => self.comments = Some(index),
            PartKind::CommentsExtended => self.comments_extended = Some(index),
            _ => {}
        }
        Some(index)
    }
    /// Borrow retained root relationships.
    pub fn rels(&self) -> Option<&[Relationship]> {
        self.rels.as_deref()
    }
    /// Borrow XML parts in reservation order.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }
    /// Borrow the latest designated document part.
    pub fn document_part(&self) -> Option<&Part> {
        self.document.map(|i| &self.parts[i])
    }
    /// Borrow the latest designated core props data.
    pub fn core_props_part(&self) -> Option<&super::CorePropsPart> {
        match self.parts[self.core_props?].data.as_ref()? {
            PartData::CoreProps(data) => Some(data),
            _ => None,
        }
    }
    /// Borrow the latest designated extended props data.
    pub fn extended_props_part(&self) -> Option<&super::ExtendedPropsPart> {
        match self.parts[self.extended_props?].data.as_ref()? {
            PartData::ExtendedProps(data) => Some(data),
            _ => None,
        }
    }
    /// Borrow the latest designated comments data.
    pub fn comments_part(&self) -> Option<&super::CommentsPart> {
        match self.parts[self.comments?].data.as_ref()? {
            PartData::Comments(data) => Some(data),
            _ => None,
        }
    }
    /// Borrow the latest designated comments extended data.
    pub fn comments_extended_part(&self) -> Option<&super::CommentsExtendedPart> {
        match self.parts[self.comments_extended?].data.as_ref()? {
            PartData::CommentsExtended(data) => Some(data),
            _ => None,
        }
    }
    /// Resolve an ID using part relationships when present, otherwise roots, retaining the part folder.
    pub fn find_part_by_rel_id(&self, id: &str, base_part: Option<&Part>) -> Option<&Part> {
        let (rels, folder) = base_part.map_or((self.rels(), ""), |part| {
            (
                part.rels.as_deref().or(self.rels()),
                utils::split_path(&part.path).0,
            )
        });
        let target = Relationship::find_target(rels, id)?;
        self.index
            .get(&utils::resolve_path(target, folder))
            .map(|i| &self.parts[*i])
    }
    /// Resolve the first matching relationship ID to its package target.
    pub fn get_path_by_id(&self, part: &Part, id: &str) -> Option<String> {
        let target = Relationship::find_target(part.rels.as_deref(), id)?;
        Some(utils::resolve_path(target, utils::split_path(&part.path).0))
    }
    /// Read bytes by relationship ID without fetching URLs or writing files.
    ///
    /// # Errors
    /// Returns an archive error if the selected entry cannot be read.
    pub fn load_resource(&self, part: &Part, id: &str) -> Result<Option<Vec<u8>>, DocxError> {
        self.get_path_by_id(part, id)
            .map_or(Ok(None), |path| self.package.read(&path))
    }
}
