//! Compound-file container reading that returns the legacy workbook stream.
use super::blob::{Blob, ParseError, Result, index};

/// Compound-file signature.
pub(super) const SIGNATURE: [u8; 8] = [0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];
/// Chain links at or above this value end a chain.
const SPECIAL: u32 = 0x8000_0000;
/// End-of-chain marker.
const END_OF_CHAIN: u32 = 0xffff_fffe;
/// Free-sector marker.
const FREE: u32 = 0xffff_ffff;
/// Mini-stream sector width.
const MINI_SECTOR: usize = 64;
/// Streams below this size are stored in the mini stream.
const MINI_CUTOFF: usize = 4096;
/// Directory entry width.
const ENTRY: usize = 128;

/// Header fields consumed by stream selection.
struct Header {
    /// Regular sector width.
    sector_size: usize,
    /// First directory sector.
    directory: u32,
    /// First mini allocation sector.
    mini_fat: u32,
    /// Allocation sectors in order, header entries first.
    fat: Vec<u32>,
}

impl Header {
    /// Validate the fixed header and collect allocation sectors listed in it.
    fn parse(data: &[u8]) -> Result<(Self, u32, u32)> {
        if data.len() < 512 {
            return Err(ParseError::Truncated);
        }
        let mut blob = Blob::new(data);
        if blob.take(8)? != SIGNATURE {
            return Err(ParseError::Invalid("compound header"));
        }
        blob.take(18)?;
        let version = blob.u16()?;
        blob.take(2)?;
        let sector_size = match (version, blob.u16()?) {
            (3, 9) => 512,
            (4, 12) => 4096,
            _ => return Err(ParseError::Invalid("compound version")),
        };
        if blob.u16()? != 6 || blob.take(6)? != [0; 6] {
            return Err(ParseError::Invalid("compound header"));
        }
        if blob.u32()? != 0 && version == 3 {
            return Err(ParseError::Invalid("compound header"));
        }
        blob.take(4)?;
        let directory = blob.u32()?;
        blob.take(4)?;
        if blob.u32()? != 4096 {
            return Err(ParseError::Invalid("compound header"));
        }
        let mini_fat = blob.u32()?;
        blob.take(4)?;
        let (difat, difat_count) = (blob.u32()?, blob.u32()?);
        let mut fat = Vec::new();
        for _ in 0..109 {
            match blob.u32()? {
                sector if sector < SPECIAL => fat.push(sector),
                _ => break,
            }
        }
        let header = Self {
            sector_size,
            directory,
            mini_fat,
            fat,
        };
        Ok((header, difat, difat_count))
    }
}

/// Directory entry fields consumed by stream selection.
struct Entry {
    /// Entry name.
    name: String,
    /// Object type: 1 storage, 2 stream, 5 root.
    kind: u8,
    /// Left, right and child links.
    links: [u32; 3],
    /// First sector of the content.
    start: u32,
    /// Declared content length.
    size: usize,
}

impl Entry {
    /// Decode one fixed-width directory entry.
    fn parse(data: &[u8]) -> Result<Self> {
        let mut blob = Blob::at(data, 64)?;
        let length = usize::from(blob.u16()?).min(64).saturating_sub(2);
        let kind = blob.u8()?;
        blob.u8()?;
        let links = [blob.u32()?, blob.u32()?, blob.u32()?];
        let mut blob = Blob::at(data, 116)?;
        let (start, size) = (blob.u32()?, blob.u32()?);
        let units: Vec<u16> = data[..length]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair))
            .collect();
        Ok(Self {
            name: String::from_utf16_lossy(&units),
            kind,
            links,
            start,
            size: index(size)?,
        })
    }
}

/// Borrowed container with its allocation index.
struct Container<'a> {
    /// Supplied bytes.
    data: &'a [u8],
    /// Validated header.
    header: Header,
}

impl<'a> Container<'a> {
    /// Validate the header and extend the allocation sectors through the extension chain.
    fn open(data: &'a [u8]) -> Result<Self> {
        let (header, mut next, mut count) = Header::parse(data)?;
        let mut container = Self { data, header };
        let links = container.header.sector_size / 4 - 1;
        while next != FREE {
            if next == END_OF_CHAIN && count != 0 {
                return Err(ParseError::Invalid("allocation extension"));
            }
            let Some(sector) = container.sector(next).filter(|_| next != END_OF_CHAIN) else {
                break;
            };
            let mut blob = Blob::new(sector);
            for _ in 0..links {
                match blob.u32()? {
                    END_OF_CHAIN => break,
                    link => container.header.fat.push(link),
                }
            }
            if count == 0 {
                break;
            }
            next = Blob::at(sector, container.header.sector_size - 4)?.u32()?;
            count -= 1;
        }
        if count != 0 {
            return Err(ParseError::Invalid("allocation extension"));
        }
        Ok(container)
    }
    /// Borrow a regular sector, allowing a short final sector.
    fn sector(&self, id: u32) -> Option<&'a [u8]> {
        let size = self.header.sector_size;
        let start = index(id).ok()?.checked_add(1)?.checked_mul(size)?;
        let end = start.saturating_add(size).min(self.data.len());
        self.data.get(start..end).filter(|s| !s.is_empty())
    }
    /// Look up the next link of a chain through its allocation sector.
    fn next(&self, id: u32) -> Option<u32> {
        let per_sector = self.header.sector_size / 4;
        let position = index(id).ok()?;
        let sector = self.sector(*self.header.fat.get(position / per_sector)?)?;
        Blob::at(sector, position % per_sector * 4).ok()?.u32().ok()
    }
    /// Concatenate a regular chain, ending at a marker, a missing table sector or a cycle.
    fn chain(&self, start: u32) -> Result<Vec<u8>> {
        let mut seen = vec![false; self.data.len() / self.header.sector_size + 1];
        let mut bytes = Vec::new();
        let mut id = start;
        while id < SPECIAL {
            let visited = seen.get_mut(index(id)?).ok_or(ParseError::Truncated)?;
            if std::mem::replace(visited, true) {
                break;
            }
            bytes.extend_from_slice(self.sector(id).ok_or(ParseError::Truncated)?);
            let Some(next) = self.next(id) else {
                break;
            };
            id = next;
        }
        Ok(bytes)
    }
}

