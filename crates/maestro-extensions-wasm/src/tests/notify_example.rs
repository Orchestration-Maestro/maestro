//! The shipped notification hook with controlled environment, terminal bytes and children.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use wasmtime_wasi::{WasiCtxBuilder, p2::pipe::MemoryOutputPipe};

use super::{documents::Answer, host::Harness, host_family::guest};
use crate::child_process::Stdio;

#[path = "../../../../examples/extensions/notify.rs"]
mod notify;

/// The shipped factory, also loaded through the in-process registration interface.
struct Notify;
impl crate::Extension for Notify {
    fn load(api: crate::ExtensionAPI) -> crate::ExtensionFuture<'static, ()> {
        notify::factory(api)
    }
}

/// Delivers agent completion through the callback the built component registered.
async fn agent_end(harness: &mut Harness) -> Result<Answer, String> {
    let key = harness
        .callback("event agent_end")
        .map_err(|e| e.to_string())?;
    let ctx = harness.ordinary("/controlled").map_err(|e| e.to_string())?;
    let exports = harness.exports.clone();
    let event = super::documents::document("agent_end").to_string();
    let called = harness
        .store
        .run_concurrent(async |accessor| {
            exports
                .call_invoke_event(
                    accessor,
                    super::host::borrow(key),
                    event,
                    guest::Capabilities { ctx, signal: None },
                )
                .await
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())??;
    Answer::of(super::host_family::delivery(called))
}

/// Starts one isolated guest with supplied environment and captured, nonterminal stdout.
async fn start(
    compiled: &(wasmtime::Engine, wasmtime::component::Component),
    env: &[(&str, &str)],
) -> Result<(Harness, MemoryOutputPipe), String> {
    let stdout = MemoryOutputPipe::new(4096);
    let mut wasi = WasiCtxBuilder::new();
    wasi.envs(env).stdout(stdout.clone());
    let harness = Box::pin(Harness::from_compiled_with_wasi(
        &compiled.0,
        &compiled.1,
        wasi.build(),
    ))
    .await
    .map_err(|e| e.to_string())?;
    Ok((harness, stdout))
}

/// Every invocation sends exact protocol bytes, including empty-variable fallbacks.
#[test]
fn notify_component_selects_terminal_protocol() -> Result<(), String> {
    let (host, _) = super::block_on(super::example_support::start::<Notify>())?;
    super::example_support::release_all(&host, &["event agent_end"]);
    let path = super::build::example_component("notify")?;
    let compiled = super::build::compile(&path).map_err(|e| e.to_string())?;
    super::block_on(async {
        for (env, expected) in [
            (vec![], "\x1b]777;notify;Maestro;Ready for input\x07"),
            (
                vec![("WT_SESSION", ""), ("KITTY_WINDOW_ID", "")],
                "\x1b]777;notify;Maestro;Ready for input\x07",
            ),
            (
                vec![("KITTY_WINDOW_ID", "42")],
                "\x1b]99;i=1:d=0;Maestro\x1b\\\x1b]99;i=1:p=body;Ready for input\x1b\\",
            ),
            (
                vec![("WT_SESSION", ""), ("KITTY_WINDOW_ID", "0")],
                "\x1b]99;i=1:d=0;Maestro\x1b\\\x1b]99;i=1:p=body;Ready for input\x1b\\",
            ),
        ] {
            let (mut harness, stdout) = start(&compiled, &env).await?;
            assert!(
                stdout.contents().is_empty(),
                "no notification during registration"
            );
            for count in 1..=2 {
                assert_eq!(
                    agent_end(&mut harness).await?,
                    Answer::returned(&super::documents::document("agent_end"), None)
                );
                assert_eq!(
                    stdout.contents().as_ref(),
                    expected.repeat(count).as_bytes(),
                    "{env:?}"
                );
            }
            assert!(harness.observed().native_io.launches.is_empty());
            let key = harness
                .callback("event agent_end")
                .map_err(|e| e.to_string())?;
            harness.release(key).await.map_err(|e| e.to_string())?;
            assert_eq!(harness.observed().live, 0);
        }
        Ok(())
    })
}

/// Windows wins over Kitty and receives a literal program/argv, with output discarded.
#[test]
fn notify_component_windows_command_is_controlled() -> Result<(), String> {
    let path = super::build::example_component("notify")?;
    let compiled = super::build::compile(&path).map_err(|e| e.to_string())?;
    super::block_on(async {
        let (mut harness, stdout) = start(
            &compiled,
            &[("WT_SESSION", "session"), ("KITTY_WINDOW_ID", "42")],
        )
        .await?;
        harness.observed().native_io.stdout = vec![b"controlled Windows stdout".to_vec()];
        harness.observed().native_io.stderr = vec![b"controlled Windows stderr".to_vec()];
        harness.observed().native_io.exit_code = Some(0);
        assert_eq!(
            agent_end(&mut harness).await?,
            Answer::returned(&super::documents::document("agent_end"), None)
        );
        let observed = harness.observed();
        assert_eq!(observed.native_io.launches.len(), 1);
        let (program, args, options) = &observed.native_io.launches[0];
        assert_eq!(program, "powershell.exe");
        assert_eq!(
            args,
            &[
                "-NoProfile",
                "-Command",
                concat!(
                    "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; ",
                    "$xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText01); ",
                    "$xml.GetElementsByTagName('text')[0].AppendChild($xml.CreateTextNode('Ready for input')) > $null; ",
                    "[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('Maestro').Show([Windows.UI.Notifications.ToastNotification]::new($xml))"
                ),
            ]
        );
        assert_eq!(options.stdio, [Stdio::Ignore; 3]);
        assert!(options.cwd.is_none());
        assert!(options.env.is_none());
        assert!(!options.detached);
        assert!(stdout.contents().is_empty());
        harness.observed().native_io.error = Some("PowerShell unavailable".into());
        assert_eq!(
            agent_end(&mut harness).await?.ended,
            super::documents::Ended::Failed("PowerShell unavailable".into())
        );
        assert_eq!(harness.observed().native_io.launches.len(), 1);
        let key = harness
            .callback("event agent_end")
            .map_err(|e| e.to_string())?;
        harness.release(key).await.map_err(|e| e.to_string())?;
        assert_eq!(harness.observed().live, 0);
        Ok(())
    })
}
