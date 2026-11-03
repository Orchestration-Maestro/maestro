//! Sends an opt-in desktop notification when the agent is ready for input.
// maestro-component: MAESTRO_NOTIFY_COMPONENT
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use maestro_extensions_wasm::{
    ExtensionAPI, ExtensionEventResult, ExtensionFuture, ExtensionResult,
    child_process::{SpawnOptions, Stdio},
    io::NativeIo,
};
use std::{io::Write, rc::Rc};

/// Quotes literal text for a single-quoted PowerShell string; XML text is created by the DOM.
fn windows_toast_script(title: &str, body: &str) -> String {
    let title = title.replace('\'', "''");
    let body = body.replace('\'', "''");
    let kind = "Windows.UI.Notifications";
    let manager =
        format!("[{kind}.ToastNotificationManager, {kind}, ContentType = WindowsRuntime]");
    [
        format!("{manager} > $null"),
        format!("$xml = [{kind}.ToastNotificationManager]::GetTemplateContent([{kind}.ToastTemplateType]::ToastText01)"),
        format!("$xml.GetElementsByTagName('text')[0].AppendChild($xml.CreateTextNode('{body}')) > $null"),
        format!("[{kind}.ToastNotificationManager]::CreateToastNotifier('{title}').Show([{kind}.ToastNotification]::new($xml))"),
    ]
    .join("; ")
}

/// Removes terminal control characters and OSC field delimiters from notification text.
fn osc_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() && *c != ';')
        .collect()
}

/// Encodes the terminal's notification without allowing text to inject another OSC command.
fn terminal_notification(title: &str, body: &str, kitty: bool) -> String {
    let (title, body) = (osc_text(title), osc_text(body));
    if kitty {
        format!("\x1b]99;i=1:d=0;{title}\x1b\\\x1b]99;i=1:p=body;{body}\x1b\\")
    } else {
        format!("\x1b]777;notify;{title};{body}\x07")
    }
}

/// Sends completion through the selected native notification protocol.
async fn notify(io: Rc<dyn NativeIo>) -> ExtensionResult<Option<ExtensionEventResult>> {
    if std::env::var_os("WT_SESSION").is_some_and(|v| !v.is_empty()) {
        let child = io
            .spawn(
                "powershell.exe".into(),
                vec![
                    "-NoProfile".into(),
                    "-Command".into(),
                    windows_toast_script("Maestro", "Ready for input"),
                ],
                SpawnOptions {
                    stdio: [Stdio::Ignore; 3],
                    ..SpawnOptions::default()
                },
            )
            .await?;
        child.wait().await?;
    } else {
        let kitty = std::env::var_os("KITTY_WINDOW_ID").is_some_and(|v| !v.is_empty());
        let output = terminal_notification("Maestro", "Ready for input", kitty);
        let mut stdout = std::io::stdout().lock();
        stdout
            .write_all(output.as_bytes())
            .map_err(|e| e.to_string())?;
        stdout.flush().map_err(|e| e.to_string())?;
    }
    Ok(None)
}

/// Registers the completion hook; Windows Terminal takes precedence over Kitty.
#[must_use]
pub fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        let io = api.native_io();
        api.on(
            "agent_end",
            Rc::new(move |_, _| Box::pin(notify(io.clone()))),
        )
    })
}

#[cfg(target_arch = "wasm32")]
maestro_extensions_wasm::export_extension!(factory);

#[cfg(test)]
mod tests {
    use super::{terminal_notification, windows_toast_script};

    /// Notification text cannot close a terminal sequence or add a field.
    #[test]
    fn notify_escapes_terminal_title_and_body() {
        for (kitty, expected) in [
            (false, "\x1b]777;notify;Title雪;Body'\"&<>\x07"),
            (
                true,
                "\x1b]99;i=1:d=0;Title雪\x1b\\\x1b]99;i=1:p=body;Body'\"&<>\x1b\\",
            ),
        ] {
            assert_eq!(
                terminal_notification("Ti;tle\x1b\x07雪", "Bo\r\n\t\u{9c}dy'\"&<>", kitty),
                expected
            );
        }
    }

    /// PowerShell receives literal apostrophes while its DOM escapes XML markup.
    #[test]
    fn notify_quotes_windows_title_and_body() {
        assert_eq!(
            windows_toast_script("O'Brien", "It's <ready> & \"safe\"; $(x)\n雪"),
            concat!(
                "[Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] > $null; ",
                "$xml = [Windows.UI.Notifications.ToastNotificationManager]::GetTemplateContent([Windows.UI.Notifications.ToastTemplateType]::ToastText01); ",
                "$xml.GetElementsByTagName('text')[0].AppendChild($xml.CreateTextNode('It''s <ready> & \"safe\"; $(x)\n雪')) > $null; ",
                "[Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier('O''Brien').Show([Windows.UI.Notifications.ToastNotification]::new($xml))"
            )
        );
    }
}
