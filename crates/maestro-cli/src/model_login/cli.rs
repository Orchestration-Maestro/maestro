//! Help, provider tables and OAuth login for the standalone model-account command.
use std::{error::Error, sync::Arc};

use maestro_app::{
    BoxFuture, OAuthCallbacks, OAuthCredentials, OAuthError,
    model_account::cli::{load_auth, save_auth},
    presentation_data::{OAuthAuthInfo, OAuthLoginCallbacks, OAuthPrompt, OAuthProviderHandle},
};
use serde::Serialize;

/// The account file the command reads and writes in the caller's directory.
const AUTH_FILE: &str = "auth.json";

/// Width of a provider identifier column, in UTF-16 code units.
const ID_COLUMN: usize = 20;

/// A failure that ends the command with one diagnostic line.
type Failure = Box<dyn Error>;

/// One host input interface: its question and its release.
pub trait AccountInput: Send + Sync {
    /// Ask the complete displayed question and return the supplied answer unchanged.
    fn prompt(&self, question: String) -> BoxFuture<Result<String, OAuthError>>;

    /// Release this interface; a released interface is never asked again.
    fn close(&self);
}

/// The host text streams and input interfaces the command presents through.
pub trait AccountConsole: Send + Sync {
    /// Write complete supplied text without adding a newline, to the error stream when
    /// `is_error` is set.
    fn write(&self, text: &str, is_error: bool);

    /// Create a fresh input interface; the help and list commands create none.
    fn create_interface(&self) -> Arc<dyn AccountInput>;
}

/// Run the account command over an ordered provider snapshot, a live resolver and a console.
///
/// Consumes the arguments after the command name and returns its exit status without ending
/// the caller. The snapshot decides which providers are offered; the resolver decides which
/// implementation serves a login, once the snapshot has accepted the identifier.
pub async fn main(
    args: &[String],
    providers: Vec<OAuthProviderHandle>,
    get_provider: &dyn Fn(&str) -> Option<OAuthProviderHandle>,
    console: Arc<dyn AccountConsole>,
) -> i32 {
    match run(args, &providers, get_provider, Arc::clone(&console)).await {
        Ok(status) => status,
        Err(failure) => {
            console.write(&format!("Error: {failure}\n"), true);
            1
        }
    }
}

/// Dispatch one command, returning its exit status.
async fn run(
    args: &[String],
    providers: &[OAuthProviderHandle],
    get_provider: &dyn Fn(&str) -> Option<OAuthProviderHandle>,
    console: Arc<dyn AccountConsole>,
) -> Result<i32, Failure> {
    let command = args.first().map_or("", String::as_str);
    match command {
        "" | "help" | "--help" | "-h" => {
            console.write(&help(providers), false);
            Ok(0)
        }
        "list" => {
            console.write("Available OAuth providers:\n\n", false);
            for provider in providers {
                console.write(&format!("{}\n", provider_row(provider)), false);
            }
            Ok(0)
        }
        "login" => {
            let requested = args.get(1).map_or("", String::as_str);
            login(requested, providers, get_provider, console).await
        }
        other => {
            console.write(
                &format!(
                    "Unknown command: {other}\nUse 'maestro-model-account --help' for usage\n"
                ),
                true,
            );
            Ok(1)
        }
    }
}

/// Select the requested provider and authorize it.
async fn login(
    requested: &str,
    providers: &[OAuthProviderHandle],
    get_provider: &dyn Fn(&str) -> Option<OAuthProviderHandle>,
    console: Arc<dyn AccountConsole>,
) -> Result<i32, Failure> {
    let provider_id = if requested.is_empty() {
        let Some(selected) = choose_provider(providers, console.as_ref()).await? else {
            return Ok(1);
        };
        selected
    } else if providers.iter().any(|provider| provider.id() == requested) {
        requested
    } else {
        console.write(
            &format!(
                "Unknown provider: {requested}\nUse 'maestro-model-account list' to see available providers\n"
            ),
            true,
        );
        return Ok(1);
    };
    console.write(&format!("Logging in to {provider_id}...\n"), false);
    authorize(provider_id, get_provider, console).await
}

/// Ask for one snapshot position and return the selected identifier.
async fn choose_provider<'a>(
    providers: &'a [OAuthProviderHandle],
    console: &dyn AccountConsole,
) -> Result<Option<&'a str>, Failure> {
    let input = console.create_interface();
    let guard = InputGuard(Arc::clone(&input));
    console.write("Select a provider:\n\n", false);
    for (position, provider) in providers.iter().enumerate() {
        console.write(&format!("  {}. {}\n", position + 1, provider.name()), false);
    }
    console.write("\n", false);
    let answer = input
        .prompt(format!("Enter number (1-{}): ", providers.len()))
        .await?;
    drop(guard);
    let Some(provider) = selected_provider(&answer, providers) else {
        console.write("Invalid selection\n", true);
        return Ok(None);
    };
    Ok(Some(provider.id()))
}

