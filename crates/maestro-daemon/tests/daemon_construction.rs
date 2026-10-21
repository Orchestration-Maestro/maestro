//! Initial daemon presentation preferences come from the accepted configuration.
#![cfg(test)]
#![cfg(unix)]

#[path = "support/launch.rs"]
mod launch;

use maestro_daemon::{
    api::event_hub::EventHub,
    app::{App, state::AgentPanelSort},
    settings::model::{AgentPanelSortConfig, Config},
};
use std::sync::{Arc, RwLock};

/// Constructs the real daemon owner without creating a workspace or native child.
fn app(config: &Config) -> App<fn() -> u128> {
    let manifests = Arc::new(RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = launch::launch_inputs(&[], manifests);
    App::new(
        config,
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        EventHub::default(),
    )
}

#[test]
fn construction_uses_configured_agent_panel_sort() {
    let mut config = Config::default();
    config.ui.agent_panel_sort = AgentPanelSortConfig::Priority;
    assert_eq!(
        app(&config).state.agent_panel_sort,
        AgentPanelSort::Priority
    );
}

#[test]
fn construction_uses_configured_sidebar_state() {
    let mut config = Config::default();
    config.ui.sidebar_start_collapsed.0 = true;
    assert!(app(&config).state.sidebar_collapsed.0);
}

#[test]
fn construction_uses_redraw_on_focus_gained_config() {
    let mut config = Config::default();
    config.ui.redraw_on_focus_gained.0 = false;
    assert!(!app(&config).state.redraw_on_focus_gained.0);
}
