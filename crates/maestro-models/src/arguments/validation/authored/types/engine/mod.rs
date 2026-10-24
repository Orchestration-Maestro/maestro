//! The schema engine stages the authored conversion is built on.
pub(crate) mod cyclic;
pub(crate) mod enumeration;
pub(crate) mod evaluate;
pub(crate) mod instantiate;
pub(crate) mod patterns;
pub(crate) mod r#ref;
pub(crate) mod template_literal;

/// Tuple positions spelled as named object properties.
pub(crate) mod tuple;
