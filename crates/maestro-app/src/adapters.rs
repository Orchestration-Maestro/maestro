#![doc = include_str!("../../../docs/ports.md")]

use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use maestro_settings::SettingsManager;

/// A seam to a technology: its settings key and the operations adapters must offer.
pub struct Port<T: ?Sized> {
    /// The key below `adapters` in the settings.
    id: &'static str,
    /// Operations every registration must declare.
    required: &'static [&'static str],
    /// The adapter type the port hands out.
    adapter: PhantomData<fn() -> Arc<T>>,
}

impl<T: ?Sized> Port<T> {
    /// Declares a port with its settings key and required operations.
    #[must_use]
    pub const fn new(id: &'static str, required: &'static [&'static str]) -> Self {
        Self {
            id,
            required,
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
    /// The adapter does not declare every operation the port requires.
    MissingOperations {
        /// The port's settings key.
        port: &'static str,
        /// The refused adapter's name.
        name: String,
        /// Required operations the adapter left out.
        missing: Vec<&'static str>,
    },
    /// Settings select no adapter for the port.
    NotSelected {
        /// The port's settings key.
        port: &'static str,
        /// Names registered under the port.
        installed: Vec<String>,
    },
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
            Self::MissingOperations {
                port,
                name,
                missing,
            } => write!(
                formatter,
                "adapter `{name}` for port `{port}` lacks required operations: {}",
                missing.join(", ")
            ),
            Self::NotSelected { port, installed } => write!(
                formatter,
                "no adapter selected for port `{port}`; installed: {}",
                installed.join(", ")
            ),
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

impl std::error::Error for AdapterError {}

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
    /// # Errors
    ///
    /// Returns [`AdapterError::Duplicate`] when the port already has the name, and
    /// [`AdapterError::MissingOperations`] when `operations` leaves out an operation
    /// the port requires. A refused registration changes nothing.
    pub fn register<T: ?Sized + 'static>(
        &mut self,
        port: &Port<T>,
        name: &str,
        operations: &[&str],
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
        let missing: Vec<&'static str> = port
            .required
            .iter()
            .copied()
            .filter(|required| !operations.contains(required))
            .collect();
        if !missing.is_empty() {
            return Err(AdapterError::MissingOperations {
                port: port.id,
                name: name.to_owned(),
                missing,
            });
        }
        self.entries.push(Entry {
            port: port.id,
            name: name.to_owned(),
            factory: Box::new(Box::new(factory) as Factory<T>),
        });
        Ok(())
    }

    /// Builds the adapter the settings select for the port.
    ///
    /// # Errors
    ///
    /// Returns [`AdapterError::NotSelected`] when `adapters.<port>` is unset and
    /// [`AdapterError::Unknown`] when no adapter registered that name; both list the
    /// installed names.
    pub fn select<T: ?Sized + 'static>(
        &self,
        port: &Port<T>,
        settings: &SettingsManager,
    ) -> Result<Arc<T>, AdapterError> {
        let Some(name) = settings.get_adapter(port.id) else {
            return Err(AdapterError::NotSelected {
                port: port.id,
                installed: self.names(port),
            });
        };
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
