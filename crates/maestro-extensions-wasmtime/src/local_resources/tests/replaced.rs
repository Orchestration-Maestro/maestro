//! A replacement session's context ends with its capability.
use super::ended_endpoint;
use crate::{local_resources::Capability, replaced_reads::Endpoint, tests::runner};
use maestro_extensions::ReplacedSessionContext;
use std::rc::Rc;

#[test]
fn released_replaced_session_rejects_its_command_view_with_its_ended_text() {
    let context = ReplacedSessionContext::new(
        runner("/replaced").create_command_context(),
        Rc::new(|_, _| Box::pin(async { Ok(()) })),
        Rc::new(|_, _| Box::pin(async { Ok(()) })),
    );
    let errors = ended_endpoint(Capability::Replaced(context), Endpoint, |replaced| {
        [replaced.command().err().unwrap()]
    });
    assert_eq!(errors, ["extension replaced-session resource ended"]);
}