/// Concatenate a mini-stream chain, bounded by the declared size.
fn mini_chain(entry: &Entry, store: &[u8], table: &[u8]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut seen = vec![false; store.len() / MINI_SECTOR];
    let mut id = entry.start;
    while bytes.len() < entry.size && id < SPECIAL {
        let offset = index(id)?;
        let visited = seen.get_mut(offset).ok_or(ParseError::Truncated)?;
        if std::mem::replace(visited, true) {
            break;
        }
        bytes.extend_from_slice(Blob::at(store, offset * MINI_SECTOR)?.take(MINI_SECTOR)?);
        let Some(next) = Blob::at(table, offset * 4)
            .ok()
            .and_then(|mut blob| blob.u32().ok())
        else {
            break;
        };
        id = next;
    }
    bytes.truncate(entry.size);
    Ok(bytes)
}

/// Indices of the root storage's direct children, reached through its sibling tree.
fn root_children(entries: &[Entry]) -> Vec<bool> {
    let mut member = vec![false; entries.len()];
    let mut pending = vec![entries.first().map_or(FREE, |root| root.links[2])];
    while let Some(id) = pending.pop() {
        let Some(slot) = index(id).ok().filter(|i| *i < entries.len()) else {
            continue;
        };
        if !std::mem::replace(&mut member[slot], true) {
            pending.extend(&entries[slot].links[..2]);
        }
    }
    member
}

/// Return the root-level `Workbook` stream, else `Book`, as owned bytes.
pub(super) fn workbook_stream(data: &[u8]) -> Result<Vec<u8>> {
    let container = Container::open(data)?;
    let directory = container.chain(container.header.directory)?;
    let entries = directory
        .as_chunks::<ENTRY>()
        .0
        .iter()
        .map(|entry| Entry::parse(entry))
        .collect::<Result<Vec<_>>>()?;
    let children = root_children(&entries);
    let stream = ["Workbook", "Book"]
        .into_iter()
        .find_map(|name| {
            entries.iter().zip(&children).find_map(|(entry, child)| {
                (*child && entry.kind == 2 && entry.name.eq_ignore_ascii_case(name))
                    .then_some(entry)
            })
        })
        .ok_or(ParseError::Invalid("workbook stream"))?;
    if stream.size >= MINI_CUTOFF {
        let mut bytes = container.chain(stream.start)?;
        bytes.truncate(stream.size);
        return Ok(bytes);
    }
    let root = entries
        .iter()
        .rfind(|entry| entry.kind == 5)
        .ok_or(ParseError::Invalid("root entry"))?;
    let store = container.chain(root.start)?;
    let table = container.chain(container.header.mini_fat)?;
    mini_chain(stream, &store, &table)
}

#[cfg(test)]
mod tests {
    use super::{Entry, mini_chain};

    #[test]
    fn compound_extension_count_rejects_an_early_chain_end() {
        let mut data = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/workbooks/compound-allocation-extension.xls"
        ))
        .to_vec();
        assert!(super::Container::open(&data).is_ok());
        data[68..72].copy_from_slice(&super::FREE.to_le_bytes());
        data[72..76].copy_from_slice(&1_u32.to_le_bytes());
        assert!(matches!(
            super::Container::open(&data),
            Err(super::ParseError::Invalid(_))
        ));
    }

    #[test]
    fn mini_chain_stops_at_revisited_sector_without_repeating_data() {
        let entry = Entry {
            name: String::new(),
            kind: 2,
            links: [0; 3],
            start: 0,
            size: 128,
        };
        let store = [vec![1; 64], vec![2; 64]].concat();
        assert_eq!(
            mini_chain(&entry, &store, &0_u32.to_le_bytes()).unwrap(),
            vec![1; 64]
        );
        let table = [1_u32.to_le_bytes(), 0xffff_fffe_u32.to_le_bytes()].concat();
        assert_eq!(mini_chain(&entry, &store, &table).unwrap(), store);
    }
}
