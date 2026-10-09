# Terminal Markdown

`Markdown` renders headings, paragraphs, code blocks, rules, quotes and lists
through supplied `MarkdownTheme` callbacks. Inline formatting includes emphasis,
strong text, strikethrough, code and links. The native `CommonMark` parser owns the
grammar; bare HTTP/HTTPS, www and email links use the extended-link rules. A www
link requires a period in the domain after its prefix. HTML is literal text,
including inside lists; quoted HTML suppresses the message foreground, like other
quote text. Images display authored label markup without enclosing
quote/list prefixes, unescaping brackets but retaining other escapes.
Table layout is not provided.

`MarkdownOptions` supplies horizontal and vertical padding and an optional
`DefaultTextStyle`. Its `decorations` select `TextDecoration` values; they apply in
bold, italic, strikethrough, underline order, ignoring duplicates. Foreground and
decorations style content; background styles the final rows including margins.
Empty or whitespace-only input renders nothing, even with padding. Requested
margins are retained at narrow widths; output is not necessarily width-bounded.
See [styled text](text.md) for wrapping and [inline images](images.md) for protocol
rows and hyperlink transport.

The optional highlighter receives complete code text and the optional full fence
information string. Indented code has no information string; an empty fence has
an empty string. Returned rows receive the configured code indentation (two
spaces when absent, none when explicitly empty) and ordinary component layout.
Without a highlighter, `code_block` styles each code line. Callback panics propagate;
the component neither catches them nor invents fallback highlighting.

Rows are cached by width. `set_text`, including equal text, and `invalidate` drop
that cache. A new layout reads live callback state and terminal capabilities;
changing either alone does not invalidate cached rows. A callback that invalidates
or replaces text prevents publication of the in-progress layout. Weak handles avoid
cycles when callbacks capture their own component.

```rust
use std::rc::Rc;
use maestro_tui::components::markdown::{
    DefaultTextStyle, Markdown, MarkdownOptions, MarkdownTheme, TextDecoration,
};
use maestro_tui::{Component, TerminalImage};

let theme = Rc::new(MarkdownTheme {
    heading: Box::new(str::to_owned),
    link: Box::new(str::to_owned),
    link_url: Box::new(str::to_owned),
    code: Box::new(str::to_owned),
    code_block: Box::new(str::to_owned),
    code_block_border: Box::new(str::to_owned),
    quote: Box::new(str::to_owned),
    quote_border: Box::new(str::to_owned),
    hr: Box::new(str::to_owned),
    list_bullet: Box::new(str::to_owned),
    bold: Box::new(str::to_owned),
    italic: Box::new(str::to_owned),
    strikethrough: Box::new(str::to_owned),
    underline: Box::new(str::to_owned),
    highlight_code: None,
    code_block_indent: None,
});
let markdown = Markdown::new(
    "# Hello\n\n**World**".into(),
    MarkdownOptions {
        padding_x: 1,
        padding_y: 0,
        default_text_style: Some(DefaultTextStyle {
            decorations: vec![TextDecoration::Italic],
            ..DefaultTextStyle::default()
        }),
    },
    theme,
    TerminalImage::new(|_| None, || 1),
);
assert_eq!(markdown.render(9), [" Hello   ", "         ", " World   "]);
markdown.set_text("Updated".into());
assert_eq!(markdown.render(9), [" Updated "]);
```
