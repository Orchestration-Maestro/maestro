//! Native isolation and loopback proxy traffic through the shipped component.
use maestro_app::{
    Cancellation, EventError, ExtensionRunner, ExtensionUIContext, ExtensionUIContextPort, Local,
    NotificationType,
};
use serde_json::json;
use std::{cell::RefCell, path::Path, rc::Rc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

/// Notifications and the published sandbox status.
#[derive(Default)]
struct Ui {
    /// Messages delivered through the ordinary UI capability.
    messages: RefCell<Vec<String>>,
    /// Latest sandbox status.
    status: RefCell<Option<String>>,
    /// Severity of each notification.
    kinds: RefCell<Vec<Option<NotificationType>>>,
}
impl ExtensionUIContextPort for Ui {
    fn theme(&self) -> maestro_theme::LiveTheme {
        maestro_theme::LiveTheme::from_theme(Rc::new(
            maestro_theme::load_theme_from_path(
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../maestro-theme/assets/theme/dark.json"
                ),
                Some(maestro_theme::ColorMode::Truecolor),
            )
            .unwrap(),
        ))
    }
    fn notify(&self, message: String, kind: Option<NotificationType>) -> Result<(), EventError> {
        self.messages.borrow_mut().push(message);
        self.kinds.borrow_mut().push(kind);
        Ok(())
    }
    fn set_status(&self, key: String, text: Option<String>) -> Result<(), EventError> {
        assert_eq!(key, "sandbox");
        self.status.replace(text);
        Ok(())
    }
}

