// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod claude_3_5_haiku_20241022;
mod claude_opus_4_0;
mod claude_sonnet_4_5;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(claude_3_5_haiku_20241022::models());
    models.extend(claude_opus_4_0::models());
    models.extend(claude_sonnet_4_5::models());
    models
}
