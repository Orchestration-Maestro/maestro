//! One indexed ZIP owner for Word parts and resource bytes.
use super::super::{DocxError, Options, Relationship, XmlParser, parse_xml_string, utils};
use std::{
    io::{Cursor, Read},
    sync::Mutex,
};

/// Largest expanded entry read into memory, in bytes.
const ENTRY_LIMIT: u64 = 512 << 20;

/// Indexed archive with a privately serialized seek cursor.
pub(crate) struct OpenXmlPackage {
    /// Supplied archive bytes and native metadata.
    archive: Mutex<PackageState>,
    /// Sanitized archive spelling to last central-directory entry index.
    entries: indexmap::IndexMap<String, usize>,
    /// XML admission policy.
    options: Options,
}

/// Archive cursor and cumulative declared-byte allowance serialized together.
struct PackageState {
    /// Supplied archive bytes and native metadata.
    archive: zip::ZipArchive<Cursor<Vec<u8>>>,
    /// Expanded bytes still available for requested parts, including repeated reads.
    remaining: u64,
}

impl OpenXmlPackage {
    /// Open supplied bytes and index admitted names once.
    pub(crate) fn open(data: &[u8], options: Options) -> Result<Self, DocxError> {
        let mut archive =
            zip::ZipArchive::new(Cursor::new(data.to_vec())).map_err(archive_error)?;
        let mut entries = indexmap::IndexMap::new();
        for index in 0..archive.len() {
            let entry = archive.by_index(index).map_err(archive_error)?;
            let candidate = (entry.central_header_start(), index);
            entries
                .entry(utils::archive_name(&entry.name().map_err(archive_error)?))
                .and_modify(|current| *current = candidate.max(*current))
                .or_insert(candidate);
        }
        Ok(Self {
            archive: Mutex::new(PackageState {
                archive,
                remaining: ENTRY_LIMIT,
            }),
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
    /// Enumerate sanitized names in first-insertion order.
    pub(crate) fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
    /// Decode an exact admitted name without relationship-path normalization.
    pub(crate) fn text_exact(&self, name: &str) -> Result<Option<String>, DocxError> {
        Ok(self
            .read_index(self.entries.get(name).copied())?
            .map(maestro_text::lossy_string))
    }
    /// Read resource bytes only when requested.
    pub(crate) fn read(&self, path: &str) -> Result<Option<Vec<u8>>, DocxError> {
        self.read_index(self.index(path))
    }
    /// Share indexed reads between exact and relationship lookups.
    fn read_index(&self, index: Option<usize>) -> Result<Option<Vec<u8>>, DocxError> {
        let Some(index) = index else {
            return Ok(None);
        };
        let mut state = self
            .archive
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let PackageState { archive, remaining } = &mut *state;
        let mut entry = archive.by_index(index).map_err(archive_error)?;
        // The archive reader fails an entry once it passes its declared size.
        if entry.size() > ENTRY_LIMIT {
            return Err(archive_error("entry is larger than 512 MiB"));
        }
        *remaining = remaining
            .checked_sub(entry.size())
            .ok_or_else(|| archive_error("package reads exceed 512 MiB"))?;
        // The archive reader may read one extra byte to detect understated sizes.
        let capacity = usize::try_from(entry.size() + 1).map_err(archive_error)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(capacity).map_err(archive_error)?;
        entry.read_to_end(&mut bytes).map_err(archive_error)?;
        Ok(Some(bytes.into_boxed_slice().into_vec()))
    }
    /// Read decoded source XML, retaining replacement characters.
    pub(crate) fn text(&self, path: &str) -> Result<Option<String>, DocxError> {
        Ok(self.read(path)?.map(maestro_text::lossy_string))
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
        self.read_relationships(path, false)
    }
    /// Read spreadsheet targets without normalizing literal URL whitespace.
    pub(crate) fn literal_relationships(
        &self,
        path: &str,
    ) -> Result<Option<Vec<Relationship>>, DocxError> {
        self.read_relationships(Some(path), true)
    }
    /// Decode relationships with an explicit target whitespace policy.
    fn read_relationships(
        &self,
        path: Option<&str>,
        literal: bool,
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
                .map(|node| {
                    Ok(Relationship {
                        id: xml.attr(node, "Id").map(str::to_owned),
                        r#type: xml.attr(node, "Type").map(str::to_owned),
                        target: if literal {
                            xml.literal_attr(node, "Target")?
                        } else {
                            xml.attr(node, "Target").map(str::to_owned)
                        },
                        target_mode: xml.attr(node, "TargetMode").map(str::to_owned),
                    })
                })
                .collect::<Result<_, DocxError>>()?,
        ))
    }
}

/// Wrap a native archive diagnostic at the declared archive boundary.
fn archive_error(error: impl std::fmt::Display) -> DocxError {
    DocxError::Archive(error.to_string())
}
