use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::{array, source, string};

pub(crate) fn check(metadata: &Value) -> Result<(), String> {
    let members = array(metadata, "workspace_members")?;
    let mut declarations = BTreeSet::new();
    let guest = array(metadata, "packages")?
        .iter()
        .find(|package| {
            members.contains(&package["id"]) && package["name"] == "maestro-extensions-wasm"
        })
        .map(|package| {
            Path::new(string(package, "manifest_path")?)
                .parent()
                .ok_or("manifest has no parent")?
                .canonicalize()
                .map_err(|error| format!("cannot resolve guest member: {error}"))
        })
        .transpose()?;
    let mut wit = WitInputs::default();
    for package in array(metadata, "packages")? {
        if !members.contains(&package["id"]) {
            continue;
        }
        let owner = string(package, "name")?;
        let manifest = Path::new(string(package, "manifest_path")?);
        for path in source::files(manifest.parent().ok_or("manifest has no parent")?)? {
            let contents = std::fs::read_to_string(&path)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            let tokens = source::tokens(&contents);
            wit.scan(
                owner,
                manifest.parent().ok_or("manifest has no parent")?,
                guest.as_deref(),
                &path,
                &contents,
                &tokens,
            )?;
            for pair in tokens.windows(2) {
                if !matches!(
                    pair[0].text,
                    "struct" | "enum" | "union" | "trait" | "type" | "mod"
                ) {
                    continue;
                }
                let name = pair[1].text;
                if selector(name) && owner != "maestro-chat" {
                    let line = source::line(&contents, pair[0].start);
                    return Err(format!(
                        "{}:{line}: {name} declaration belongs to maestro-chat, not {owner}",
                        path.display()
                    ));
                }
                if matches!(
                    name,
                    "ToolDefinition" | "ToolRenderContext" | "ToolRenderResultOptions"
                ) {
                    let line = source::line(&contents, pair[0].start);
                    if owner != "maestro-tools" {
                        return Err(format!(
                            "{}:{line}: {name} declaration belongs to maestro-tools, not {owner}",
                            path.display()
                        ));
                    }
                    if !declarations.insert(name.to_owned()) {
                        return Err(format!(
                            "{}:{line}: duplicate declaration {name}",
                            path.display()
                        ));
                    }
                }
            }
        }
    }
    wit.finish()
}

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

#[derive(Default)]
struct WitInputs {
    guest: BTreeSet<std::path::PathBuf>,
    host: BTreeSet<std::path::PathBuf>,
    guest_declared: bool,
    host_declared: bool,
    host_location: Option<String>,
}

