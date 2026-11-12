//! Controlled settings storage with the real daemon application owner.

use crate::{service::event_hub::EventHub, settings::model::Config, state::App};
use maestro_settings::{InMemorySettingsStorage, SettingsManager, SettingsScope, SettingsStorage};
use std::sync::{Arc, RwLock};

/// Replace the global document through the settings owner's locked seam.
pub fn put(storage: &dyn SettingsStorage, text: &str) {
    storage
        .with_lock(SettingsScope::Global, &mut |_| Ok(Some(text.into())))
        .unwrap();
}

/// Constructs a daemon and independently editable global settings storage.
pub fn fixture() -> (App, SettingsManager, Arc<InMemorySettingsStorage>) {
    let manifests = Arc::new(RwLock::new(
        maestro_detection::definitions::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = crate::state::tests::support::spawning::launch_inputs(&[], manifests);
    let app = App::new(
        &Config::default(),
        || 0,
        launch,
        receiver,
        EventHub::default(),
    );
    let storage = Arc::new(InMemorySettingsStorage::new());
    put(&*storage, "{}");
    let manager = SettingsManager::from_storage(storage.clone());
    (app, manager, storage)
}

pub(crate) use crate::tests::block_on;
