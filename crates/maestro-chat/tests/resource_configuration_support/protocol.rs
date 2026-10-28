//! Fallible composition and authored theme failures through both public routes.
use super::{harness::Harness, records::Scopes};
use maestro_app::{PathMetadata, ResolvedPaths, ResolvedResource, SourceOrigin, SourceScope};
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
use maestro_tui::Component;
use serde_json::json;
use std::{
    cell::{Cell, RefCell},
    error::Error,
    rc::Rc,
};
/// One resource retaining all typed provenance fields through the application edge.
pub(super) fn resolved(path: &str, base: Option<&str>) -> ResolvedPaths {
    ResolvedPaths {
        extensions: vec![ResolvedResource {
            path: path.into(),
            enabled: true,
            metadata: PathMetadata {
                source: "npm:target".into(),
                scope: SourceScope::User,
                origin: SourceOrigin::Package,
                base_dir: base.map(str::to_owned),
            },
        }],
        ..ResolvedPaths::default()
    }
}
/// Minimal distinguishable documents for the selected package.
pub(super) fn scopes() -> Scopes {
    Scopes {
        global: json!({"packages":["npm:target"]}),
        project: json!({"decoy":true}),
    }
}
/// A child returning the exact supplied owned error rather than reconstructing it.
#[derive(Default)]
struct Child {
    /// Original error moved out on the next fallible render.
    error: RefCell<Option<Box<dyn Error>>>,
    /// Number of inherited invalidations.
    invalidations: Cell<usize>,
}
impl Component for Child {
    fn render(&self, width: usize) -> Vec<String> {
        self.try_render(width).unwrap_or_default()
    }
    fn try_render(&self, _width: usize) -> Result<Vec<String>, Box<dyn Error>> {
        if let Some(error) = self.error.take() {
            Err(error)
        } else {
            Ok(vec!["supplied child row".into()])
        }
    }
    fn invalidate(&self) {
        self.invalidations.set(self.invalidations.get() + 1);
    }
}
/// Checks inherited rows, original error ownership, invalidation and live role failures.
pub fn container() {
    let _guard = crate::keybindings::bindings_guard();
    let mut resources = resolved("/pkg/extensions/a.wasm", Some("/pkg"));
    let mut disabled = resolved("/pkg/extensions/b.wasm", Some("/pkg"));
    disabled.extensions[0].enabled = false;
    resources.extensions.extend(disabled.extensions);
    let harness = Harness::new(resources, &scopes(), ("/work", "/agent"), true);
    assert!(harness.component.input_handler().is_none());
    let children = harness.component.children();
    assert_eq!(children.borrow().len(), 8);
    let child = Rc::new(Child::default());
    harness.component.clear();
    harness.component.add_child(child.clone());
    assert_eq!(
        harness.component.try_render(80).unwrap(),
        ["supplied child row"]
    );
    let original: Box<dyn Error> = Box::new(std::io::Error::other("supplied child failure"));
    let address = std::ptr::from_ref(original.as_ref());
    child.error.replace(Some(original));
    let returned = harness.component.try_render(80).unwrap_err();
    assert!(std::ptr::addr_eq(
        address,
        std::ptr::from_ref(returned.as_ref())
    ));
    harness.component.invalidate();
    assert_eq!(child.invalidations.get(), 1);
    child.error.replace(Some(Box::new(std::io::Error::other(
        "legacy child failure",
    ))));
    assert!(harness.component.render(80).is_empty());
    harness.component.set_children(children);
    missing_roles(&harness);
    harness.persisted(&scopes());
}
/// Each foreground failure reaches both routes and legacy rendering yields no frame.
fn missing_roles(harness: &Harness) {
    let roles = [
        ThemeColor::Dim,
        ThemeColor::Muted,
        ThemeColor::Accent,
        ThemeColor::Success,
    ];
    for missing in &roles {
        let colors = roles
            .iter()
            .filter(|role| *role != missing)
            .cloned()
            .map(|role| (role, ColorValue::Index(31)))
            .chain([(ThemeColor::Border, ColorValue::Index(32))]);
        let theme =
            Rc::new(Theme::new(colors, [], ColorMode::Truecolor, ThemeOptions::default()).unwrap());
        harness.publication.set_theme_instance(theme).unwrap();
        let expected = format!("Unknown theme color: {}", missing.as_str());
        assert_eq!(
            harness.component.try_render(80).unwrap_err().to_string(),
            expected
        );
        assert_eq!(
            harness
                .component
                .get_resource_list()
                .try_render(80)
                .unwrap_err()
                .to_string(),
            expected
        );
        assert!(harness.component.render(80).is_empty());
        assert!(harness.component.get_resource_list().render(80).is_empty());
    }
}

/// A refusing publisher for resource preference edits.
struct RefuseToggle;
impl maestro_app::SettingsDecider for RefuseToggle {
    fn decide(&self, _: &maestro_app::DecisionRequest<'_>) -> Result<(), String> {
        Err("release refuses resource toggle".into())
    }
}

/// Refusal leaves scoped documents, rendered rows and callbacks untouched.
pub fn refused_toggles() {
    let _guard = crate::keybindings::bindings_guard();
    for scope in [SourceScope::User, SourceScope::Project] {
        for origin in [SourceOrigin::TopLevel, SourceOrigin::Package] {
            let mut resources = resolved("/pkg/extensions/a.wasm", Some("/pkg"));
            resources.extensions[0].metadata.scope = scope;
            resources.extensions[0].metadata.origin = origin;
            let scopes = Scopes {
                global: json!({"packages":["npm:target"]}),
                project: json!({"packages":["npm:target"]}),
            };
            let h = Harness::new(resources, &scopes, ("/work", "/agent"), true);
            h.settings
                .borrow_mut()
                .register_decider(std::sync::Arc::new(RefuseToggle))
                .unwrap();
            let list = h.component.get_resource_list();
            let before = list.try_render(80).unwrap();
            assert_eq!(
                list.handle_input(" ").unwrap_err().to_string(),
                "release refuses resource toggle"
            );
            assert_eq!(list.try_render(80).unwrap(), before);
            assert!(h.events.borrow().is_empty());
            h.persisted(&scopes);
        }
    }
}
