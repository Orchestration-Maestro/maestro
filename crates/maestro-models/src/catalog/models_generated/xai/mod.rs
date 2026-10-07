// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod grok_1;
mod grok_2;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(grok_1::models_0());
    models.extend(grok_1::models_10());
    models.extend(grok_2::models_20());
    models
}
