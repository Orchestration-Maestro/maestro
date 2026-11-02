//! Reading standard input and preparing the first prompt.
use crate::input::{
    args::Args,
    file_processor::{FileArgumentOperations, ProcessFileOptions, process_file_arguments},
    initial_message::{InitialMessageInput, InitialMessageResult, build_initial_message},
};
use maestro_app::{is_whitespace_scalar, lossy_string};
use std::io::{self, Write};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt};

/// Read non-terminal input through EOF, keeping non-whitespace text.
///
/// # Errors
/// Returns the input failure unchanged.
pub async fn read_piped_stdin(
    stdin: &mut (dyn AsyncRead + Unpin),
    stdin_is_tty: bool,
) -> io::Result<Option<String>> {
    if stdin_is_tty {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    stdin.read_to_end(&mut bytes).await?;
    let text = lossy_string(bytes);
    let text = text.trim_matches(is_whitespace_scalar);
    Ok((!text.is_empty()).then(|| text.to_owned()))
}

/// Prepare authored files and combine them with stdin and the first prompt.
///
/// # Errors
/// Returns the file operation's failure unchanged.
pub fn prepare_initial_message(
    parsed: &mut Args,
    auto_resize_images: bool,
    stdin_content: Option<&str>,
    operations: &dyn FileArgumentOperations,
) -> io::Result<InitialMessageResult> {
    let files = if parsed.file_args.is_empty() {
        None
    } else {
        Some(process_file_arguments(
            &parsed.file_args,
            ProcessFileOptions {
                auto_resize_images: Some(auto_resize_images),
            },
            operations,
        )?)
    };
    let (text, images) = files.map_or((None, Vec::new()), |files| (Some(files.text), files.images));
    Ok(build_initial_message(InitialMessageInput {
        parsed,
        file_text: text.as_deref(),
        file_images: images,
        stdin_content,
    }))
}

/// Ask for an untrimmed yes/no answer; EOF without a line supplies no decision.
///
/// # Errors
/// Returns the input or prompt-output failure unchanged.
pub async fn prompt_confirm(
    message: &str,
    stdin: &mut (dyn AsyncBufRead + Unpin),
    stdout: &mut dyn Write,
) -> io::Result<Option<bool>> {
    write!(stdout, "{message} [y/N] ")?;
    stdout.flush()?;
    let mut answer = Vec::new();
    loop {
        let available = stdin.fill_buf().await?;
        if available.is_empty() {
            return Ok(None);
        }
        if let Some(end) = available
            .iter()
            .position(|byte| matches!(byte, b'\r' | b'\n'))
        {
            answer.extend_from_slice(&available[..end]);
            stdin.consume(end + 1);
            let answer = lossy_string(answer);
            return Ok(Some(
                answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"),
            ));
        }
        answer.extend_from_slice(available);
        let consumed = available.len();
        stdin.consume(consumed);
    }
}
