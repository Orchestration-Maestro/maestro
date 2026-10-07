// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod deepseek_v4_flash;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(deepseek_v4_flash::models());
    models
}
