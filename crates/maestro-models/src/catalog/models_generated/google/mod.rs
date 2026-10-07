// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod gemini_1_5_flash;
mod gemini_2_5_flash_preview_05_20;
mod gemini_flash_latest;
mod gemma_3_27b_it;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(gemini_1_5_flash::models());
    models.extend(gemini_2_5_flash_preview_05_20::models());
    models.extend(gemini_flash_latest::models());
    models.extend(gemma_3_27b_it::models());
    models
}
