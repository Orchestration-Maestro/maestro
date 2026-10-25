//! Controlled settings storage with the real daemon application owner.

use maestro_daemon::{api::event_hub::EventHub, app::App, settings::model::Config};
use maestro_settings::{InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage};
use std::sync::{Arc, RwLock};

/// Daemon owner with a deterministic identity clock.
pub type TestApp = App<fn() -> u128>;

/// Replace the global document through the settings owner's locked seam.
pub fn put(storage: &dyn SettingsStorage, text: &str) {
    storage
        .with_lock(SettingsScope::Global, &mut |_| Ok(Some(text.into())))
        .unwrap();
}

/// Constructs a daemon and independently editable global settings storage.
pub fn fixture() -> (TestApp, SettingsManager, Arc<InMemorySettingsStorage>) {
    let manifests = Arc::new(RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = crate::support::launch::launch_inputs(&[], manifests);
    let app = App::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        EventHub::default(),
    );
    let storage = Arc::new(InMemorySettingsStorage::new());
    put(&*storage, "{}");
    let manager = SettingsManager::from_storage(storage.clone());
    (app, manager, storage)
}

pub use crate::support::runtime::block_on;
