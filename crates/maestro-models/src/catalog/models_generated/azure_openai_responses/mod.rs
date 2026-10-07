// Generated model descriptor data.

use crate::Model;
use indexmap::IndexMap;
mod gpt_4;
mod gpt_5;
mod gpt_5_1_codex_mini;
mod gpt_5_4_nano;
mod o1;
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(gpt_4::models());
    models.extend(gpt_5::models());
    models.extend(gpt_5_1_codex_mini::models());
    models.extend(gpt_5_4_nano::models());
    models.extend(o1::models());
    models
}
