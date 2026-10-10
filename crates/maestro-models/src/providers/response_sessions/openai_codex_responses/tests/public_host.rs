//! Native identity as observed on a public request.

#[test]
fn maestro_response_sessions_request_ids_and_host_identity() {
    let _guard = super::exclusive();
    super::socket_transport::run_native(
        maestro_response_sessions_request_ids_and_host_identity_scenario(),
    );
}

/// Execute the controlled public-operation scenario.
async fn maestro_response_sessions_request_ids_and_host_identity_scenario() {
    let (model, context, mut options) = super::invocation();
    options.common.transport = Some(crate::Transport::Sse);
    let (sender, receiver) = std::sync::mpsc::channel();
    options.common.fetch = Some(std::sync::Arc::new(move |request| {
        sender.send(request.headers).unwrap();
        Box::pin(async { Ok(super::public_streams::response()) })
    }));
    let stream =
        super::super::stream_openai_codex_responses((*model).clone(), context, Some(options));
    assert_eq!(
        stream.result().await.read().unwrap().stop_reason,
        crate::StopReason::Stop
    );
    let headers = receiver.recv().unwrap();
    let release = std::process::Command::new("uname")
        .arg("-r")
        .output()
        .unwrap();
    let release = std::str::from_utf8(&release.stdout).unwrap().trim_end();
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "ia32",
        "aarch64" => "arm64",
        arch => arch,
    };
    let platform = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "win32",
        os => os,
    };
    let expected = format!("maestro ({platform} {release}; {arch})");
    assert!(
        headers
            .get("user-agent")
            .is_some_and(|value| *value == expected),
        "native User-Agent differs"
    );
    assert!(
        headers
            .get("originator")
            .is_some_and(|value| value == "maestro"),
        "originator differs"
    );
    socket_identities().await;
}

/// Observe random and explicit request identifiers on real upgrades.
async fn socket_identities() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (model, context, mut options) = super::invocation();
    let mut model = (*model).clone();
    model.base_url = format!("http://{}", listener.local_addr().unwrap());
    let (sender, receiver) = std::sync::mpsc::channel();
    let peer = tokio::spawn(async move {
        use futures_util::StreamExt as _;
        for _ in 0..3 {
            let mut socket = tokio_tungstenite::accept_hdr_async(
                listener.accept().await.unwrap().0,
                IdentityCapture {
                    sender: sender.clone(),
                },
            )
            .await
            .unwrap();
            super::loopback::answer(&mut socket, "host", "identity").await;
            while socket.next().await.is_some() {}
        }
    });
    let mut ids = Vec::new();
    for session in [None, None, Some("explicit-host")] {
        options.common.session_id = session.map(str::to_owned);
        let output = super::super::stream_openai_codex_responses(
            model.clone(),
            context.clone(),
            Some(options.clone()),
        )
        .result()
        .await;
        assert_eq!(output.read().unwrap().stop_reason, crate::StopReason::Stop);
        super::super::close_openai_codex_web_socket_sessions(session);
        let (id, affinity) = receiver.recv().unwrap();
        assert!(id == affinity, "affinity identity differs");
        if session.is_none() {
            let parsed = uuid::Uuid::parse_str(&id).unwrap();
            assert_eq!(parsed.get_version_num(), 4);
            assert_eq!(parsed.get_variant(), uuid::Variant::RFC4122);
            assert_eq!(id.len(), 36);
            ids.push(id);
        } else {
            assert!(id == "explicit-host", "explicit affinity was replaced");
        }
    }
    assert!(
        ids[0] != ids[1],
        "successive request identifiers must differ"
    );
    peer.await.unwrap();
}

/// Upgrade observer retaining only the two noncredential identity fields.
struct IdentityCapture {
    /// Selected identity observations.
    sender: std::sync::mpsc::Sender<(String, String)>,
}
impl tokio_tungstenite::tungstenite::handshake::server::Callback for IdentityCapture {
    fn on_request(
        self,
        request: &tokio_tungstenite::tungstenite::handshake::server::Request,
        response: tokio_tungstenite::tungstenite::handshake::server::Response,
    ) -> Result<
        tokio_tungstenite::tungstenite::handshake::server::Response,
        tokio_tungstenite::tungstenite::handshake::server::ErrorResponse,
    > {
        let id = request.headers()["x-client-request-id"]
            .to_str()
            .unwrap()
            .to_owned();
        let session = request.headers()["session_id"].to_str().unwrap().to_owned();
        self.sender.send((id, session)).unwrap();
        Ok(response)
    }
}
