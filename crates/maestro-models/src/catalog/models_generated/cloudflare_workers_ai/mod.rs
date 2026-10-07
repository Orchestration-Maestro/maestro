// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod cf_google_gemma_4_26b_a4b_it;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(cf_google_gemma_4_26b_a4b_it::models());
    models
}
