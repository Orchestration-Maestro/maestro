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

/// Refusal to select or remove a screen with an invalid lifetime.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenSetError {
    /// The alternate screen has not been initialized.
    Uninitialized,
    /// The primary screen must remain resident.
    PrimaryRemoval,
    /// The active alternate screen must be deselected before removal.
    ActiveRemoval,
}

impl std::fmt::Display for ScreenSetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Uninitialized => "alternate screen is not initialized",
            Self::PrimaryRemoval => "primary screen cannot be removed",
            Self::ActiveRemoval => "active screen cannot be removed",
        })
    }
}

impl std::error::Error for ScreenSetError {}

/// Initialized alternate storage with its selection state.
struct AltScreen {
    /// Resident alternate screen.
    screen: Box<Screen>,
    /// Whether this screen rather than the primary is selected.
    active: bool,
}

/// Independent terminal screens selected by an owned key.
pub struct ScreenSet {
    /// Always-resident primary screen.
    primary: Screen,
    /// Alternate storage and selection, allocated only on first use.
    alternate: Option<AltScreen>,
    /// Removal counters, independent of screen initialization.
    generations: [usize; 2],
}

impl ScreenSet {
    /// Initialize only the primary screen.
    #[must_use]
    pub fn init(opts: Options) -> Self {
        Self {
            primary: Screen::init(opts),
            alternate: None,
            generations: [0; 2],
        }
    }

    /// Identify the selected resident screen.
    #[must_use]
    pub fn active_key(&self) -> Key {
        match self.alternate {
            Some(AltScreen { active: true, .. }) => Key::Alternate,
            _ => Key::Primary,
        }
    }

    /// Borrow the selected resident screen.
    #[must_use]
    pub fn active(&self) -> &Screen {
        match &self.alternate {
            Some(AltScreen {
                screen,
                active: true,
            }) => screen,
            _ => &self.primary,
        }
    }

    /// Mutably borrow the selected resident screen.
    pub fn active_mut(&mut self) -> &mut Screen {
        match &mut self.alternate {
            Some(AltScreen {
                screen,
                active: true,
            }) => screen,
            _ => &mut self.primary,
        }
    }

    /// Borrow an initialized screen without creating storage.
    #[must_use]
    pub fn get(&self, key: Key) -> Option<&Screen> {
        match (key, &self.alternate) {
            (Key::Primary, _) => Some(&self.primary),
            (Key::Alternate, None) => None,
            (Key::Alternate, Some(alternate)) => Some(&alternate.screen),
        }
    }

    /// Mutably borrow an initialized screen without creating storage.
    pub fn get_mut(&mut self, key: Key) -> Option<&mut Screen> {
        match (key, &mut self.alternate) {
            (Key::Primary, _) => Some(&mut self.primary),
            (Key::Alternate, None) => None,
            (Key::Alternate, Some(alternate)) => Some(&mut alternate.screen),
        }
    }

    /// Borrow both resident screens in source/destination order without cloning a cursor.
    pub(crate) fn pair_mut(&mut self, from: Key) -> Option<(&mut Screen, &mut Screen)> {
        let alternate = &mut self.alternate.as_mut()?.screen;
        Some(match from {
            Key::Primary => (&mut self.primary, alternate),
            Key::Alternate => (alternate, &mut self.primary),
        })
    }

    /// Read the lifetime counter for a screen key.
    #[must_use]
    pub fn generation(&self, key: Key) -> usize {
        self.generations[key.index()]
    }

    /// Borrow a screen, initializing absent alternate storage with the options.
    pub fn get_init(&mut self, key: Key, opts: Options) -> &mut Screen {
        if key == Key::Primary {
            return &mut self.primary;
        }
        &mut self
            .alternate
            .get_or_insert_with(|| AltScreen {
                screen: Box::new(Screen::init(opts)),
                active: false,
            })
            .screen
    }

    /// Remove inactive alternate storage, advancing its lifetime if present.
    ///
    /// # Errors
    /// Refuses removal of the primary or the selected alternate screen.
    pub fn remove(&mut self, key: Key) -> Result<(), ScreenSetError> {
        if key == Key::Primary {
            return Err(ScreenSetError::PrimaryRemoval);
        }
        match self.alternate {
            None => return Ok(()),
            Some(AltScreen { active: true, .. }) => return Err(ScreenSetError::ActiveRemoval),
            Some(_) => {}
        }
        self.alternate = None;
        self.generations[key.index()] = self.generation(key).wrapping_add(1);
        Ok(())
    }

    /// Select an initialized screen.
    ///
    /// # Errors
    /// Refuses selection of an uninitialized alternate screen.
    pub fn switch_to(&mut self, key: Key) -> Result<(), ScreenSetError> {
        match &mut self.alternate {
            Some(alternate) => alternate.active = key == Key::Alternate,
            None if key == Key::Alternate => return Err(ScreenSetError::Uninitialized),
            None => {}
        }
        Ok(())
    }
}
