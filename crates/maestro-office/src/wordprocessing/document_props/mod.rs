//! Selected Word package properties.
mod core_props;
mod core_props_part;
pub use core_props::CorePropsDeclaration;
pub use core_props_part::CorePropsPart;
mod extended_props;
mod extended_props_part;
pub use extended_props::ExtendedPropsDeclaration;
pub use extended_props_part::ExtendedPropsPart;
mod custom_props;
mod custom_props_part;
pub use custom_props::CustomProperty;
pub use custom_props_part::CustomPropsPart;
