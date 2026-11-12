//! Terminal title synchronization preserves public observations and sidebar damage.
#![cfg(test)]
#![cfg(target_os = "linux")]

use maestro_daemon::state::test_support::spawning;
use maestro_daemon::{service::event_hub::EventHub, settings::model::Config, state::App};
use std::sync::{Arc, RwLock};

#[test]
fn daemon_sidebar_redraws_only_for_the_configured_title_form() {
    use maestro_daemon::{
        settings::navigation::AgentSidebarToken, state::terminal_titles::TerminalTitleChanges,
    };
    let manifests = Arc::new(RwLock::new(
        maestro_detection::definitions::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = spawning::launch_inputs(&[], manifests).unwrap();
    let mut app = App::new(
        &Config::default(),
        || 0,
        launch,
        receiver,
        EventHub::default(),
    );
    app.config.ui.sidebar.agents.rows = vec![vec![AgentSidebarToken::Agent]];
    app.config.ui.sidebar.agents.rows_by_agent.insert(
        "claude".into(),
        vec![vec![AgentSidebarToken::TerminalTitleStripped]],
    );
    let spinner_only = TerminalTitleChanges {
        raw_changed: true,
        stripped_changed: false,
    };
    assert!(!app.terminal_title_sidebar_changed(&spinner_only));
    assert!(app.terminal_title_sidebar_changed(&TerminalTitleChanges {
        raw_changed: false,
        stripped_changed: true
    }));
    app.config.ui.sidebar.agents.rows_by_agent.insert(
        "claude".into(),
        vec![vec![AgentSidebarToken::TerminalTitle]],
    );
    assert!(app.terminal_title_sidebar_changed(&spinner_only));
}
