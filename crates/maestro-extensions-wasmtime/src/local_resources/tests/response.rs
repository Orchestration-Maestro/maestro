//! A native HTTP response ends with its capability.
use super::ended_endpoint;
use crate::local_resources::{Capability, http::Response};
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
    let errors = ended_endpoint(
        Capability::NativeResponse(Rc::new(Fixed)),
        Response,
        |response| {
            [
                response.status().unwrap_err(),
                response.status_text().unwrap_err(),
                response.headers().unwrap_err(),
                response.body().err().unwrap(),
            ]
        },
    );
    assert_eq!(errors, ["native response resource ended"; 4]);
}

#[test]
fn released_response_rejects_consumption_with_its_ended_text() {
    let errors = ended_endpoint(
        Capability::NativeResponse(Rc::new(Fixed)),
        Response,
        |response| {
            tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap()
                .block_on(async {
                    [
                        response.text().await.unwrap_err(),
                        response.json().await.unwrap_err(),
                    ]
                })
        },
    );
    assert_eq!(errors, ["native response resource ended"; 2]);
}
