//! Typed brand pack records and their native decoding.
use crate::BrandError;
use maestro_json::escape_pointer_token;
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
    /// Authored size in CSS pixels, checked by [`PackData::parse`].
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
    /// Authored default template path; native consumers may join it to a directory.
    pub template: String,
    /// Authored variant paths by name; native consumers may join them to a directory.
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
pub(crate) struct GlyphData {
    /// The symbol text.
    pub symbol: String,
    /// Mode role alias of its color.
    pub color: String,
}

/// The colors of one mode: role to palette key.
#[derive(Deserialize)]
pub(crate) struct ModeData {
    /// Role to palette key.
    pub colors: BTreeMap<String, String>,
}

/// Terminal alias maps: slot or name to mode role.
#[derive(Deserialize)]
pub(crate) struct TerminalData {
    /// The ANSI slots.
    pub ansi: BTreeMap<String, String>,
    /// The truecolor names.
    pub truecolor: BTreeMap<String, String>,
}

/// Decoded brand data; use [`PackData::parse`] for complete pack admission.
#[derive(Deserialize)]
pub struct PackData {
    /// Pack label.
    pub(crate) name: String,
    /// Displayed product name.
    pub(crate) wordmark: String,
    /// Displayed tagline.
    pub(crate) tagline: String,
    /// Palette key to opaque color text.
    pub(crate) palette: BTreeMap<String, String>,
    /// Mode name to its colors.
    #[serde(deserialize_with = "records")]
    pub(crate) modes: BTreeMap<String, ModeData>,
    /// Font role to font.
    #[serde(deserialize_with = "records")]
    pub(crate) fonts: BTreeMap<String, BrandFont>,
    /// Type role to type.
    #[serde(rename = "type", deserialize_with = "records")]
    pub(crate) typography: BTreeMap<String, BrandType>,
    /// Spacing role to CSS pixels.
    pub(crate) spacing: BTreeMap<String, f64>,
    /// Radius role to CSS pixels.
    pub(crate) radii: BTreeMap<String, f64>,
    /// Mark paths and measurements.
    #[serde(deserialize_with = "record")]
    pub mark: BrandMark,
    /// Glyph role to glyph.
    #[serde(deserialize_with = "records")]
    pub(crate) glyphs: BTreeMap<String, GlyphData>,
    /// Terminal alias maps.
    #[serde(deserialize_with = "record")]
    pub(crate) terminal: TerminalData,
}

/// A record accepted only in its object form, never as a positional array.
struct Object<T>(T);

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Value::deserialize(deserializer)?;
        let is_object = value.is_object();
        maestro_json::object::record(value)
            .map(Self)
            .map_err(|cause| {
                if is_object {
                    D::Error::custom(cause)
                } else {
                    D::Error::custom("expected an object record")
                }
            })
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
    /// Decode an object pack, keeping final duplicate values, then check measurements.
    ///
    /// # Errors
    /// Returns a decoder error for malformed or wrongly shaped data, or a measurement error
    /// for a nonpositive type size or negative spacing or radius.
    pub fn parse(text: &str) -> Result<Self, BrandError> {
        let Object(pack) =
            serde_json::from_str::<Object<Self>>(text).map_err(BrandError::Decode)?;
        pack.check_measurements()?;
        Ok(pack)
    }

    /// Reject a type size that is not positive or a spacing or radius below zero.
    fn check_measurements(&self) -> Result<(), BrandError> {
        let sizes = self.typography.iter().map(|(role, kind)| {
            (
                kind.size > 0.0,
                format!("/type/{}/size", escape_pointer_token(role)),
            )
        });
        let spacing = self.spacing.iter().map(|(role, value)| {
            (
                *value >= 0.0,
                format!("/spacing/{}", escape_pointer_token(role)),
            )
        });
        let radii = self.radii.iter().map(|(role, value)| {
            (
                *value >= 0.0,
                format!("/radii/{}", escape_pointer_token(role)),
            )
        });
        match sizes.chain(spacing).chain(radii).find(|(valid, _)| !valid) {
            Some((_, path)) => Err(BrandError::Measurement(path)),
            None => Ok(()),
        }
    }
}
