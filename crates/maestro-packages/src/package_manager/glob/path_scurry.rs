//! Ordered authored child identities with a confirmed listing prefix.
/// One retained identity and its current native comparison spelling.
pub(super) struct Child {
    /// Spelling retained by literal lookup or first listing.
    pub(super) authored_name: String,
    /// Native spelling used by component matching.
    pub(super) name: String,
    /// Platform-selected lookup identity.
    comparison_key: String,
}
/// Directory observations, without retained filesystem metadata.
#[derive(Default)]
pub(super) struct Children {
    /// Confirmed prefix followed by provisional literal lookups.
    entries: Vec<Child>,
    /// Start of the provisional suffix.
    provisional: usize,
    /// Whether a successful listing established the prefix.
    pub(super) complete: bool,
}
impl Children {
    /// Looks up a child without replacing its authored spelling.
    pub(super) fn child(&mut self, name: &str, nocase: bool) -> &Child {
        let key = comparison_key(name, nocase);
        let index = self
            .entries
            .iter()
            .position(|child| child.comparison_key == key)
            .unwrap_or_else(|| {
                self.entries.push(Child {
                    authored_name: name.into(),
                    name: name.into(),
                    comparison_key: key,
                });
                self.entries.len() - 1
            });
        &self.entries[index]
    }
    /// Promotes provisional identities and establishes the native listing order.
    pub(super) fn readdir_sync(&mut self, names: impl Iterator<Item = String>, nocase: bool) {
        for name in names {
            let key = comparison_key(&name, nocase);
            let index = self.entries[self.provisional..]
                .iter()
                .position(|child| child.comparison_key == key)
                .map(|index| index + self.provisional);
            match index {
                Some(index) if index == self.provisional => self.entries[index].name = name,
                Some(index) => {
                    let mut child = self.entries.remove(index);
                    child.name = name;
                    self.entries.insert(0, child);
                }
                None => self.entries.insert(
                    0,
                    Child {
                        authored_name: name.clone(),
                        name,
                        comparison_key: key,
                    },
                ),
            }
            self.provisional += 1;
        }
        self.complete = true;
    }
    /// Borrows only the confirmed listing, excluding later literal lookups.
    pub(super) fn listed(&self) -> &[Child] {
        &self.entries[..self.provisional]
    }
}
/// Selects name comparison without compatibility normalization.
pub(super) fn comparison_key(name: &str, nocase: bool) -> String {
    if nocase {
        name.to_lowercase()
    } else {
        name.into()
    }
}
