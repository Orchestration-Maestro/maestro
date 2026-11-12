//! Footer metadata lent to a factory ends with its capability.
use crate::{
    footer_data::Request, local_resources::Pump, read_imports::ReadEndpoint, tests::ended_errors,
};
use maestro_extensions::footer::{ExtensionStatuses, ReadonlyFooterDataProvider};
use std::rc::Rc;

/// Footer metadata with fixed values.
struct Fixed;

impl ReadonlyFooterDataProvider for Fixed {
    fn get_git_branch(&self) -> Option<String> {
        Some("main".into())
    }

    fn get_extension_statuses(&self) -> ExtensionStatuses {
        ExtensionStatuses::new()
    }

    fn get_available_provider_count(&self) -> f64 {
        2.0
    }

    fn on_branch_change(&self, _callback: Rc<dyn Fn()>) -> Box<dyn Fn()> {
        Box::new(|| {})
    }
}

#[test]
fn released_footer_rejects_every_kind_of_operation_with_its_ended_text() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let footer = pump.registry().footer(Rc::new(Fixed)).unwrap();
    let lease = footer.0.clone();
    let errors = ended_errors(lease, pump, admission, move || {
        [
            footer.read(Request::Branch).unwrap_err(),
            footer.read(Request::Statuses).unwrap_err(),
            footer.read(Request::Providers).unwrap_err(),
        ]
    });
    assert_eq!(errors, ["extension footer resource ended"; 3]);
}
