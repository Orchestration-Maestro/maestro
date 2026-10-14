//! One indexed ZIP owner for Word parts and resource bytes.
use super::super::{DocxError, Options, Relationship, XmlParser, parse_xml_string, utils};
use std::{
    collections::HashMap,
    io::{Cursor, Read},
    sync::Mutex,
};

/// Indexed archive with a privately serialized seek cursor.
pub(crate) struct OpenXmlPackage {
    /// Supplied archive bytes and native metadata.
    archive: Mutex<zip::ZipArchive<Cursor<Vec<u8>>>>,
    /// Sanitized archive spelling to last central-directory entry index.
    entries: HashMap<String, usize>,
    /// XML admission policy.
    options: Options,
}

impl OpenXmlPackage {
    /// Open supplied bytes and index admitted names once.
    pub(crate) fn open(data: &[u8], options: Options) -> Result<Self, DocxError> {
        let mut archive =
            zip::ZipArchive::new(Cursor::new(data.to_vec())).map_err(archive_error)?;
        let mut entries = HashMap::new();
        for index in 0..archive.len() {
            let entry = archive.by_index(index).map_err(archive_error)?;
            let candidate = (entry.central_header_start(), index);
            entries
                .entry(utils::archive_name(entry.name()))
                .and_modify(|current| *current = candidate.max(*current))
                .or_insert(candidate);
        }
        Ok(Self {
            archive: Mutex::new(archive),
            entries: entries
                .into_iter()
                .map(|(name, (_, index))| (name, index))
                .collect(),
            options,
        })
    }
    /// Find slash spelling before the backslash fallback.
    fn index(&self, path: &str) -> Option<usize> {
        let path = utils::normalize_path(path);
        self.entries
            .get(path)
            .or_else(|| self.entries.get(&path.replace('/', "\\")))
            .copied()
    }
    /// Check entry presence without decompressing it.
    pub(crate) fn contains(&self, path: &str) -> bool {
        self.index(path).is_some()
    }
    /// Read resource bytes only when requested.
    pub(crate) fn read(&self, path: &str) -> Result<Option<Vec<u8>>, DocxError> {
        let Some(index) = self.index(path) else {
            return Ok(None);
        };
        let mut archive = self
            .archive
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut entry = archive.by_index(index).map_err(archive_error)?;
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(archive_error)?;
        Ok(Some(bytes))
    }
    /// Read decoded source XML, retaining replacement characters.
    pub(crate) fn text(&self, path: &str) -> Result<Option<String>, DocxError> {
        Ok(self
            .read(path)?
            .map(|data| String::from_utf8_lossy(&data).into_owned()))
    }
    /// Parse after archive access has released its cursor lock.
    pub(crate) fn parse<'a>(&self, text: &'a str, path: &str) -> Result<XmlParser<'a>, DocxError> {
        parse_xml_string(text, self.options.trim_xml_declaration).map_err(|mut error| {
            if let DocxError::Xml { path: context, .. } = &mut error {
                *context = Some(path.into());
            }
            error
        })
    }
    /// Read root or part relationships before interpreting part XML.
    pub(crate) fn relationships(
        &self,
        path: Option<&str>,
    ) -> Result<Option<Vec<Relationship>>, DocxError> {
        let path = path.map_or_else(
            || "_rels/.rels".into(),
            |path| {
                let (folder, name) = utils::split_path(path);
                format!("{folder}_rels/{name}.rels")
            },
        );
        let Some(text) = self.text(&path)?.filter(|s| !s.is_empty()) else {
            return Ok(None);
        };
        let xml = self.parse(&text, &path)?;
        Ok(Some(
            xml.elements(xml.root(), None)
                .map(|node| Relationship {
                    id: xml.attr(node, "Id").map(str::to_owned),
                    r#type: xml.attr(node, "Type").map(str::to_owned),
                    target: xml.attr(node, "Target").map(str::to_owned),
                    target_mode: xml.attr(node, "TargetMode").map(str::to_owned),
                })
                .collect(),
        ))
    }
}

/// Wrap a native archive diagnostic at the declared archive boundary.
fn archive_error(error: impl std::fmt::Display) -> DocxError {
    DocxError::Archive(error.to_string())
}