/// The snapshot provider that one complete numbered answer selects.
fn selected_provider<'a>(
    answer: &str,
    providers: &'a [OAuthProviderHandle],
) -> Option<&'a OAuthProviderHandle> {
    let trimmed = answer.trim_matches(maestro_app::is_whitespace_scalar);
    let digits = trimmed.strip_prefix('+').unwrap_or(trimmed);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let position: usize = digits.parse().ok()?;
    providers.get(position.checked_sub(1)?)
}

/// Authorize the selected provider through the live resolver and store its credentials.
async fn authorize(
    provider_id: &str,
    get_provider: &dyn Fn(&str) -> Option<OAuthProviderHandle>,
    console: Arc<dyn AccountConsole>,
) -> Result<i32, Failure> {
    let Some(provider) = get_provider(provider_id) else {
        console.write(&format!("Unknown provider: {provider_id}\n"), true);
        return Ok(1);
    };
    let input = console.create_interface();
    let guard = InputGuard(Arc::clone(&input));
    let callbacks: OAuthCallbacks = Arc::new(Callbacks {
        console: Arc::clone(&console),
        input,
    });
    let credentials = provider.login(callbacks, None).await?;
    store(provider_id, credentials)?;
    console.write(&format!("\nCredentials saved to {AUTH_FILE}\n"), false);
    drop(guard);
    Ok(0)
}

/// Add one login to the account file under its provider key.
fn store(provider_id: &str, credentials: OAuthCredentials) -> Result<(), Failure> {
    let entry = serde_json::to_value(StoredCredentials {
        kind: "oauth",
        credentials,
    })?;
    let mut auth = load_auth(AUTH_FILE);
    auth.insert(provider_id.to_owned(), entry);
    save_auth(AUTH_FILE, &auth)?;
    Ok(())
}

/// One account entry: the account tag first, then every supplied credential member.
#[derive(Serialize)]
struct StoredCredentials {
    /// Account tag, in the slot a supplied `type` member takes over.
    #[serde(rename = "type")]
    kind: &'static str,
    /// Supplied token data, including its provider-specific members.
    #[serde(flatten)]
    credentials: OAuthCredentials,
}

/// Releases an input interface whenever the login it belongs to finishes.
struct InputGuard(Arc<dyn AccountInput>);

impl Drop for InputGuard {
    fn drop(&mut self) {
        self.0.close();
    }
}

/// Presents one authorization flow through the console and its input interface.
struct Callbacks {
    /// Host text streams.
    console: Arc<dyn AccountConsole>,
    /// Interface questions are asked through.
    input: Arc<dyn AccountInput>,
}

impl OAuthLoginCallbacks for Callbacks {
    fn callback_presence(&self) -> (bool, bool, bool) {
        (true, false, false)
    }
    fn on_auth(&self, info: OAuthAuthInfo) -> Result<(), OAuthError> {
        self.console.write(
            &format!("\nOpen this URL in your browser:\n{}\n", info.url),
            false,
        );
        if let Some(instructions) = info.instructions.filter(|text| !text.is_empty()) {
            self.console.write(&format!("{instructions}\n"), false);
        }
        self.console.write("\n", false);
        Ok(())
    }

    fn on_prompt(&self, prompt: OAuthPrompt) -> BoxFuture<Result<String, OAuthError>> {
        let placeholder = prompt
            .placeholder
            .filter(|text| !text.is_empty())
            .map_or_else(String::new, |text| format!(" ({text})"));
        self.input
            .prompt(format!("{}{placeholder}: ", prompt.message))
    }

    fn on_progress(&self, message: &str) -> Result<(), OAuthError> {
        self.console.write(&format!("{message}\n"), false);
        Ok(())
    }
}

/// One provider table row: the identifier in its column, then the display name.
fn provider_row(provider: &OAuthProviderHandle) -> String {
    let id = provider.id();
    let column = ID_COLUMN.saturating_sub(id.encode_utf16().count());
    format!("  {id}{} {}", " ".repeat(column), provider.name())
}

/// The complete usage document for the supplied snapshot.
fn help(providers: &[OAuthProviderHandle]) -> String {
    let rows: Vec<String> = providers.iter().map(provider_row).collect();
    format!(
        concat!(
            "Usage: maestro-model-account <command> [provider]\n",
            "\n",
            "Commands:\n",
            "  login [provider]  Login to an OAuth provider\n",
            "  list              List available providers\n",
            "\n",
            "Providers:\n",
            "{}\n",
            "\n",
            "Examples:\n",
            "  maestro-model-account login              # interactive provider selection\n",
            "  maestro-model-account login anthropic    # login to specific provider\n",
            "  maestro-model-account list               # list providers\n",
            "\n",
        ),
        rows.join("\n")
    )
}
