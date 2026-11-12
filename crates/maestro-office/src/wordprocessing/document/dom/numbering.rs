//! Prepared concrete numbering records.
use indexmap::IndexMap;
/// Prepared picture bullet reference.
#[derive(Clone, Debug)]
pub struct NumberingPicBullet {
    /// Numeric picture identifier.
    pub id: Option<f64>,
    /// Nullable image relationship.
    pub src: Option<String>,
    /// Nullable shape style.
    pub style: Option<String>,
}
/// One concrete numbering level with independent style maps.
#[derive(Clone, Debug)]
pub struct IDomNumbering {
    /// Concrete identifier; outer absence means undefined.
    pub id: Option<Option<String>>,
    /// Numeric level.
    pub level: Option<f64>,
    /// Numeric start.
    pub start: Option<f64>,
    /// Paragraph style; outer absence means undefined.
    pub p_style_name: Option<Option<String>>,
    /// Ordered paragraph presentation.
    pub p_style: IndexMap<String, Option<String>>,
    /// Ordered run presentation.
    pub r_style: IndexMap<String, Option<String>>,
    /// Encountered level text.
    pub level_text: Option<Option<String>>,
    /// Nullable suffix.
    pub suff: Option<String>,
    /// Encountered format.
    pub format: Option<Option<String>>,
    /// Encountered picture selection and nullable result.
    pub bullet: Option<Option<NumberingPicBullet>>,
}
