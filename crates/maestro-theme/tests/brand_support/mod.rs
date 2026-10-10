//! Controlled pack operations, pack documents and recorded expectations for the brand pack tests.
use maestro_theme::{
    BrandMode, BrandPack, ColorMode, ThemeBg, ThemeColor, ThemeError, ThemeInfo, ThemeOperations,
    load_brand_pack, load_theme_from_path,
};
use serde::Deserialize;
use serde_json::Value;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;

/// The shared pack document.
const FORGE: &str = include_str!("../../../../assets/brand/brand.json");
/// Where the controlled operations serve the shared pack.
pub const FORGE_PATH: &str = "/forge/brand.json";
/// Where the controlled operations serve the alternate pack.
pub const ALTERNATE_PATH: &str = "/alt/packs/alternate.json";
/// The shared pack on the native filesystem.
pub const FORGE_NATIVE: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/brand/brand.json");
/// The second shipped pack on the native filesystem.
pub const SIGNAL_NATIVE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/brand/packs/signal.json"
);

/// Serves fixed files and records each read in order.
pub struct Mem {
    /// File text by path.
    files: RefCell<BTreeMap<String, String>>,
    /// Paths read, in call order.
    pub reads: RefCell<Vec<String>>,
}

impl Mem {
    /// Serve exactly the given files.
    pub fn new(files: &[(&str, &str)]) -> Self {
        Self {
            files: RefCell::new(
                files
                    .iter()
                    .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
                    .collect(),
            ),
            reads: RefCell::default(),
        }
    }

    /// Serve the shared pack and its four templates beside it.
    pub fn forge() -> Self {
        Self::new(&[
            (FORGE_PATH, FORGE),
            (
                "/forge/mark.svg",
                include_str!("../../../../assets/brand/mark.svg"),
            ),
            (
                "/forge/mark-flat.svg",
                include_str!("../../../../assets/brand/mark-flat.svg"),
            ),
            (
                "/forge/mark-mono.svg",
                include_str!("../../../../assets/brand/mark-mono.svg"),
            ),
            (
                "/forge/mark-small.svg",
                include_str!("../../../../assets/brand/mark-small.svg"),
            ),
        ])
    }

    /// Serve the alternate pack and its five templates.
    pub fn alternate() -> Self {
        Self::new(&[
            (
                ALTERNATE_PATH,
                include_str!("alternate/packs/alternate.json"),
            ),
            (
                "/alt/marks/alternate.svg",
                include_str!("alternate/marks/alternate.svg"),
            ),
            (
                "/alt/marks/alternate-flat.svg",
                include_str!("alternate/marks/alternate-flat.svg"),
            ),
            (
                "/alt/marks/alternate-mono.svg",
                include_str!("alternate/marks/alternate-mono.svg"),
            ),
            (
                "/alt/marks/alternate-small.svg",
                include_str!("alternate/marks/alternate-small.svg"),
            ),
            (
                "/alt/marks/alternate-badge.svg",
                include_str!("alternate/marks/alternate-badge.svg"),
            ),
        ])
    }

    /// Serve one more file.
    pub fn with(self, path: &str, text: &str) -> Self {
        self.files
            .borrow_mut()
            .insert(path.to_owned(), text.to_owned());
        self
    }
}

impl ThemeOperations for Mem {
    fn read_to_string(&self, path: &str) -> io::Result<String> {
        self.reads.borrow_mut().push(path.to_owned());
        self.files
            .borrow()
            .get(path)
            .cloned()
            .ok_or_else(|| io::ErrorKind::NotFound.into())
    }

    fn environment(&self, _name: &str) -> Option<String> {
        None
    }

    fn exists(&self, path: &str) -> bool {
        self.files.borrow().contains_key(path)
    }

    fn read_dir(&self, _path: &str) -> io::Result<Vec<String>> {
        Err(io::ErrorKind::Unsupported.into())
    }

    fn sort_by_name(&self, _themes: &mut [ThemeInfo]) -> io::Result<()> {
        Ok(())
    }
}

/// The shared pack as editable JSON.
pub fn forge_json() -> Value {
    serde_json::from_str(FORGE).unwrap()
}

/// Decode a pack document served at the shared pack's path.
pub fn load_value(value: &Value) -> Result<BrandPack, ThemeError> {
    load_brand_pack(FORGE_PATH, &Mem::new(&[(FORGE_PATH, &value.to_string())]))
}

/// Decode the shared pack from the controlled files.
pub fn forge() -> BrandPack {
    load_brand_pack(FORGE_PATH, &Mem::forge()).unwrap()
}

/// Decode the alternate pack from the controlled files.
pub fn alternate() -> BrandPack {
    load_brand_pack(ALTERNATE_PATH, &Mem::alternate()).unwrap()
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
    pub truecolor: BTreeMap<String, String>,
    /// Prefix by key in 256-color mode.
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

/// One recorded entry, such as `forge-dark`.
pub fn terminal(id: &str) -> Terminal {
    let mut all: BTreeMap<String, Terminal> =
        serde_json::from_str(include_str!("terminal.json")).unwrap();
    all.remove(id).unwrap()
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
        let theme = load_theme_from_path(
            "theme.json",
            Some(mode),
            &Mem::new(&[("theme.json", document)]),
        )
        .unwrap();
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
