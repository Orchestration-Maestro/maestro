use std::collections::BTreeSet;
use std::path::Path;

use crate::source::{Member, Source, test_layout};
use proc_macro2::{TokenStream, TokenTree};
use syn::ext::IdentExt;

/// Static interface inputs for extension code generation.
mod wit;
use wit::{ScanContext, WitInputs};

/// Single-owner rule for retained message renderer calls.
mod message_renderers;

/// Verify declaration ownership and canonical extension interface inputs.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let guest = members
        .iter()
        .find(|member| member.name == "maestro-extensions-wasm")
        .map(|member| {
            member
                .directory
                .canonicalize()
                .map_err(|error| format!("cannot resolve guest member: {error}"))
        })
        .transpose()?;
    let mut declarations = BTreeSet::new();
    let mut wit = WitInputs::default();
    for member in members {
        for source in &member.sources {
            let mut records = Records::default();
            if let Ok(tokens) = source.contents.parse() {
                records.scan(tokens);
            }
            check_declarations(member, source, &records, &mut declarations)?;
            check_session_setup(member, source, &records)?;
            let context = ScanContext {
                owner: &member.name,
                directory: &member.directory,
                guest: guest.as_deref(),
                path: &source.path,
            };
            for (line, tokens) in records.inputs {
                wit.scan(&context, line, tokens)?;
            }
        }
    }
    message_renderers::check(members)?;
    wit.finish()
}

/// Rust declaration keywords whose following identifiers are inspected.
const DECLARATION_KINDS: &[&str] = &["struct", "enum", "union", "trait", "type", "mod"];

/// Session configuration record that only the shared interactive test setup constructs.
const SESSION_CONFIGURATION: &str = "AgentSessionConfig";

/// The one owner of the interactive-mode test session setup.
const SESSION_SETUP: &str = "tests/footer_support/session.rs";

/// Footer support tree shared by the footer and interactive startup tests.
const FOOTER_SUPPORT: &str = "tests/footer_support";

/// Source tree holding the interactive mode and its own test modules.
const INTERACTIVE_MODE: &str = "src/interactive_mode";

#[derive(Default)]
/// Declarations and interface-generation macros found in one source.
struct Records {
    /// First line naming the shared interactive session configuration.
    session_configuration_line: Option<usize>,
    /// Declaration names paired with their source lines.
    declarations: Vec<(String, String, usize)>,
    /// Interface-generation macro arguments paired with their source lines.
    inputs: Vec<(usize, proc_macro2::TokenStream)>,
}

impl Records {
    /// Walk tokens while ignoring attributes and generated metavariable names.
    fn scan(&mut self, tokens: TokenStream) {
        let tokens: Vec<_> = tokens.into_iter().collect();
        for (index, token) in tokens.iter().enumerate() {
            let generated = matches!(tokens[..index].last(), Some(TokenTree::Punct(punctuation)) if punctuation.as_char() == '$');
            match token {
                TokenTree::Ident(keyword) if !generated => {
                    self.session_configuration(keyword);
                    self.declaration(keyword, &tokens[index + 1..]);
                    self.wit_macro(&tokens[index..]);
                }
                TokenTree::Group(group) if !attribute_group(&tokens[..index]) => {
                    self.scan(group.stream());
                }
                _ => {}
            }
        }
    }

    /// Record the first line that names the shared interactive session configuration.
    fn session_configuration(&mut self, keyword: &proc_macro2::Ident) {
        if keyword.unraw() == SESSION_CONFIGURATION {
            self.session_configuration_line
                .get_or_insert(keyword.span().start().line);
        }
    }

    /// Record a recognized declaration with its unescaped name.
    fn declaration(&mut self, keyword: &proc_macro2::Ident, following: &[TokenTree]) {
        let kind = keyword.to_string();
        if !DECLARATION_KINDS.contains(&kind.as_str()) {
            return;
        }
        let Some(TokenTree::Ident(name)) = following.first() else {
            return;
        };
        if kind == "union" && !union_body(following.get(1)) {
            return;
        }
        self.declarations
            .push((name.unraw().to_string(), kind, keyword.span().start().line));
    }

