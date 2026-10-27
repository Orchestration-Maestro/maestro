//! Isolated missing-directory presentations over the shared terminal harness.
use super::presenters::{Harness, settings, theme, view};
use maestro_app::presentation_data::input_selectors::{
    ExtensionSelectorFactory, ExtensionSelectorRequest,
};
use maestro_app::{SessionCwdIssue, SettingsManager};
use maestro_cli::startup::main::presentation::MissingSessionCwdPresentation;
use maestro_test_directory::Scratch;
use maestro_test_process::{TestProcess, selection};
use std::{cell::RefCell, rc::Rc};

/// Run global keybinding construction and cleanup in a single checked test process.
pub fn isolated(name: &str) -> bool {
    if selection().is_child() {
        return false;
    }
    let scratch = Scratch::new().unwrap();
    std::fs::write(
        scratch.path().join("keybindings.json"),
        "{\n  \"tui.select.confirm\": \"ctrl+y\",\n  \"tui.select.cancel\": \"ctrl+x\"\n}\n",
    )
    .unwrap();
    let output = command(name, "presentation", scratch.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "isolated test failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    true
}

/// A checked child with the startup tests' cleared process environment.
pub fn command(name: &str, case: &str, root: &std::path::Path) -> TestProcess {
    TestProcess::current(name)
        .unwrap()
        .env_clear()
        .case(case)
        .env("HOME", root)
        .env(
            "MAESTRO_PACKAGE_DIR",
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .parent()
                .unwrap(),
        )
        .env("MAESTRO_CODING_AGENT_DIR", root)
}

/// One retained request slot for a presented dialog.
pub type Request = Rc<RefCell<Option<ExtensionSelectorRequest>>>;

/// The owners and observations of a pending dialog.
pub struct Dialog {
    /// Disposable theme documents.
    pub scratch: Scratch,
    /// Shared terminal effects.
    pub harness: Harness,
    /// Component factory's supplied request.
    pub captured: Request,
    /// Focus observations from the shared probe.
    pub frames: Rc<RefCell<Vec<String>>>,
    /// Settings read before writer construction.
    pub settings: Rc<RefCell<SettingsManager>>,
}
impl Dialog {
    /// Fresh terminal, theme documents and settings.
    pub fn new() -> Self {
        Self {
            scratch: Scratch::new().unwrap(),
            harness: Harness::new(),
            captured: Rc::default(),
            frames: Rc::default(),
            settings: settings(),
        }
    }
    /// Source-shaped missing-directory record.
    pub fn issue() -> SessionCwdIssue {
        SessionCwdIssue {
            session_file: Some("stored.jsonl".into()),
            session_cwd: "/gone".into(),
            fallback_cwd: "/fallback".into(),
        }
    }
    /// Capture the supplied request and return the existing focus probe.
    pub fn presentation(&self) -> MissingSessionCwdPresentation {
        let (captured, frames) = (Rc::clone(&self.captured), Rc::clone(&self.frames));
        let presenter: ExtensionSelectorFactory = Rc::new(move |request| {
            *captured.borrow_mut() = Some(request);
            Ok(view(&frames))
        });
        MissingSessionCwdPresentation {
            create_ui: self.harness.create_ui(),
            theme: theme(self.scratch.path()),
            presenter,
        }
    }
}

/// Check actual loading fallbacks without inventing a failing keybinding reader.
pub fn keybinding_defaults() {
    let agent = maestro_app::configuration::config::get_agent_dir().unwrap();
    let path = std::path::Path::new(&agent).join("keybindings.json");
    for malformed in [true, false] {
        std::fs::remove_file(&path).ok();
        if malformed {
            std::fs::write(&path, "{invalid").unwrap();
        } else {
            std::fs::create_dir(&path).unwrap();
        }
        let manager = maestro_app::KeybindingsManager::create(None).unwrap();
        assert!(manager.matches("\r", "tui.select.confirm"));
        assert!(manager.matches("\u{1b}", "tui.select.cancel"));
        if !malformed {
            std::fs::remove_dir(&path).unwrap();
        }
    }
}

impl Dialog {
    /// Exercise constructor policy separately from the later clear-on-shrink setter.
    pub fn policy_presentation(&self, policies: (bool, bool)) -> MissingSessionCwdPresentation {
        let (hardware, clear) = policies;
        self.harness
            .runtime
            .set_environment("MAESTRO_HARDWARE_CURSOR", if hardware { "0" } else { "1" });
        self.harness
            .runtime
            .set_environment("MAESTRO_CLEAR_ON_SHRINK", if clear { "0" } else { "1" });
        self.settings
            .borrow_mut()
            .set_show_hardware_cursor(hardware);
        self.settings.borrow_mut().set_clear_on_shrink(clear);
        let mut presentation = self.presentation();
        let create_ui = Rc::clone(&presentation.create_ui);
        presentation.create_ui = Rc::new(move |selected| {
            assert_eq!(selected, hardware);
            create_ui(selected)
        });
        let presenter = Rc::clone(&presentation.presenter);
        let terminal = self.harness.terminal.clone();
        presentation.presenter = Rc::new(move |request| {
            assert_eq!(
                request.title,
                maestro_app::format_missing_session_cwd_prompt(&Self::issue())
            );
            assert_eq!(request.options, ["Continue", "Cancel"]);
            assert!(maestro_tui::get_keybindings().matches("\u{19}", "tui.select.confirm"));
            assert!(maestro_tui::get_keybindings().matches("\u{18}", "tui.select.cancel"));
            assert_eq!(request.tui.get_show_hardware_cursor(), hardware);
            assert_eq!(request.tui.get_clear_on_shrink(), clear);
            assert!(
                terminal.writes().is_empty(),
                "construction must not call a cursor setter"
            );
            presenter(request)
        });
        presentation
    }
}
