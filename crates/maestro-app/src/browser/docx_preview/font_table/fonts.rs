//! Owned font declarations.
/// Closed embedded-font variant mapping.
#[derive(Clone, Copy, Debug)]
pub enum EmbedFontType {
    /// Regular face.
    Regular,
    /// Bold face.
    Bold,
    /// Italic face.
    Italic,
    /// Bold italic face.
    BoldItalic,
}
/// One embedded font relationship.
#[derive(Clone, Debug)]
pub struct EmbedFontRef {
    /// Relationship identifier.
    pub id: Option<String>,
    /// Obfuscation key.
    pub key: Option<String>,
    /// Face variant.
    pub r#type: EmbedFontType,
}
/// One font declaration with ordered references.
#[derive(Clone, Debug, Default)]
pub struct FontDeclaration {
    /// Nullable font name.
    pub name: Option<String>,
    /// Encountered alternative name.
    pub alt_name: Option<Option<String>>,
    /// Encountered font family.
    pub family: Option<Option<String>>,
    /// Embedded faces in source order.
    pub embed_font_refs: Vec<EmbedFontRef>,
}
