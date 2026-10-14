//! Ordered, cached discovery of supported Word package parts.
use super::{
    DocxError, Options, Part, PartKind, Relationship, common::open_xml_package::OpenXmlPackage,
    utils,
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
            let path = &document.parts[index].path;
            let rels = document.package.relationships(Some(path))?;
            let xml = document.package.text(path)?.unwrap_or_default();
            document.package.parse(&xml, path)?;
            let folder = utils::split_path(path).0;
            let children: Vec<_> = rels
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
            document.parts[index].rels = rels;
            document.parts[index].xml = xml;
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
        });
        if kind == PartKind::Document {
            self.document = Some(index);
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
    /// Resolve an ID from a supplied part or the root relationships to a stored part.
    pub fn find_part_by_rel_id(&self, id: &str, base_part: Option<&Part>) -> Option<&Part> {
        let (rels, folder) = base_part.map_or((self.rels(), ""), |part| {
            (part.rels.as_deref(), utils::split_path(&part.path).0)
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
