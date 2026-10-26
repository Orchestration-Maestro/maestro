//! Typed native operation results share the existing routed continuation.
use wasmtime::component::{ComponentType, Lower, Resource, ResourceTable};

/// Native results remain their original owners until the guest accepts the route's result.
pub(crate) enum Value {
    /// A completed mutation.
    Unit,
    /// A configured-auth observation.
    Boolean(bool),
    /// An encoded shared record.
    Text(String),
    /// Rendered rows.
    Lines(Vec<String>),
    /// The lines and cursor after a completion was applied.
    Completion(crate::guest_calls::providers::Completed),
    /// A file-completion decision, absent without one.
    Trigger(Option<bool>),
    /// Independently retained credentials.
    Credentials(crate::credential_reads::Endpoint),
    /// Independently retained catalog.
    Catalog(crate::catalog_reads::Endpoint),
}
/// A result transferred to the guest through the same resource table.
#[derive(ComponentType, Lower)]
#[component(variant)]
pub(super) enum Transferred {
    /// A completed mutation.
    #[component(name = "unit")]
    Unit,
    /// A configured-auth observation.
    #[component(name = "boolean")]
    Boolean(bool),
    /// An encoded shared record.
    #[component(name = "text")]
    Text(String),
    /// Rendered rows.
    #[component(name = "lines")]
    Lines(Vec<String>),
    /// The lines and cursor after a completion was applied.
    #[component(name = "completion")]
    Completion(crate::guest_calls::providers::Completed),
    /// A file-completion decision, absent without one.
    #[component(name = "trigger")]
    Trigger(Option<bool>),
    /// Independently retained credentials.
    #[component(name = "credentials")]
    Credentials(Resource<crate::credential_reads::Endpoint>),
    /// Independently retained catalog.
    #[component(name = "catalog")]
    Catalog(Resource<crate::catalog_reads::Endpoint>),
}
impl Value {
    /// Transfers one actual result, without reducing non-unit success to unit.
    pub(super) fn transfer(self, table: &mut ResourceTable) -> wasmtime::Result<Transferred> {
        Ok(match self {
            Self::Unit => Transferred::Unit,
            Self::Boolean(value) => Transferred::Boolean(value),
            Self::Text(value) => Transferred::Text(value),
            Self::Lines(value) => Transferred::Lines(value),
            Self::Completion(value) => Transferred::Completion(value),
            Self::Trigger(value) => Transferred::Trigger(value),
            Self::Credentials(value) => Transferred::Credentials(table.push(value)?),
            Self::Catalog(value) => Transferred::Catalog(table.push(value)?),
        })
    }
}
/// Declares direct type-preserving native result admissions.
macro_rules! returns {
    ($($ty:ty => $kind:ident),* $(,)?) => {$(
        impl From<$ty> for Value {
            fn from(value: $ty) -> Self { Self::$kind(value) }
        }
    )*};
}
returns! { bool => Boolean, String => Text, Vec<String> => Lines, crate::guest_calls::providers::Completed => Completion, Option<bool> => Trigger, crate::credential_reads::Endpoint => Credentials, crate::catalog_reads::Endpoint => Catalog }
impl From<()> for Value {
    fn from((): ()) -> Self {
        Self::Unit
    }
}
