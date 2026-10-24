//! Single-owner rule for the lines a protocol server writes.
use proc_macro2::{Delimiter, TokenStream, TokenTree};

use crate::source::{Member, test_scope};

/// Production directories of the protocol server and its standard-stream adapter.
const WRITERS: &[(&str, &str)] = &[
    ("maestro-app", "src/rpc_mode"),
    ("maestro-app", "src/rpc_types"),
    ("maestro-rpc", "src"),
];

/// The file that declares the one record writer.
const WRITER_FILE: (&str, &str) = ("maestro-app", "src/rpc_mode/mod.rs");

/// The writer's body: the shared framing's line, written whole.
const WRITER_BODY: &str =
    "{ let line = serialize_json_line(&serde_json::to_value(record)?)?; output(&line) }";

/// Reject protocol output that bypasses the record writer, the shared framing or the
/// shared response envelope.
pub(crate) fn check(members: &[Member]) -> Result<(), String> {
    let mut found = Vec::new();
    for member in members {
        for source in &member.sources {
            let writer = WRITERS.iter().any(|(owner, directory)| {
                member.name == *owner && source.path.starts_with(member.directory.join(directory))
            });
            if !writer || test_scope(member, source) {
                continue;
            }
            let Ok(tokens) = source.contents.parse::<TokenStream>() else {
                continue;
            };
            let mut scan = Scan {
                owner: member.name == WRITER_FILE.0
                    && source.path == member.directory.join(WRITER_FILE.1),
                found: Vec::new(),
            };
            scan.walk(tokens);
            let path = source.path.display();
            found.extend(
                scan.found
                    .into_iter()
                    .map(|(line, message)| format!("{path}:{line}: {message}")),
            );
        }
    }
    if found.is_empty() {
        Ok(())
    } else {
        Err(found.join("\n"))
    }
}

/// Bypasses found in one writer source.
struct Scan {
    /// Whether the source may declare the record writer.
    owner: bool,
    /// Each bypass's line and message.
    found: Vec<(usize, &'static str)>,
}

impl Scan {
    /// Walks `tokens`, macro arguments included and attributes skipped.
    fn walk(&mut self, tokens: TokenStream) {
        let tokens: Vec<TokenTree> = tokens.into_iter().collect();
        let mut index = 0;
        while let Some(token) = tokens.get(index) {
            index += 1;
            let line = token.span().start().line;
            match token {
                TokenTree::Ident(name) if *name == "fn" && self.writer(&tokens[index..]) => {
                    index += self.writer_body(&tokens[index..], line);
                }
                // The writer's own file imports the shared framing.
                TokenTree::Ident(name) if *name == "use" && self.owner => {
                    index += tokens[index..]
                        .iter()
                        .position(
                            |token| matches!(token, TokenTree::Punct(end) if end.as_char() == ';'),
                        )
                        .map_or(0, |end| end + 1);
                }
                TokenTree::Ident(name) if *name == "serialize_json_line" => {
                    self.found
                        .push((line, "shared framing called outside the record writer"));
                }
                TokenTree::Ident(name) if *name == "output" && called(tokens.get(index)) => {
                    self.found
                        .push((line, "output written outside the record writer"));
                }
                TokenTree::Literal(literal) => self.literal(&literal.to_string(), line),
                TokenTree::Group(_) if super::attribute_group(&tokens[..index - 1]) => {}
                TokenTree::Group(group) => {
                    self.callee(group, tokens.get(index), line);
                    self.walk(group.stream());
                }
                _ => {}
            }
        }
    }

    /// Rejects `group` as a callee of `next` when it ends with the output, as
    /// `(self.output)(line)` does.
    fn callee(&mut self, group: &proc_macro2::Group, next: Option<&TokenTree>, line: usize) {
        let output = group.delimiter() == Delimiter::Parenthesis
            && matches!(group.stream().into_iter().last(), Some(TokenTree::Ident(name)) if name == "output");
        if output && called(next) {
            self.found
                .push((line, "output written outside the record writer"));
        }
    }

    /// Whether `following` names the record writer in its own file.
    fn writer(&self, following: &[TokenTree]) -> bool {
        self.owner
            && matches!(following.first(), Some(TokenTree::Ident(name)) if name == "write_record")
    }

    /// Checks the record writer's body and returns how many tokens it spans.
    fn writer_body(&mut self, following: &[TokenTree], line: usize) -> usize {
        let Some(end) = following.iter().position(
            |token| matches!(token, TokenTree::Group(group) if group.delimiter() == Delimiter::Brace),
        ) else {
            return 0;
        };
        let expected = WRITER_BODY
            .parse::<TokenStream>()
            .map(|body| body.to_string());
        if expected.ok() != Some(following[end].to_string()) {
            self.found
                .push((line, "the record writer must write the shared line whole"));
        }
        end + 1
    }

    /// Rejects a hand-built line ending or response envelope.
    fn literal(&mut self, text: &str, line: usize) {
        let text = text.replace("\\\n", "");
        if text.contains("\\n") || text.contains('\n') {
            self.found.push((
                line,
                "hand-built line ending; frame records with maestro_json::jsonl",
            ));
        }
        if text == "\"response\"" {
            self.found.push((
                line,
                "hand-built response envelope; use serialize_rpc_response",
            ));
        }
    }
}

/// Whether `next` is the argument list of a call.
fn called(next: Option<&TokenTree>) -> bool {
    matches!(next, Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis)
}
