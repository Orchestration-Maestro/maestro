//! Explicit permission grants cross the real namespace and user-selected proxy routes.
use super::*;

/// Writes a complete project override before the example is loaded.
fn configuration(
    directory: &super::super::super::support::Root,
    network: serde_json::Value,
    filesystem: serde_json::Value,
) {
    std::fs::create_dir(directory.path("project/.maestro")).unwrap();
    std::fs::write(
        directory.path("project/.maestro/sandbox.json"),
        serde_json::Value::Object(
            [
                ("network".into(), network),
                ("filesystem".into(), filesystem),
                ("enableWeakerNestedSandbox".into(), json!(true)),
            ]
            .into_iter()
            .collect(),
        )
        .to_string(),
    )
    .unwrap();
}

#[test]
fn sandbox_explicit_unix_git_and_weaker_nesting_grants_take_effect() {
    if isolated("sandbox_explicit_unix_git_and_weaker_nesting_grants_take_effect") {
        return;
    }
    super::super::run(|local| async move {
        let directory = super::super::super::support::Root::new();
        std::fs::create_dir(directory.path("project/.git")).unwrap();
        std::fs::write(directory.path("project/.git/config"), "before").unwrap();
        configuration(
            &directory,
            json!({"allowAllUnixSockets":true}),
            json!({"allowWrite":["."],"denyWrite":[],"denyRead":[],"allowGitConfig":true}),
        );
        let socket = directory.path("project/host.sock");
        let listener = tokio::net::UnixListener::bind(&socket).unwrap();
        let session = super::super::loaded(&directory, local).await;
        let runner = session.extension_runner();
        let ui = Rc::new(Ui::default());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        close_after(&runner, async {
            event(&runner, true).await;
            assert!(
                ui.messages
                    .borrow()
                    .iter()
                    .any(|message| message == "Sandbox initialized")
            );
            let host_namespace = std::fs::read_link("/proc/self/ns/pid").unwrap();
            assert_eq!(
                bash(&runner, "readlink /proc/self/ns/pid").await.trim(),
                host_namespace.to_str().unwrap()
            );
            assert_eq!(
                bash(&runner, "printf permitted >.git/config; cat .git/config").await,
                "permitted"
            );
            let received = tokio::task::spawn_local(async move {
                let (mut peer, _) = listener.accept().await.unwrap();
                let mut bytes = Vec::new();
                peer.read_to_end(&mut bytes).await.unwrap();
                bytes
            });
            bash(
                &runner,
                "printf unix-grant | socat -u - UNIX-CONNECT:host.sock",
            )
            .await;
            assert_eq!(received.await.unwrap(), b"unix-grant");
        })
        .await;
    });
}

/// Echoes exactly one bridge connection without transferring ownership of its listening port.
async fn external_echo(listener: Rc<TcpListener>) {
    let (mut peer, _) = listener.accept().await.unwrap();
    let mut bytes = Vec::new();
    peer.read_to_end(&mut bytes).await.unwrap();
    peer.write_all(&bytes).await.unwrap();
    peer.shutdown().await.unwrap();
}

#[test]
fn sandbox_explicit_proxy_ports_remain_user_owned_and_forward_bytes() {
    if isolated("sandbox_explicit_proxy_ports_remain_user_owned_and_forward_bytes") {
        return;
    }
    super::super::run(|local| async move {
        let directory = super::super::super::support::Root::new();
        let http = Rc::new(TcpListener::bind("127.0.0.1:0").await.unwrap());
        let socks = Rc::new(TcpListener::bind("127.0.0.1:0").await.unwrap());
        let expected = [
            http.local_addr().unwrap().port(),
            socks.local_addr().unwrap().port(),
        ];
        configuration(
            &directory,
            json!({"allowedDomains":["127.0.0.1"],"httpProxyPort":expected[0],"socksProxyPort":expected[1]}),
            json!({"allowWrite":["."],"denyWrite":[],"denyRead":[]}),
        );
        let session = super::super::loaded(&directory, local).await;
        let runner = session.extension_runner();
        let ui = Rc::new(Ui::default());
        runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
        close_after(&runner, async {
            event(&runner, true).await;
            assert!(
                ui.messages
                    .borrow()
                    .iter()
                    .any(|message| message == "Sandbox initialized")
            );
            assert_eq!(ports(&runner).await, expected);
            let first = tokio::task::spawn_local(external_echo(http.clone()));
            assert_eq!(
                bash(&runner, "printf custom-http | socat - TCP4:127.0.0.1:3128").await,
                "custom-http"
            );
            first.await.unwrap();
            let second = tokio::task::spawn_local(external_echo(socks.clone()));
            assert_eq!(
                bash(&runner, "printf custom-socks | socat - TCP4:127.0.0.1:1080").await,
                "custom-socks"
            );
            second.await.unwrap();
        })
        .await;
        for port in expected {
            assert!(TcpStream::connect(("127.0.0.1", port)).await.is_ok());
        }
    });
}
