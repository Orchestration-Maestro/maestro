//! Immediate font-table parsing.
use super::super::{XmlElement, XmlParser};
use super::{EmbedFontRef, EmbedFontType, FontDeclaration};
/// Owned font-table contents.
#[derive(Debug, Default)]
pub struct FontTablePart {
    /// Declarations in source order.
    pub fonts: Vec<FontDeclaration>,
}
impl FontTablePart {
    /// Read immediate declarations into owned records.
    #[must_use]
    pub fn parse_xml<'doc, 'input: 'doc>(
        xml: &'doc XmlParser<'input>,
        root: XmlElement<'doc, 'input>,
    ) -> Self {
        Self {
            fonts: xml
                .elements(root, None)
                .map(|e| parse_font(xml, e))
                .collect(),
        }
    }
}
/// Read one declaration, replacing scalar fields on later encounters.
fn parse_font(xml: &XmlParser<'_>, element: XmlElement<'_, '_>) -> FontDeclaration {
    let mut result = FontDeclaration {
        name: xml.attr(element, "name").map(str::to_owned),
        ..FontDeclaration::default()
    };
    for child in xml.elements(element, None) {
        let kind = match child.local_name() {
            "altName" => {
                result.alt_name = Some(xml.attr(child, "val").map(str::to_owned));
                continue;
            }
            "family" => {
                result.family = Some(xml.attr(child, "val").map(str::to_owned));
                continue;
            }
            "embedRegular" => EmbedFontType::Regular,
            "embedBold" => EmbedFontType::Bold,
            "embedItalic" => EmbedFontType::Italic,
            "embedBoldItalic" => EmbedFontType::BoldItalic,
            _ => continue,
        };
        result.embed_font_refs.push(EmbedFontRef {
            id: xml.attr(child, "id").map(str::to_owned),
            key: xml.attr(child, "fontKey").map(str::to_owned),
            r#type: kind,
        });
    }
    result
}

impl super::super::WordDocument {
    /// Read and de-obfuscate an embedded font into owned bytes.
    ///
    /// # Errors
    /// Returns resource-read failures or `DocxError::FontKey` when an existing
    /// resource has no key, including when that resource is empty.
    pub fn load_font(
        &self,
        font_table: &super::super::Part,
        id: &str,
        key: Option<&str>,
    ) -> Result<Option<Vec<u8>>, super::super::DocxError> {
        let Some(mut bytes) = self.load_resource(font_table, id)? else {
            return Ok(None);
        };
        let key = key.ok_or(super::super::DocxError::FontKey)?;
        deobfuscate(&mut bytes, key);
        Ok(Some(bytes))
    }
}
/// XOR existing prefix bytes with the reversed, permissively parsed key.
fn deobfuscate(data: &mut [u8], key: &str) {
    use num_traits::ToPrimitive;
    let trimmed: String = key
        .chars()
        .filter(|c| !matches!(c, '{' | '}' | '-'))
        .collect();
    let mut chars = trimmed.chars();
    let mut numbers = [0_u8; 16];
    for number in numbers.iter_mut().rev() {
        let pair: String = chars.by_ref().take(2).collect();
        *number = super::super::document::common::integer_prefix(&pair, 16)
            .to_u8()
            .unwrap_or(0);
    }
    for (index, byte) in data.iter_mut().take(32).enumerate() {
        *byte ^= numbers[index % numbers.len()];
    }
}
