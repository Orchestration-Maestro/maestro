//! Packed metadata for a terminal row.

/// Row flags and page-local cell offset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Row(u64);

/// The prompt role of a terminal row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SemanticPrompt {
    /// No prompt cells.
    None = 0,
    /// The beginning of a primary prompt.
    Prompt = 1,
    /// A continuation of a previous prompt.
    PromptContinuation = 2,
}

impl Row {
    /// Read the soft-wrap flag.
    #[must_use]
    pub fn wrap(self) -> bool {
        self.0 & (1 << 32) != 0
    }

    /// Set the soft-wrap flag.
    pub fn set_wrap(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 32)) | (u64::from(value) << 32);
    }

    /// Read the previous-row continuation flag.
    #[must_use]
    pub fn wrap_continuation(self) -> bool {
        self.0 & (1 << 33) != 0
    }

    /// Set the previous-row continuation flag.
    pub fn set_wrap_continuation(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 33)) | (u64::from(value) << 33);
    }

    /// Read the cluster flag.
    #[must_use]
    pub fn grapheme(self) -> bool {
        self.0 & (1 << 34) != 0
    }

    /// Set the cluster flag.
    pub fn set_grapheme(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 34)) | (u64::from(value) << 34);
    }

    /// Read the style-table flag.
    #[must_use]
    pub fn styled(self) -> bool {
        self.0 & (1 << 35) != 0
    }

    /// Set the style-table flag.
    pub fn set_styled(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 35)) | (u64::from(value) << 35);
    }

    /// Read the hyperlink flag.
    #[must_use]
    pub fn hyperlink(self) -> bool {
        self.0 & (1 << 36) != 0
    }

    /// Set the hyperlink flag.
    pub fn set_hyperlink(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 36)) | (u64::from(value) << 36);
    }

    /// Read the graphics-placeholder flag.
    #[must_use]
    pub fn kitty_virtual_placeholder(self) -> bool {
        self.0 & (1 << 39) != 0
    }

    /// Set the graphics-placeholder flag.
    pub fn set_kitty_virtual_placeholder(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 39)) | (u64::from(value) << 39);
    }

    /// Read the redraw flag.
    #[must_use]
    pub fn dirty(self) -> bool {
        self.0 & (1 << 40) != 0
    }

    /// Set the redraw flag.
    pub fn set_dirty(&mut self, value: bool) {
        self.0 = (self.0 & !(1 << 40)) | (u64::from(value) << 40);
    }

    /// Read the shell prompt role.
    #[must_use]
    pub fn semantic_prompt(self) -> SemanticPrompt {
        match (self.0 >> 37) & 3 {
            1 => SemanticPrompt::Prompt,
            2 => SemanticPrompt::PromptContinuation,
            _ => SemanticPrompt::None,
        }
    }

    /// Set the shell prompt role.
    pub fn set_semantic_prompt(&mut self, value: SemanticPrompt) {
        self.0 = (self.0 & !(3 << 37)) | (u64::from(value as u8) << 37);
    }

    /// Whether the row flags indicate externally owned table data.
    #[must_use]
    pub fn managed_memory(self) -> bool {
        self.styled() || self.hyperlink() || self.grapheme()
    }
}
