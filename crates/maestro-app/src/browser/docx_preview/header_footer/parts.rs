//! Header and footer preparation through the shared body parser.
use super::super::{DocxError, OwnedXmlElement, parse_body_elements};
use super::{WmlFooter, WmlHeader};
/// Prepared header content.
#[derive(Debug, Default)]
pub struct HeaderPart {
    /// Statically typed owned header root.
    pub root_element: WmlHeader,
}
impl HeaderPart {
    /// Prepare a header using the ordinary body parser.
    ///
    /// # Errors
    /// Propagates XML admission or selection failures.
    pub fn parse_xml(input: &OwnedXmlElement) -> Result<Self, DocxError> {
        Ok(Self {
            root_element: WmlHeader {
                children: parse_body_elements(input)?,
                ..Default::default()
            },
        })
    }
}

/// Prepared footer content.
#[derive(Debug, Default)]
pub struct FooterPart {
    /// Statically typed owned footer root.
    pub root_element: WmlFooter,
}
impl FooterPart {
    /// Prepare a footer using the ordinary body parser.
    ///
    /// # Errors
    /// Propagates XML admission or selection failures.
    pub fn parse_xml(input: &OwnedXmlElement) -> Result<Self, DocxError> {
        Ok(Self {
            root_element: WmlFooter {
                children: parse_body_elements(input)?,
                ..Default::default()
            },
        })
    }
}
