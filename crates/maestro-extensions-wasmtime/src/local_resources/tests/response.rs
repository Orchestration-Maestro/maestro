//! A native HTTP response ends with its capability.
use super::Pump;
use crate::{
    local_resources::{Capability, http::Response},
    tests::ended_errors,
};
use maestro_extensions::native_io::{IoFuture, Readable, Response as NativeResponse};
use std::rc::Rc;

/// A response with fixed metadata and no body.
struct Fixed;

impl NativeResponse for Fixed {
    fn json(&self) -> IoFuture<'_, serde_json::Value> {
        Box::pin(async { Ok(serde_json::Value::Null) })
    }

    fn text(&self) -> IoFuture<'_, String> {
        Box::pin(async { Ok(String::new()) })
    }

    fn status(&self) -> u16 {
        204
    }

    fn status_text(&self) -> String {
        "No Content".into()
    }

    fn headers(&self) -> Vec<(String, String)> {
        Vec::new()
    }

    fn body(&self) -> Option<Rc<dyn Readable>> {
        None
    }
}

#[test]
fn released_response_rejects_every_kind_of_operation_with_its_ended_text() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let lease = pump
        .registry()
        .insert(Capability::NativeResponse(Rc::new(Fixed)))
        .unwrap();
    let response = Response(lease.clone());
    let errors = ended_errors(lease, pump, admission, move || {
        [
            response.status().unwrap_err(),
            response.status_text().unwrap_err(),
            response.headers().unwrap_err(),
            response.body().err().unwrap(),
        ]
    });
    assert_eq!(errors, ["native response resource ended"; 4]);
}
