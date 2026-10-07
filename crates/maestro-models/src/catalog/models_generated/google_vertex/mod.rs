// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod gemini_1_5_flash;
mod gemini_3_pro_preview;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(gemini_1_5_flash::models());
    models.extend(gemini_3_pro_preview::models());
    models
}