    /// Record supported interface-generation macro inputs.
    fn wit_macro(&mut self, tokens: &[TokenTree]) {
        if !matches!(tokens.first(), Some(TokenTree::Ident(name)) if name == "wit_bindgen" || name == "wasmtime")
        {
            return;
        }
        let Some(end) = tokens
            .iter()
            .position(|token| matches!(token, TokenTree::Group(_)))
        else {
            return;
        };
        let Ok(mac) = syn::parse2::<syn::Macro>(tokens[..=end].iter().cloned().collect()) else {
            return;
        };
        let path: Vec<_> = mac
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.unraw().to_string())
            .collect();
        if path == ["wit_bindgen", "generate"] || path == ["wasmtime", "component", "bindgen"] {
            self.inputs
                .push((tokens[0].span().start().line, mac.tokens));
        }
    }
}

/// Recognize token groups belonging to an attribute.
fn attribute_group(previous: &[TokenTree]) -> bool {
    match previous {
        [.., TokenTree::Punct(hash)] if hash.as_char() == '#' => true,
        [.., TokenTree::Punct(hash), TokenTree::Punct(bang)] => {
            hash.as_char() == '#' && bang.as_char() == '!'
        }
        _ => false,
    }
}

/// Distinguish a union declaration from an identifier used as a value.
fn union_body(token: Option<&TokenTree>) -> bool {
    match token {
        Some(TokenTree::Group(group)) => group.delimiter() == proc_macro2::Delimiter::Brace,
        Some(TokenTree::Punct(punctuation)) => punctuation.as_char() == '<',
        Some(TokenTree::Ident(keyword)) => keyword == "where",
        _ => false,
    }
}

/// Interactive-mode chat test sources that build their sessions through the shared setup.
///
/// Paths compare by component, so native separators never change the answer.
fn migrated(relative: &Path) -> bool {
    if relative.starts_with(FOOTER_SUPPORT) {
        return relative != Path::new(SESSION_SETUP);
    }
    relative.starts_with(INTERACTIVE_MODE) && test_layout(relative, Path::new(""))
}

/// Reject a second interactive-mode session setup outside its extracted owner.
fn check_session_setup(member: &Member, source: &Source, records: &Records) -> Result<(), String> {
    let Some(line) = records.session_configuration_line else {
        return Ok(());
    };
    let in_scope = member.name == "maestro-chat"
        && source
            .path
            .strip_prefix(&member.directory)
            .is_ok_and(migrated);
    if !in_scope {
        return Ok(());
    }
    Err(format!(
        "{}:{line}: session setup belongs in {SESSION_SETUP}",
        source.path.display()
    ))
}

/// Recognize selector declarations owned by the chat frontend.
fn selector(name: &str) -> bool {
    matches!(
        name,
        "ConfigSelectorComponent"
            | "ExtensionSelectorComponent"
            | "ModelSelectorComponent"
            | "OAuthSelectorComponent"
            | "ScopedModelsSelectorComponent"
            | "SessionSelectorComponent"
            | "SettingsSelectorComponent"
            | "ShowImagesSelectorComponent"
            | "ThemeSelectorComponent"
            | "ThinkingSelectorComponent"
            | "TreeSelectorComponent"
            | "UserMessageSelectorComponent"
            | "config_selector"
            | "extension_selector"
            | "model_selector"
            | "oauth_selector"
            | "scoped_models_selector"
            | "session_selector"
            | "settings_selector"
            | "show_images_selector"
            | "theme_selector"
            | "thinking_selector"
            | "tree_selector"
            | "user_message_selector"
            | "session_selector_search"
    )
}

/// Recognize the command-line wrapper module that carries no widget declarations.
fn cli_configuration_wrapper(member: &Member, source: &Source, name: &str, kind: &str) -> bool {
    name == "config_selector"
        && kind == "mod"
        && member.name == "maestro-cli"
        && source.path == member.directory.join("src/input/mod.rs")
}

