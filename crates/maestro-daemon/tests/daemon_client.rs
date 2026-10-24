//! Retained daemon targets reach their own native socket endpoints.
#![cfg(all(test, unix))]

use maestro_daemon::api::client::{ApiClientError, ConnectionTarget, DaemonClient};
use maestro_daemon::instances::{Selection, SelectionEnvironment};
use maestro_protocol::{
    rpc_types::{Request, RpcCommand},
    schema::PingParams,
};
use maestro_test_directory::Scratch;
use std::os::unix::net::UnixListener;
use std::{
    io::{BufRead, BufReader, Write},
    path::Path,
};

/// A live selected endpoint returns a distinguishable reply while the default endpoint
/// remains bound; the peer also checks canonical request framing.
fn selected_request(
    root: &Path,
    path: &Path,
    client: &DaemonClient,
) -> Result<(), Box<dyn std::error::Error>> {
    let default_path = root.join("daemon.sock");
    let default = reply(
        UnixListener::bind(&default_path)?,
        b"{\"endpoint\":\"default\"}\n",
    );
    std::fs::create_dir_all(
        path.parent()
            .ok_or_else(|| std::io::Error::other("missing parent"))?,
    )?;
    let selected = reply(UnixListener::bind(path)?, b"{\"endpoint\":\"selected\"}\n");
    let request = Request {
        id: Some("target".into()),
        command: RpcCommand::Ping(PingParams::default()),
    };
    let response = client.request_value(&request)?;
    // Complete the unused peer too, so both threads settle before the assertion.
    let unused = if response["endpoint"] == "selected" {
        &default_path
    } else {
        path
    };
    let cleanup = DaemonClient::for_target(ConnectionTarget::SocketPath(unused.into()));
    let cleaned = cleanup.request_value(&request);
    let selected_request = selected
        .join()
        .map_err(|_| std::io::Error::other("selected peer panicked"))??;
    let default_request = default
        .join()
        .map_err(|_| std::io::Error::other("default peer panicked"))??;
    assert_eq!(selected_request, "{\"id\":\"target\",\"type\":\"ping\"}\n");
    assert_eq!(default_request, "{\"id\":\"target\",\"type\":\"ping\"}\n");
    cleaned?;
    assert_eq!(response["endpoint"], "selected");
    Ok(())
}

#[test]
fn named_daemon_client_uses_the_supplied_path_table() {
    let root = Scratch::new().unwrap();
    let selection = Selection::from_args(
        root.path().to_str().unwrap(),
        &[],
        Some("work"),
        SelectionEnvironment::default(),
    )
    .unwrap();
    let endpoint = std::path::PathBuf::from(&selection.api_socket_path);
    let client = DaemonClient::local(selection);
    assert_eq!(client.socket_path(), endpoint);
    selected_request(root.path(), &endpoint, &client).unwrap();
}

#[test]
fn explicit_client_socket_paths_keep_native_spelling() {
    let root = Scratch::new().unwrap();
    std::fs::create_dir(root.path().join("selected")).unwrap();
    let endpoint = root.path().join("selected/../explicit.sock");
    let client = DaemonClient::for_target(ConnectionTarget::SocketPath(endpoint.clone()));
    assert_eq!(client.socket_path().as_os_str(), endpoint.as_os_str());
    selected_request(root.path(), &endpoint, &client).unwrap();
}

/// Accepts one request and supplies the exact response bytes from a controlled peer.
fn reply(
    listener: UnixListener,
    response: &'static [u8],
) -> std::thread::JoinHandle<std::io::Result<String>> {
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept()?;
        let mut line = String::new();
        BufReader::new(&mut stream).read_line(&mut line)?;
        stream.write_all(response)?;
        stream.flush()?;
        Ok(line)
    })
}

#[test]
fn raw_client_failures_keep_native_causes() {
    use std::error::Error;

    let root = Scratch::new().unwrap();
    let request = Request {
        id: Some("target".into()),
        command: RpcCommand::Ping(PingParams::default()),
    };
    for (name, response) in [
        ("json", b"not json\n".as_slice()),
        ("empty", b"\n"),
        ("eof", b""),
    ] {
        let path = root.path().join(name);
        let peer = reply(UnixListener::bind(&path).unwrap(), response);
        let client = DaemonClient::for_target(ConnectionTarget::SocketPath(path));
        let error = client.request_value(&request).unwrap_err();
        assert_eq!(
            peer.join().unwrap().unwrap(),
            "{\"id\":\"target\",\"type\":\"ping\"}\n"
        );
        if name == "json" {
            let ApiClientError::Json(cause) = &error else {
                panic!("JSON failure required");
            };
            assert!(cause.is_syntax());
            assert!(error.source().unwrap().is::<serde_json::Error>());
            assert_eq!(error.to_string(), cause.to_string());
        } else {
            assert!(matches!(error, ApiClientError::EmptyResponse));
            assert!(error.source().is_none());
            assert_eq!(error.to_string(), "empty api response");
        }
    }
    let client =
        DaemonClient::for_target(ConnectionTarget::SocketPath(root.path().join("missing")));
    let error = client.request_value(&request).unwrap_err();
    let ApiClientError::Io(cause) = &error else {
        panic!("native failure required");
    };
    assert!(cause.raw_os_error().is_some());
    assert!(error.source().unwrap().is::<std::io::Error>());
    assert_eq!(error.to_string(), cause.to_string());
}

#[test]
fn client_request_keeps_large_metadata_sequence() {
    let root = Scratch::new().unwrap();
    let path = root.path().join("sequence.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let peer = reply(listener, b"{\"success\":true}\n");
    let request = Request {
        id: Some("sequence".into()),
        command: RpcCommand::WorkspaceReportMetadata(
            maestro_protocol::schema::WorkspaceReportMetadataParams {
                workspace_id: "workspace".into(),
                source: "hook".into(),
                tokens: std::collections::HashMap::new(),
                seq: Some(9_007_199_254_740_993),
                ttl_ms: None,
            },
        ),
    };
    let client = DaemonClient::for_target(ConnectionTarget::SocketPath(path));
    let response = client.request_value(&request).unwrap();
    let received: serde_json::Value = serde_json::from_str(&peer.join().unwrap().unwrap()).unwrap();
    assert_eq!(response["success"], true);
    assert_eq!(received["seq"], 9_007_199_254_740_993_u64);
}
