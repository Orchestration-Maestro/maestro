//! Owned Markdown parsing exposed to presentation consumers.

/// Markdown documents and tokenizer operations used by browser rendering and export.
pub mod markdown {
    #[doc(inline)]
    pub use maestro_markdown::{
        Alignment, BlockToken, BlockTokenizer, Cell, Document, ImageMetadata, InlineAtom,
        InlineContext, InlineScope, InlineToken, InlineTokenizer, Kind, LinkType, MarkedOptions,
        Node, Table, parse_with_options, parse_with_tokenizer,
    };
}
