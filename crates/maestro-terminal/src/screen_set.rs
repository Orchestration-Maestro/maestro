//! Lazy ownership of primary and alternate terminal screens.

use crate::{Screen, screen::Options};

/// Terminal screen identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    /// Always-initialized primary screen.
    Primary,
    /// Lazily initialized alternate screen.
    Alternate,
}

impl Key {
    /// Index of this screen's lifetime counter.
    fn index(self) -> usize {
        match self {
            Self::Primary => 0,
            Self::Alternate => 1,
        }
    }
}

/// Independent terminal screens selected by an owned key.
pub struct ScreenSet {
    /// Key of the currently selected screen.
    pub active_key: Key,
    /// Always-resident primary screen.
    primary: Screen,
    /// Alternate storage allocated only on first use.
    alternate: Option<Box<Screen>>,
    /// Removal counters, independent of screen initialization.
    generations: [usize; 2],
}

impl ScreenSet {
    /// Initialize only the primary screen.
    #[must_use]
    pub fn init(opts: Options) -> Self {
        Self {
            active_key: Key::Primary,
            primary: Screen::init(opts),
            alternate: None,
            generations: [0; 2],
        }
    }

    /// Borrow an initialized screen without creating storage.
    #[must_use]
    pub fn get(&self, key: Key) -> Option<&Screen> {
        match key {
            Key::Primary => Some(&self.primary),
            Key::Alternate => self.alternate.as_deref(),
        }
    }

    /// Mutably borrow an initialized screen without creating storage.
    pub fn get_mut(&mut self, key: Key) -> Option<&mut Screen> {
        match key {
            Key::Primary => Some(&mut self.primary),
            Key::Alternate => self.alternate.as_deref_mut(),
        }
    }

    /// Read the lifetime counter for a screen key.
    #[must_use]
    pub fn generation(&self, key: Key) -> usize {
        self.generations[key.index()]
    }

    /// Borrow a screen, initializing absent alternate storage with the options.
    pub fn get_init(&mut self, key: Key, opts: Options) -> &mut Screen {
        match key {
            Key::Primary => &mut self.primary,
            Key::Alternate => self
                .alternate
                .get_or_insert_with(|| Box::new(Screen::init(opts))),
        }
    }

    /// Remove alternate storage, advancing its lifetime only if it existed.
    ///
    /// # Panics
    /// Panics if asked to remove the primary screen.
    pub fn remove(&mut self, key: Key) {
        assert_ne!(key, Key::Primary);
        if self.alternate.take().is_some() {
            self.generations[key.index()] = self.generation(key).wrapping_add(1);
        }
    }

    /// Select an initialized screen.
    ///
    /// # Panics
    /// Panics if the target screen has not been initialized.
    pub fn switch_to(&mut self, key: Key) {
        assert!(self.get(key).is_some());
        self.active_key = key;
    }
}
