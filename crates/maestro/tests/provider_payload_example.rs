//! Provider observations from the shipped component and a controlled HTTP server.
use crate::support::{Root, provider::Provider, rpc::Rpc};
use serde_json::Value;

#[test]
fn provider_payload_appends_actual_requests_and_response_headers() {
    let root = Root::new();
    let provider = Provider::new();
    root.models(&provider.base);
    std::fs::create_dir(root.path("project/.maestro")).unwrap();
    let log = root.path("project/.maestro/provider-payload.log");
    std::fs::write(&log, "existing\n\n").unwrap();
    let component = std::env::var("MAESTRO_PROVIDER_PAYLOAD_COMPONENT")
        .expect("nextest component setup builds provider_payload");
    let mut rpc = Rpc::command(&mut root.command(&[
        "--mode",
        "rpc",
        "--provider",
        "startup",
        "--model",
        "controlled",
        "--extension",
        &component,
    ]));
    rpc.prompt("First controlled prompt Ω");
    let first = std::fs::read_to_string(&log).unwrap();
    assert!(first.starts_with("existing\n\n{\n  "));
    rpc.prompt("Second controlled prompt");
    let appended = std::fs::read_to_string(&log).unwrap();
    assert!(
        appended.starts_with(&first),
        "the second request must append"
    );
    let records: Vec<_> = appended.split("\n\n").collect();
    assert_eq!(
        records.len(),
        6,
        "prefix, two record pairs, trailing separator"
    );
    let requests = provider.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    for (index, request) in requests.iter().enumerate() {
        assert_eq!(
            serde_json::from_str::<Value>(records[1 + index * 2]).unwrap(),
            *request
        );
        let response = records[2 + index * 2];
        assert!(!response.contains('\n'), "response headers stay compact");
        let headers: Value =
            serde_json::from_str(response.strip_prefix("[200] ").unwrap()).unwrap();
        assert_eq!(headers["content-type"], "text/event-stream");
        assert_eq!(headers["connection"], "close");
        assert!(
            headers["content-length"]
                .as_str()
                .unwrap()
                .parse::<usize>()
                .unwrap()
                > 0
        );
        assert_eq!(headers.as_object().unwrap().len(), 3);
    }
    drop(requests);
    rpc.finish();
}
