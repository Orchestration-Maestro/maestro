//! Typed native operation results share the existing routed continuation.
use wasmtime::component::{ComponentType, Lower, Resource, ResourceTable};

/// Native results remain their original owners until the guest accepts the route's result.
pub(crate) enum Value {
    /// A completed mutation.
    Unit,
    /// Original native component.
    Component(crate::native_renders::ComponentEndpoint),
    /// Exact component output lines.
    Lines(Vec<String>),
    /// Native key-release preference.
    Key(bool),
    /// A configured-auth observation.
    Boolean(bool),
    /// An encoded shared record.
    Text(String),
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
    /// Independently retained credentials.
    #[component(name = "credentials")]
    Credentials(Resource<crate::credential_reads::Endpoint>),
    /// Independently retained catalog.
    #[component(name = "catalog")]
    Catalog(Resource<crate::catalog_reads::Endpoint>),
    /// Original native component.
    #[component(name = "component")]
    Component(Resource<crate::native_renders::ComponentEndpoint>),
    /// Exact component output lines.
    #[component(name = "lines")]
    Lines(Vec<String>),
    /// Native key-release preference.
    #[component(name = "key")]
    Key(bool),
}
impl Value {
    /// Transfers one actual result, without reducing non-unit success to unit.
    pub(super) fn transfer(self, table: &mut ResourceTable) -> wasmtime::Result<Transferred> {
        Ok(match self {
            Self::Unit => Transferred::Unit,
            Self::Component(value) => Transferred::Component(table.push(value)?),
            Self::Lines(value) => Transferred::Lines(value),
            Self::Key(value) => Transferred::Key(value),
            Self::Boolean(value) => Transferred::Boolean(value),
            Self::Text(value) => Transferred::Text(value),
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
returns! { bool => Boolean, String => Text, crate::credential_reads::Endpoint => Credentials, crate::catalog_reads::Endpoint => Catalog }
impl From<()> for Value {
    fn from((): ()) -> Self {
        Self::Unit
    }
}
