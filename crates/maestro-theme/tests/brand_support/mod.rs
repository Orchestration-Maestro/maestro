//! Pack files, pack documents and recorded expectations for the brand pack tests.
use crate::live::Scratch;
use maestro_theme::{
    BrandMode, BrandPack, ColorMode, ThemeBg, ThemeColor, ThemeError, load_brand_pack,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

/// The shared pack document.
const FORGE: &str = include_str!("../../../../assets/brand/brand.json");
/// The path decoded shared-pack text is labelled with.
pub const FORGE_PATH: &str = "/forge/brand.json";
/// The alternate pack and its marks on the native filesystem.
pub const ALTERNATE_NATIVE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/brand_support/alternate/packs/alternate.json"
);
/// The shared pack on the native filesystem.
pub const FORGE_NATIVE: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/brand/brand.json");
/// The second shipped pack on the native filesystem.
pub const SIGNAL_NATIVE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/brand/packs/signal.json"
);

/// Write each `(relative path, text)` below a fresh directory.
pub fn files(entries: &[(&str, &str)]) -> Scratch {
    let scratch = Scratch::new();
    for (path, text) in entries {
        scratch.write(path, text);
    }
    scratch
}

/// The shared pack at `forge/brand.json` and its four templates beside it.
pub fn forge_files() -> Scratch {
    files(&[
        ("forge/brand.json", FORGE),
        (
            "forge/mark.svg",
            include_str!("../../../../assets/brand/mark.svg"),
        ),
        (
            "forge/mark-flat.svg",
            include_str!("../../../../assets/brand/mark-flat.svg"),
        ),
        (
            "forge/mark-mono.svg",
            include_str!("../../../../assets/brand/mark-mono.svg"),
        ),
        (
            "forge/mark-small.svg",
            include_str!("../../../../assets/brand/mark-small.svg"),
        ),
    ])
}

/// The shared pack as editable JSON.
pub fn forge_json() -> Value {
    serde_json::from_str(FORGE).unwrap()
}

/// Decode a pack document labelled with the shared pack's path.
pub fn load_value(value: &Value) -> Result<BrandPack, ThemeError> {
    BrandPack::parse(FORGE_PATH, &value.to_string())
}

/// Load the shared pack from the native filesystem.
pub fn forge() -> BrandPack {
    load_brand_pack(FORGE_NATIVE).unwrap()
}

/// Load the alternate pack from the native filesystem.
pub fn alternate() -> BrandPack {
    load_brand_pack(ALTERNATE_NATIVE).unwrap()
}

/// Recorded export colors of one projected theme.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Export {
    /// Page background.
    #[serde(rename = "pageBg")]
    pub page: String,
    /// Card background.
    #[serde(rename = "cardBg")]
    pub card: String,
    /// Info background.
    #[serde(rename = "infoBg")]
    pub info: String,
}

/// Recorded terminal prefixes and export colors of one projected theme.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terminal {
    /// Prefix by key in truecolor mode.
    #[serde(default)]
    pub truecolor: BTreeMap<String, String>,
    /// Prefix by key in 256-color mode.
    #[serde(default)]
    pub color256: BTreeMap<String, String>,
    /// Export colors.
    pub export: Export,
}

/// A recorded glyph.
#[derive(Deserialize, PartialEq, Debug)]
#[serde(deny_unknown_fields)]
pub struct Glyph {
    /// Symbol text.
    pub symbol: String,
    /// Resolved color.
    pub color: String,
}

/// Recorded presentation values of one pack and mode.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presented {
    /// Resolved color by role.
    pub colors: BTreeMap<String, String>,
    /// Custom properties.
    pub css: BTreeMap<String, String>,
    /// Glyph by role.
    pub glyphs: BTreeMap<String, Glyph>,
    /// Resolved color by ANSI slot.
    pub ansi: BTreeMap<String, String>,
    /// Resolved color by truecolor name.
    pub truecolor: BTreeMap<String, String>,
    /// Resolved template by variant name; `template` is the default.
    pub svg: BTreeMap<String, String>,
}

/// One recorded entry, such as `forge-dark`; the shared pack's prefixes come from the shipped record.
pub fn terminal(id: &str) -> Terminal {
    let mut all: BTreeMap<String, Terminal> =
        serde_json::from_str(include_str!("terminal.json")).unwrap();
    let mut entry = all.remove(id).unwrap();
    if let Some(mode) = id.strip_prefix("forge-") {
        let mut shipped: BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>> =
            serde_json::from_str(include_str!("../support/shipped_prefixes.json")).unwrap();
        let mut prefixes = shipped.remove(mode).unwrap();
        entry.truecolor = prefixes.remove("truecolor").unwrap();
        entry.color256 = prefixes.remove("color256").unwrap();
    }
    entry
}

/// One recorded entry, such as `alternate-light`.
pub fn presented(id: &str) -> Presented {
    let mut all: BTreeMap<String, Presented> =
        serde_json::from_str(include_str!("presentation.json")).unwrap();
    all.remove(id).unwrap()
}

/// The key sequence of the published theme schema's required colors.
pub fn required_colors() -> Vec<String> {
    let schema: Value =
        serde_json::from_str(include_str!("../../assets/theme/theme-schema.json")).unwrap();
    schema["properties"]["colors"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap().to_owned())
        .collect()
}

/// Load a projected document in both color modes and compare every prefix to the record.
pub fn check_prefixes(document: &str, expected: &Terminal) {
    for (mode, table) in [
        (ColorMode::Truecolor, &expected.truecolor),
        (ColorMode::Color256, &expected.color256),
    ] {
        let theme = crate::support::load(document, Some(mode)).unwrap();
        assert_eq!(table.len(), 51);
        for (key, prefix) in table {
            let found = if key.ends_with("Bg") {
                theme.get_bg_ansi(&ThemeBg::Named(key.clone()))
            } else {
                theme.get_fg_ansi(&ThemeColor::Named(key.clone()))
            };
            assert_eq!(found.unwrap(), prefix, "{key}");
        }
    }
}

/// The mode name used by the recorded entries.
pub fn mode_id(mode: BrandMode) -> &'static str {
    if mode == BrandMode::Dark {
        "dark"
    } else {
        "light"
    }
}