impl WitInputs {
    fn scan(
        &mut self,
        owner: &str,
        directory: &Path,
        guest: Option<&Path>,
        path: &Path,
        contents: &str,
        tokens: &[source::Token<'_>],
    ) -> Result<(), String> {
        let guest_prefix = ["wit_bindgen", ":", ":", "generate", "!"];
        let host_prefix = ["wasmtime", ":", ":", "component", ":", ":", "bindgen", "!"];
        for index in 0..tokens.len() {
            let prefix = if has_prefix(&tokens[index..], &guest_prefix) {
                &guest_prefix[..]
            } else if has_prefix(&tokens[index..], &host_prefix) {
                &host_prefix[..]
            } else {
                continue;
            };
            let location = format!(
                "{}:{}",
                path.display(),
                source::line(contents, tokens[index].start)
            );
            let error = || {
                format!(
                    "{location}: WIT code-generation input requires review: expected static path"
                )
            };
            let tail = &tokens[index + prefix.len()..];
            if tail
                .first()
                .is_none_or(|token| !matches!(token.text, "(" | "{" | "["))
            {
                return Err(error());
            }
            let mut stack = Vec::new();
            let mut end = None;
            for (offset, token) in tail.iter().enumerate() {
                match token.text {
                    "(" => stack.push(")"),
                    "{" => stack.push("}"),
                    "[" => stack.push("]"),
                    ")" | "}" | "]" => {
                        if stack.pop() != Some(token.text) {
                            return Err(error());
                        }
                        if stack.is_empty() {
                            end = Some(offset);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let body = &tail[1..end.ok_or_else(error)?];
            let mut paths = Vec::new();
            let mut inline = false;
            if body
                .first()
                .and_then(|token| string_literal(token.text))
                .is_some()
            {
                if body.len() != 1 {
                    return Err(error());
                }
                // A shorthand string names a world in the manifest-relative default directory.
                paths.push("wit".to_owned());
            } else {
                for (offset, pair) in body.windows(2).enumerate() {
                    if pair[1].text != ":" {
                        continue;
                    }
                    if pair[0].text == "inline" {
                        inline = true;
                        if owner != "maestro-extensions-wasm" {
                            return Err(format!("{location}: inline WIT must be guest-owned"));
                        }
                    } else if pair[0].text == "path" {
                        let value = &body[offset + 2..];
                        if let Some(path) =
                            value.first().and_then(|token| string_literal(token.text))
                        {
                            if value
                                .get(1)
                                .is_some_and(|token| !matches!(token.text, "," | "}"))
                            {
                                return Err(error());
                            }
                            paths.push(path);
                        } else if value.first().is_some_and(|token| token.text == "[") {
                            let mut cursor = 1;
                            loop {
                                if value.get(cursor).is_some_and(|token| token.text == "]") {
                                    break;
                                }
                                paths.push(
                                    value
                                        .get(cursor)
                                        .and_then(|token| string_literal(token.text))
                                        .ok_or_else(error)?,
                                );
                                cursor += 1;
                                match value.get(cursor).map(|token| token.text) {
                                    Some(",") => cursor += 1,
                                    Some("]") => break,
                                    _ => return Err(error()),
                                }
                            }
                            if value
                                .get(cursor + 1)
                                .is_some_and(|token| !matches!(token.text, "," | "}"))
                            {
                                return Err(error());
                            }
                        } else {
                            return Err(error());
                        }
                    }
                }
            }
            if paths.is_empty() && !inline {
                return Err(error());
            }
            let is_guest = owner == "maestro-extensions-wasm";
            if is_guest {
                self.guest_declared = true;
            } else {
                self.host_declared = true;
                self.host_location = Some(location.clone());
            }
            for input in paths {
                let input = directory
                    .join(input)
                    .canonicalize()
                    .map_err(|error| format!("{location}: cannot resolve WIT input: {error}"))?;
                let guest = guest.ok_or_else(|| {
                    format!("{location}: WIT input has no guest-owned source member")
                })?;
                if !input.starts_with(guest) {
                    return Err(format!(
                        "{location}: WIT input must use the guest-owned canonical source: {}",
                        input.display()
                    ));
                }
                let root = if input.is_dir() {
                    input
                } else {
                    input.parent().ok_or_else(error)?.to_path_buf()
                };
                if is_guest {
                    self.guest.insert(root);
                } else {
                    self.host.insert(root);
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<(), String> {
        if self.guest_declared && self.host_declared && self.guest != self.host {
            return Err(format!(
                "{}: host and guest WIT inputs must share the same canonical source roots",
                self.host_location.as_deref().unwrap_or("WIT")
            ));
        }
        Ok(())
    }
}

fn has_prefix(tokens: &[source::Token<'_>], prefix: &[&str]) -> bool {
    tokens.len() >= prefix.len()
        && tokens
            .iter()
            .zip(prefix)
            .all(|(token, text)| token.text == *text)
}

fn string_literal(text: &str) -> Option<String> {
    if let Some(body) = text.strip_prefix('"') {
        return cooked_string(body.strip_suffix('"')?);
    }
    let rest = text.strip_prefix('r')?;
    let quote = rest.find('"')?;
    let hashes = &rest[..quote];
    if !hashes.bytes().all(|byte| byte == b'#') {
        return None;
    }
    let body = rest[quote + 1..].strip_suffix(hashes)?.strip_suffix('"')?;
    Some(body.to_owned())
}

fn cooked_string(body: &str) -> Option<String> {
    let mut characters = body.chars().peekable();
    let mut result = String::new();
    while let Some(character) = characters.next() {
        if character != '\\' {
            result.push(character);
            continue;
        }
        let escaped = match characters.next()? {
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '\\' => '\\',
            '"' => '"',
            '\'' => '\'',
            '0' => '\0',
            'x' => {
                let high = characters.next()?.to_digit(16)?;
                let low = characters.next()?.to_digit(16)?;
                let value = high * 16 + low;
                if value > 0x7f {
                    return None;
                }
                char::from_u32(value)?
            }
            'u' => {
                if characters.next()? != '{' {
                    return None;
                }
                let mut value = characters.next()?.to_digit(16)?;
                let mut digits = 1;
                loop {
                    match characters.next()? {
                        '}' => break,
                        '_' => {}
                        character => {
                            digits += 1;
                            if digits > 6 {
                                return None;
                            }
                            value = value * 16 + character.to_digit(16)?;
                        }
                    }
                }
                char::from_u32(value)?
            }
            newline @ ('\n' | '\r') => {
                if newline == '\r' && characters.next()? != '\n' {
                    return None;
                }
                while characters
                    .peek()
                    .is_some_and(|character| matches!(character, ' ' | '\t' | '\n' | '\r'))
                {
                    characters.next();
                }
                continue;
            }
            _ => return None,
        };
        result.push(escaped);
    }
    Some(result)
}
