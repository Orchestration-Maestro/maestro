#![doc = include_str!("../../../docs/ports.md")]

use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use maestro_settings::{InvalidAdapterSetting, SettingsManager};

/// A seam to a technology: its settings key and its default adapter.
///
/// The trait `T` is the contract: only a type that implements it can become
/// an adapter, so a missing operation is a compile error, never a runtime refusal.
pub struct Port<T: ?Sized> {
    /// The key below `adapters` in the settings.
    id: &'static str,
    /// The adapter selected while the settings name none.
    default: &'static str,
    /// The adapter type the port hands out.
    adapter: PhantomData<fn() -> Arc<T>>,
}

impl<T: ?Sized> Port<T> {
    /// Declares a port with its settings key and the name of its default adapter.
    #[must_use]
    pub const fn new(id: &'static str, default: &'static str) -> Self {
        Self {
            id,
            default,
            adapter: PhantomData,
        }
    }

    /// The key below `adapters` in the settings.
    #[must_use]
    pub const fn id(&self) -> &'static str {
        self.id
    }
}

/// Why a registration or a selection was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AdapterError {
    /// The port already has an adapter with this name.
    Duplicate {
        /// The port's settings key.
        port: &'static str,
        /// The repeated name.
        name: String,
    },
    /// The settings entry for the port is not a name.
    Setting(InvalidAdapterSetting),
    /// Settings select a name no adapter registered under the port.
    Unknown {
        /// The port's settings key.
        port: &'static str,
        /// The selected name.
        name: String,
        /// Names registered under the port.
        installed: Vec<String>,
    },
}

impl fmt::Display for AdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate { port, name } => {
                write!(formatter, "port `{port}` already has an adapter `{name}`")
            }
            Self::Setting(error) => error.fmt(formatter),
            Self::Unknown {
                port,
                name,
                installed,
            } => write!(
                formatter,
                "no adapter `{name}` for port `{port}`; installed: {}",
                installed.join(", ")
            ),
        }
    }
}

impl std::error::Error for AdapterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Setting(error) => Some(error),
            Self::Duplicate { .. } | Self::Unknown { .. } => None,
        }
    }
}

/// One registration as the installed report lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    /// The port's settings key.
    pub port: &'static str,
    /// The adapter's name.
    pub name: String,
}

/// A factory that builds the adapter a port hands out.
type Factory<T> = Box<dyn Fn() -> Arc<T> + Send + Sync>;

/// A registered adapter; the factory is typed by the port it was registered under.
struct Entry {
    /// The port's settings key.
    port: &'static str,
    /// The adapter's name.
    name: String,
    /// A [`Factory`] for the port's adapter type.
    factory: Box<dyn Any + Send + Sync>,
}

/// The adapters registered under every port, in registration order.
#[derive(Default)]
pub struct Registry {
    /// Registrations in the order they were admitted.
    entries: Vec<Entry>,
}

impl Registry {
    /// Admits an adapter under a port and a name.
    ///
    /// The port's trait is the contract. A type that lacks an operation cannot be
    /// returned as the port's adapter, so the registration does not compile:
    ///
    /// ```compile_fail,E0277
    /// use std::sync::Arc;
    /// use maestro_app::adapters::{Port, Registry};
    ///
    /// trait Store: Send + Sync {
    ///     fn get(&self, key: &str) -> Option<String>;
    /// }
    /// struct Lacking;
    /// const STORE: Port<dyn Store> = Port::new("store", "lacking");
    ///
    /// let mut registry = Registry::default();
    /// registry.register(&STORE, "lacking", || Arc::new(Lacking) as Arc<dyn Store>)?;
    /// # Ok::<(), maestro_app::adapters::AdapterError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::Duplicate`] when the port already has the name. A
    /// refused registration changes nothing.
    pub fn register<T: ?Sized + 'static>(
        &mut self,
        port: &Port<T>,
        name: &str,
        factory: impl Fn() -> Arc<T> + Send + Sync + 'static,
    ) -> Result<(), AdapterError> {
        if self
            .entries
            .iter()
            .any(|entry| entry.port == port.id && entry.name == name)
        {
            return Err(AdapterError::Duplicate {
                port: port.id,
                name: name.to_owned(),
            });
        }
        self.entries.push(Entry {
            port: port.id,
            name: name.to_owned(),
            factory: Box::new(Box::new(factory) as Factory<T>),
        });
        Ok(())
    }

    /// Builds the adapter the settings select for the port, or its default while
    /// `adapters.<port>` is unset.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::Setting`] when the settings entry is not a name and
    /// [`AdapterError::Unknown`] when no adapter registered the selected name; the
    /// second lists the installed names.
    pub fn select<T: ?Sized + 'static>(
        &self,
        port: &Port<T>,
        settings: &SettingsManager,
    ) -> Result<Arc<T>, AdapterError> {
        let name = settings
            .get_adapter(port.id)
            .map_err(AdapterError::Setting)?
            .unwrap_or_else(|| port.default.to_owned());
        self.entries
            .iter()
            .filter(|entry| entry.port == port.id && entry.name == name)
            .find_map(|entry| entry.factory.downcast_ref::<Factory<T>>())
            .map(|factory| factory())
            .ok_or_else(|| AdapterError::Unknown {
                port: port.id,
                name,
                installed: self.names(port),
            })
    }

    /// Names registered under the port with this adapter type.
    fn names<T: ?Sized + 'static>(&self, port: &Port<T>) -> Vec<String> {
        self.entries
            .iter()
            .filter(|entry| entry.port == port.id && entry.factory.is::<Factory<T>>())
            .map(|entry| entry.name.clone())
            .collect()
    }

    /// Lists every registration in the order it was admitted.
    #[must_use]
    pub fn installed(&self) -> Vec<Installed> {
        self.entries
            .iter()
            .map(|entry| Installed {
                port: entry.port,
                name: entry.name.clone(),
            })
            .collect()
    }
}
