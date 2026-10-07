// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod mimo_v2_flash;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(mimo_v2_flash::models());
    models
}