/// Reject declarations outside their owning crate or duplicate tool contracts.
fn check_declarations(
    member: &Member,
    source: &Source,
    records: &Records,
    declarations: &mut BTreeSet<String>,
) -> Result<(), String> {
    for (name, kind, line) in &records.declarations {
        let owner = if selector(name) {
            "maestro-chat"
        } else if matches!(
            name.as_str(),
            "ToolDefinition" | "ToolRenderContext" | "ToolRenderResultOptions"
        ) {
            "maestro-tools"
        } else if matches!(
            name.as_str(),
            "ToolInfo"
                | "SlashCommandInfo"
                | "SlashCommandSource"
                | "MessageRenderOptions"
                | "MessageDelivery"
                | "UserMessageDelivery"
                | "SendMessageOptions"
                | "SendUserMessageOptions"
        ) || (shared_event_record(name) && kind != "type")
        {
            "maestro-request"
        } else if matches!(name.as_str(), "ForkPosition" | "NavigateTreeOptions") {
            "maestro-entry"
        } else {
            continue;
        };
        let guest_tool = name == "ToolDefinition"
            && member.name == "maestro-extensions-wasm"
            && source.path == member.directory.join("src/types/tools.rs");
        let guest_metadata = name == "ToolInfo"
            && kind == "type"
            && member.name == "maestro-extensions-wasm"
            && source.path == member.directory.join("src/types/actions.rs");
        let extension_tool = name == "ToolDefinition"
            && kind == "type"
            && member.name == "maestro-extensions"
            && source.path == member.directory.join("src/types/registration.rs");
        let wrapper = cli_configuration_wrapper(member, source, name, kind);
        if member.name != owner && !guest_tool && !guest_metadata && !extension_tool && !wrapper {
            return Err(format!(
                "{}:{line}: {name} declaration belongs to {owner}, not {}",
                source.path.display(),
                member.name
            ));
        }
        if matches!(owner, "maestro-tools" | "maestro-request" | "maestro-entry")
            && !declarations.insert(format!("{}::{name}", member.name))
        {
            return Err(format!(
                "{}:{line}: duplicate declaration {name}",
                source.path.display()
            ));
        }
    }
    Ok(())
}

/// Shared schema declarations whose specialization aliases remain owner-local.
fn shared_event_record(name: &str) -> bool {
    matches!(
        name,
        "ContextFile"
            | "BuildSystemPromptOptions"
            | "ModelSelectSource"
            | "ModelSelectEvent"
            | "ThinkingLevelSelectEvent"
            | "SessionCompactEvent"
            | "TreePreparation"
            | "SessionTreeEvent"
            | "SessionBeforeTreeSummary"
            | "SessionBeforeTreeResult"
            | "SessionBeforeTreeEvent"
            | "ContextEvent"
            | "BeforeAgentStartEvent"
            | "AgentStartEvent"
            | "AgentEndEvent"
            | "TurnStartEvent"
            | "TurnEndEvent"
            | "MessageStartEvent"
            | "MessageUpdateEvent"
            | "MessageEndEvent"
            | "ResourcesDiscoverReason"
            | "ResourcesDiscoverEvent"
            | "ResourcesDiscoverResult"
            | "SessionStartReason"
            | "SessionStartEvent"
            | "SessionBeforeSwitchReason"
            | "SessionBeforeSwitchEvent"
            | "SessionBeforeForkEvent"
            | "SessionShutdownReason"
            | "SessionShutdownEvent"
            | "SessionBeforeSwitchResult"
            | "SessionBeforeForkResult"
            | "BeforeProviderRequestEvent"
            | "AfterProviderResponseEvent"
            | "InputSource"
            | "InputEvent"
            | "InputTransform"
            | "InputEventResult"
            | "SessionBeforeCompactEventData"
            | "SessionBeforeCompactEvent"
            | "SessionBeforeCompactResult"
            | "ToolExecutionStartEvent"
            | "ToolExecutionUpdateEvent"
            | "ToolExecutionEndEvent"
            | "ToolCallEvent"
            | "ToolResultEvent"
            | "SessionEvent"
            | "ExtensionEvent"
            | "ExtensionEventResult"
            | "UserBashEvent"
            | "UserBashEventResult"
            | "ToolCallEventResult"
            | "ToolResultEventResult"
            | "ContextEventResult"
            | "MessageEndEventResult"
            | "BeforeAgentStartEventResult"
    )
}
