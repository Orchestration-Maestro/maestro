// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod grok_2;
mod grok_3_mini;
mod grok_4_20_0309_reasoning;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(grok_2::models());
    models.extend(grok_3_mini::models());
    models.extend(grok_4_20_0309_reasoning::models());
    models
}
