//! Typed brand pack records and their native decoding.
use super::super::ThemeError;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::collections::BTreeMap;

/// One font role: its family, license metadata and ordered fallbacks.
///
/// The record names a font; it never reads or fetches font files.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BrandFont {
    /// Primary family name, retained exactly as authored.
    pub family: String,
    /// License identifier of the font.
    pub license: String,
    /// Upstream license location, kept as text.
    #[serde(rename = "license-url")]
    pub license_url: String,
    /// Fallback family names in order, including repeated and empty entries.
    pub fallbacks: Vec<String>,
}

/// One type role: a font role with size, line height and weight.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BrandType {
    /// Key of the font role this type role uses.
    pub font: String,
    /// Size in CSS pixels; positive.
    pub size: f64,
    /// Unitless line height.
    #[serde(rename = "line-height")]
    pub line_height: f64,
    /// Numeric font weight.
    pub weight: f64,
}

/// Mark asset paths and measurements.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct BrandMark {
    /// Default template path, joined to the pack's directory when loaded.
    pub template: String,
    /// Variant template paths by name, joined to the pack's directory when loaded.
    pub variants: BTreeMap<String, String>,
    /// Smallest rendered size in CSS pixels.
    #[serde(rename = "minimum-size")]
    pub minimum_size: f64,
    /// Smallest rendered size of the small variant in CSS pixels.
    #[serde(rename = "small-minimum-size")]
    pub small_minimum_size: f64,
    /// Clear space as a fraction of the rendered width.
    #[serde(rename = "clear-space")]
    pub clear_space: f64,
    /// App icon scale as a fraction of the rendered width.
    #[serde(rename = "app-icon-scale")]
    pub app_icon_scale: f64,
}

/// A glyph symbol and the mode role that colors it.
#[derive(Deserialize)]
pub(super) struct GlyphData {
    /// The symbol text.
    pub symbol: String,
    /// Mode role alias of its color.
    pub color: String,
}

/// The colors of one mode: role to palette key.
#[derive(Deserialize)]
pub(super) struct ModeData {
    /// Role to palette key.
    pub colors: BTreeMap<String, String>,
}

/// Terminal alias maps: slot or name to mode role.
#[derive(Deserialize)]
pub(super) struct TerminalData {
    /// The ANSI slots.
    pub ansi: BTreeMap<String, String>,
    /// The truecolor names.
    pub truecolor: BTreeMap<String, String>,
}

/// The decoded pack file.
#[derive(Deserialize)]
pub(super) struct PackData {
    /// Pack label.
    pub name: String,
    /// Displayed product name.
    pub wordmark: String,
    /// Displayed tagline.
    pub tagline: String,
    /// Palette key to opaque color text.
    pub palette: BTreeMap<String, String>,
    /// Mode name to its colors.
    #[serde(deserialize_with = "records")]
    pub modes: BTreeMap<String, ModeData>,
    /// Font role to font.
    #[serde(deserialize_with = "records")]
    pub fonts: BTreeMap<String, BrandFont>,
    /// Type role to type.
    #[serde(rename = "type", deserialize_with = "records")]
    pub typography: BTreeMap<String, BrandType>,
    /// Spacing role to CSS pixels.
    pub spacing: BTreeMap<String, f64>,
    /// Radius role to CSS pixels.
    pub radii: BTreeMap<String, f64>,
    /// Mark paths and measurements.
    #[serde(deserialize_with = "record")]
    pub mark: BrandMark,
    /// Glyph role to glyph.
    #[serde(deserialize_with = "records")]
    pub glyphs: BTreeMap<String, GlyphData>,
    /// Terminal alias maps.
    #[serde(deserialize_with = "record")]
    pub terminal: TerminalData,
}

/// A record accepted only in its object form, never as a positional array.
struct Object<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        if !value.is_object() {
            return Err(D::Error::custom("expected an object record"));
        }
        T::deserialize(value).map(Self).map_err(D::Error::custom)
    }
}

/// Decode one object-only record.
fn record<'de, D: Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<T, D::Error> {
    Object::deserialize(deserializer).map(|Object(record)| record)
}

/// Decode a map of object-only records.
fn records<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, T>, D::Error> {
    BTreeMap::<String, Object<T>>::deserialize(deserializer).map(|map| {
        map.into_iter()
            .map(|(key, Object(record))| (key, record))
            .collect()
    })
}

impl PackData {
    /// Decode pack text; duplicate members keep the last value before typing.
    pub(super) fn parse(path: &str, text: &str) -> Result<Self, ThemeError> {
        serde_json::from_str::<Object<Self>>(text)
            .map(|Object(pack)| pack)
            .map_err(|cause| {
                ThemeError::caused_by(format!("Invalid brand pack {path}: {cause}"), cause)
            })
    }

    /// Reject a type size that is not positive or a spacing or radius below zero.
    pub(super) fn check_measurements(&self) -> Result<(), ThemeError> {
        let sizes = self
            .typography
            .iter()
            .map(|(role, kind)| (kind.size > 0.0, format!("/type/{}/size", pointer(role))));
        let spacing = self
            .spacing
            .iter()
            .map(|(role, value)| (*value >= 0.0, format!("/spacing/{}", pointer(role))));
        let radii = self
            .radii
            .iter()
            .map(|(role, value)| (*value >= 0.0, format!("/radii/{}", pointer(role))));
        match sizes.chain(spacing).chain(radii).find(|(valid, _)| !valid) {
            Some((_, path)) => Err(ThemeError::message(format!(
                "Invalid brand measurement: {path}"
            ))),
            None => Ok(()),
        }
    }
}

/// Escape a JSON Pointer reference token.
fn pointer(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}