/// Supplies separate global and project layers without touching user settings.
fn fixture(root: &Path, agent: &Path) {
    std::fs::create_dir(agent.join("extensions")).unwrap();
    std::fs::create_dir(root.join(".maestro")).unwrap();
    std::fs::write(
        agent.join("extensions/sandbox.json"),
        json!({
            "network":{"allowedDomains":["127.0.0.1"]},
            "filesystem":{"denyRead":[".env.*","*.pem"]}
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(
        root.join(".maestro/sandbox.json"),
        json!({
            "network":{"deniedDomains":["localhost"]},
            "filesystem":{"allowWrite":[".","/tmp"]}
        })
        .to_string(),
    )
    .unwrap();
    std::fs::write(root.join(".env.local"), "secret-env").unwrap();
    std::fs::write(root.join("client.pem"), "secret-key").unwrap();
}

/// Executes the actual registered bash and returns its selected text.
async fn bash(runner: &Rc<ExtensionRunner<Local>>, command: &str) -> String {
    let tools = runner.get_all_registered_tools();
    let tool = maestro_app::wrap_registered_tool(&tools[0], runner.clone());
    let result = (tool.execute)(
        "sandbox-probe".into(),
        json!({"command":command}),
        None,
        None,
    )
    .await
    .unwrap();
    let content = serde_json::to_value(result.content).unwrap();
    content[0]["text"].as_str().unwrap().into()
}

/// Delivers an ordinary startup or shutdown lifecycle event.
async fn event(runner: &ExtensionRunner<Local>, start: bool) {
    if start {
        let mut event: maestro_app::SessionStartEvent =
            serde_json::from_value(json!({"reason":"startup"})).unwrap();
        runner.emit(&mut event).await.unwrap();
    } else {
        let mut event: maestro_app::SessionShutdownEvent =
            serde_json::from_value(json!({"reason":"quit"})).unwrap();
        runner.emit(&mut event).await.unwrap();
    }
}

/// Reads the actual host proxy ports exported to the isolated command.
async fn ports(runner: &Rc<ExtensionRunner<Local>>) -> [u16; 2] {
    let text = bash(runner, "printf '%s %s' \"$MAESTRO_SANDBOX_HOST_HTTP_PROXY_PORT\" \"$MAESTRO_SANDBOX_HOST_SOCKS_PROXY_PORT\"").await;
    let values: Vec<_> = text
        .split_whitespace()
        .map(|port| port.parse().unwrap())
        .collect();
    values.try_into().unwrap()
}

/// Verifies data hiding and write refusal, not platform-specific error wording.
async fn restrictions(runner: &Rc<ExtensionRunner<Local>>, root: &Path) {
    let command = "for file in .env.local client.pem; do if grep -q secret \"$file\" 2>/dev/null; then echo leaked; else echo hidden; fi; if (printf changed >>\"$file\") 2>/dev/null; then echo writable; else echo protected; fi; done";
    assert_eq!(
        bash(runner, command).await,
        "hidden\nprotected\nhidden\nprotected\n"
    );
    assert_eq!(
        std::fs::read_to_string(root.join(".env.local")).unwrap(),
        "secret-env"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("client.pem")).unwrap(),
        "secret-key"
    );
    assert_eq!(
        bash(runner, "printf new-secret > .env.created; cat .env.created").await,
        "new-secret"
    );
    assert_eq!(bash(runner, "cat .env.created").await, "(no output)");
    let _socket = std::os::unix::net::UnixListener::bind(root.join("host.sock")).unwrap();
    assert_eq!(
        bash(runner, "if socat -u /dev/null UNIX-CONNECT:host.sock 2>/dev/null; then echo bypass; else echo blocked; fi").await,
        "blocked\n"
    );
}

/// Echoes only admitted proxy connections; denied attempts must never arrive.
async fn echo(listener: TcpListener) {
    for _ in 0..3 {
        let (mut peer, _) = listener.accept().await.unwrap();
        let mut bytes = Vec::new();
        peer.read_to_end(&mut bytes).await.unwrap();
        peer.write_all(&bytes).await.unwrap();
        peer.shutdown().await.unwrap();
    }
}

/// Reads one complete proxy response header without consuming tunnel bytes.
async fn header(peer: &mut TcpStream) -> Vec<u8> {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        peer.read_exact(&mut byte).await.unwrap();
        bytes.push(byte[0]);
    }
    bytes
}

/// Proves the CONNECT admission decision is wired to actual socket forwarding.
async fn connect(proxy: u16, destination: u16, host: &str, allowed: bool) {
    let mut peer = TcpStream::connect(("127.0.0.1", proxy)).await.unwrap();
    peer.write_all(
        format!("CONNECT {host}:{destination} HTTP/1.1\r\nHost: {host}:{destination}\r\n\r\n")
            .as_bytes(),
    )
    .await
    .unwrap();
    let response = header(&mut peer).await;
    if allowed {
        assert!(response.starts_with(b"HTTP/1.1 200"), "{response:?}");
        tunnel(peer, b"connect-echo\0\xff").await;
    } else {
        assert!(response.starts_with(b"HTTP/1.1 403"), "{response:?}");
    }
}

/// Proves the SOCKS5 admission decision is wired to actual socket forwarding.
async fn socks(proxy: u16, destination: u16, host: &str, allowed: bool) {
    let mut peer = TcpStream::connect(("127.0.0.1", proxy)).await.unwrap();
    peer.write_all(&[5, 1, 0]).await.unwrap();
    let mut choice = [0; 2];
    peer.read_exact(&mut choice).await.unwrap();
    assert_eq!(choice, [5, 0]);
    let mut request = vec![5, 1, 0, 3, u8::try_from(host.len()).unwrap()];
    request.extend_from_slice(host.as_bytes());
    request.extend_from_slice(&destination.to_be_bytes());
    peer.write_all(&request).await.unwrap();
    let mut response = [0; 10];
    peer.read_exact(&mut response).await.unwrap();
    assert_eq!(response[0], 5);
    assert_eq!(response[1], if allowed { 0 } else { 2 });
    if allowed {
        tunnel(peer, b"socks-echo\0\xff").await;
    }
}

/// Half-closes the request and checks byte-exact forwarding in both directions.
async fn tunnel(mut peer: TcpStream, bytes: &[u8]) {
    peer.write_all(bytes).await.unwrap();
    peer.shutdown().await.unwrap();
    let mut reply = Vec::new();
    peer.read_to_end(&mut reply).await.unwrap();
    assert_eq!(reply, bytes);
}

/// Native timeout and cancellation remain policies of the existing bash owner.
async fn cancellation(runner: &Rc<ExtensionRunner<Local>>) {
    let registered = runner.get_all_registered_tools();
    let tool = maestro_app::wrap_registered_tool(&registered[0], runner.clone());
    let timeout = (tool.execute)(
        "timeout".into(),
        json!({"command":"exec /bin/sleep 60","timeout":0.001}),
        None,
        None,
    )
    .await
    .unwrap_err();
    assert!(timeout.to_string().contains("timed out"), "{timeout}");
    let signal = Cancellation::new();
    let abort = signal.clone();
    let update = Rc::new(move |_| {
        abort.abort();
    });
    let aborted = (tool.execute)(
        "cancel".into(),
        json!({"command":"printf ready; exec /bin/sleep 60"}),
        Some(signal),
        Some(update),
    )
    .await
    .unwrap_err();
    assert!(aborted.to_string().contains("aborted"), "{aborted}");
    assert_eq!(bash(runner, "printf still-running").await, "still-running");
}

/// The user-shell hook retains the same isolated execution capability as the tool.
async fn user_bash(runner: &ExtensionRunner<Local>, root: &Path) {
    let mut event: maestro_app::UserBashEvent = serde_json::from_value(json!({
        "command":"cat .env.local; printf user-shell", "cwd":root, "excludeFromContext":false
    }))
    .unwrap();
    let result = runner
        .emit_user_bash::<Rc<dyn maestro_app::BashOperations<Local>>, maestro_app::BashResult>(
            &mut event,
        )
        .await
        .unwrap()
        .unwrap();
    let operations = result.operations.unwrap();
    let mut bytes = Vec::new();
    let mut output = |chunk: &[u8]| bytes.extend_from_slice(chunk);
    let result = operations
        .exec(
            &event.command,
            root.to_str().unwrap(),
            maestro_app::BashExecOptions {
                on_data: &mut output,
                signal: None,
                timeout: None,
                env: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(result.exit_code, Some(0.0));
    assert_eq!(bytes, b"user-shell");
}

/// Runs the selected case below one disposable parent, with distinct caller and sandbox temp paths.
pub(super) fn isolated(test: &str) -> bool {
    if std::env::var_os("MAESTRO_SANDBOX_CHILD").is_some() {
        return false;
    }
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let root = scratch.path().join("project");
    let caller_temporary = scratch.path().join("caller-temporary");
    let sandbox_temporary = scratch.path().join("sandbox-temporary");
    std::fs::create_dir(&root).unwrap();
    std::fs::create_dir(&caller_temporary).unwrap();
    let output = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .nocapture()
        .stderr(std::process::Stdio::inherit())
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", &root)
        .env("SHELL", "/bin/bash")
        .env("TMPDIR", caller_temporary)
        .env("MAESTRO_TMPDIR", sandbox_temporary)
        .env("MAESTRO_SANDBOX_CHILD", "1")
        .env(
            "MAESTRO_SANDBOX_COMPONENT",
            std::env::var_os("MAESTRO_SANDBOX_COMPONENT").unwrap(),
        )
        .current_dir(root)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    true
}

#[test]
fn sandbox_session_wraps_bash_and_filters_loopback_traffic() {
    if isolated("sandbox_session_wraps_bash_and_filters_loopback_traffic") {
        return;
    }
    super::run(session);
}

#[test]
fn sandbox_temp_directory_keeps_artifacts_without_configured_write_paths() {
    if isolated("sandbox_temp_directory_keeps_artifacts_without_configured_write_paths") {
        return;
    }
    super::run(temporary_directory);
}

/// A supplied temporary directory is writable, private and retained after shutdown.
async fn temporary_directory(local: Rc<tokio::task::LocalSet>) {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let directory = super::super::support::Root::new();
    let root = directory.path("project");
    fixture(&root, &directory.path("agent"));
    std::fs::write(
        root.join(".maestro/sandbox.json"),
        json!({"filesystem":{"allowWrite":[],"denyRead":[],"denyWrite":[]}}).to_string(),
    )
    .unwrap();
    let session = super::loaded(&directory, local).await;
    let runner = session.extension_runner();
    let ui = Rc::new(Ui::default());
    runner.set_ui_context(Some(ExtensionUIContext::new(ui)));
    event(&runner, true).await;
    let path = bash(
        &runner,
        "file=$(mktemp) && printf kept >\"$file\" && printf '%s' \"$file\"",
    )
    .await;
    let temporary = std::env::var("MAESTRO_TMPDIR").unwrap();
    assert!(Path::new(&path).starts_with(&temporary), "{path}");
    let metadata = std::fs::metadata(&temporary).unwrap();
    assert!(metadata.is_dir());
    assert_eq!(metadata.uid(), std::fs::metadata(root).unwrap().uid());
    assert_eq!(metadata.permissions().mode() & 0o777, 0o700);
    event(&runner, false).await;
    assert_eq!(std::fs::read_to_string(path).unwrap(), "kept");
}

/// Exercises one initialized session through its tools, commands and lifecycle events.
async fn session(local: Rc<tokio::task::LocalSet>) {
    let directory = super::super::support::Root::new();
    let root = directory.path("project");
    fixture(&root, &directory.path("agent"));
    let session = super::loaded(&directory, local).await;
    let runner = session.extension_runner();
    let ui = Rc::new(Ui::default());
    runner.set_ui_context(Some(ExtensionUIContext::new(ui.clone())));
    let [http, socks_port] = close_after(&runner, async {
        eprintln!("sandbox: initialization");
        initialized(&runner, &ui).await;
        let controls = control_directories();
        assert_eq!(
            controls.len(),
            1,
            "retired control directories: {controls:?}"
        );
        eprintln!("sandbox: ports");
        let ports = ports(&runner).await;
        eprintln!("sandbox: filesystem");
        restrictions(&runner, &root).await;
        eprintln!("sandbox: proxies");
        proxy_traffic(&runner, ports[0], ports[1]).await;
        eprintln!("sandbox: cancellation");
        cancellation(&runner).await;
        eprintln!("sandbox: user bash");
        user_bash(&runner, &root).await;
        ports
    })
    .await;
    assert!(control_directories().is_empty());
    for port in [http, socks_port] {
        assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
    }
}

/// Observes only this checked child's own control artifacts.
fn control_directories() -> Vec<std::path::PathBuf> {
    std::fs::read_dir(std::env::var("TMPDIR").unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("unix-block.bpf").is_file())
        .collect()
}

/// Exercises both protocols with the same loopback destination and deny rule.
async fn proxy_traffic(runner: &Rc<ExtensionRunner<Local>>, http: u16, socks_port: u16) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let destination = listener.local_addr().unwrap().port();
    let echoed = tokio::task::spawn_local(echo(listener));
    eprintln!("sandbox: allowed CONNECT");
    connect(http, destination, "127.0.0.1", true).await;
    connect(http, destination, "localhost", false).await;
    eprintln!("sandbox: allowed SOCKS");
    socks(socks_port, destination, "127.0.0.1", true).await;
    socks(socks_port, destination, "localhost", false).await;
    assert_eq!(
        bash(runner, &format!("printf inside-bridge | socat - PROXY:127.0.0.1:127.0.0.1:{destination},proxyport=3128")).await,
        "inside-bridge"
    );
    echoed.await.unwrap();
}

/// Checks initialization and the effective layered configuration through the supplied UI.
async fn initialized(runner: &ExtensionRunner<Local>, ui: &Ui) {
    event(runner, true).await;
    assert!(
        ui.messages
            .borrow()
            .iter()
            .any(|message| message == "Sandbox initialized"),
        "{:?}",
        ui.messages.borrow()
    );
    assert!(
        ui.status
            .borrow()
            .as_ref()
            .unwrap()
            .contains("1 domains, 2 write paths")
    );
    let command = runner.get_command("sandbox").unwrap().command;
    (command.handler)(String::new(), runner.create_command_context())
        .await
        .unwrap();
    assert!(ui.messages.borrow().last().unwrap().contains("127.0.0.1"));
    assert!(ui.messages.borrow().last().unwrap().contains("localhost"));
}

mod lifecycle;

/// Releases all initialized services before invalidating their application context.
pub(super) async fn shutdown(runner: &ExtensionRunner<Local>) {
    event(runner, false).await;
}

/// Shuts down native services before propagating an assertion panic from the scenario.
async fn close_after<F: Future>(runner: &ExtensionRunner<Local>, action: F) -> F::Output {
    use std::{
        future::poll_fn,
        panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
        task::Poll,
    };
    let mut action = std::pin::pin!(action);
    let outcome = poll_fn(
        |cx| match catch_unwind(AssertUnwindSafe(|| action.as_mut().poll(cx))) {
            Ok(Poll::Pending) => Poll::Pending,
            Ok(Poll::Ready(value)) => Poll::Ready(Ok(value)),
            Err(error) => Poll::Ready(Err(error)),
        },
    )
    .await;
    event(runner, false).await;
    match outcome {
        Ok(value) => value,
        Err(error) => resume_unwind(error),
    }
}

mod permissions;
