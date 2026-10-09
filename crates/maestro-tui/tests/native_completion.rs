//! Native completion operations through the provider.

/// Shared controlled completion fixtures.
pub mod fixtures {
    pub mod completion_native;
    pub mod futures;
}
use fixtures::futures::block_on;
use maestro_tui::autocomplete::{CompletionOptions, CursorPosition, NativeAutocompleteOperations};
use maestro_tui::{AutocompleteProvider, CombinedAutocompleteProvider};

/// Run one native request through the public provider.
fn query<E: Fn(&str) -> Option<String>>(
    provider: &CombinedAutocompleteProvider<NativeAutocompleteOperations<E>>,
    text: &str,
) -> Result<Option<maestro_tui::AutocompleteSuggestions>, maestro_tui::autocomplete::CompletionError>
{
    let lines = [text.to_owned()];
    block_on(provider.get_suggestions(
        &lines,
        CursorPosition {
            line: 0,
            col: text.len(),
        },
        CompletionOptions {
            signal: &maestro_cancellation::Cancellation::new(),
            force: Some(true),
        },
    ))
}

#[test]
fn native_filesystem_errors_and_home_expansion_follow_the_same_provider_path() {
    use fixtures::completion_native::Tree;
    if std::env::var_os("MAESTRO_COMPLETION_HOME_CHILD").is_some() {
        let provider = CombinedAutocompleteProvider::new(
            vec![],
            "/missing".into(),
            None,
            NativeAutocompleteOperations::default(),
        );
        let found = query(&provider, "~/").expect("provider succeeds").unwrap();
        assert_eq!(found.prefix, "~/");
        assert_eq!(found.items.len(), 1);
        assert_eq!(found.items[0].value, "~/owned.txt");
        return;
    }
    let tree = Tree::new().expect("temporary directory created");
    tree.file("owned.txt").expect("fixture entry created");
    let provider = CombinedAutocompleteProvider::new(
        vec![],
        tree.authored(),
        None,
        NativeAutocompleteOperations::default(),
    );
    assert!(
        query(&provider, "./missing/")
            .expect("provider succeeds")
            .is_none()
    );
    assert!(
        query(&provider, "./owned.txt/")
            .expect("provider succeeds")
            .is_none()
    );
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "native_filesystem_errors_and_home_expansion_follow_the_same_provider_path",
        ])
        .env("MAESTRO_COMPLETION_HOME_CHILD", "1")
        .env("HOME", &tree.0)
        .env("USERPROFILE", &tree.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

#[cfg(unix)]
#[test]
fn native_directories_follow_links_without_filtering_hidden_entries() {
    use fixtures::completion_native::Tree;
    let tree = Tree::new().expect("temporary directory created");
    for name in ["\u{e9}", "e\u{301}", "A", "a", ".hidden", "file.txt"] {
        tree.file(name).expect("fixture entry created");
    }
    for name in ["z dir", ".git"] {
        tree.directory(name).expect("fixture entry created");
    }
    for (name, target) in [
        ("dir-link", "z dir"),
        ("file-link", "file.txt"),
        ("broken-link", "missing"),
    ] {
        std::os::unix::fs::symlink(target, tree.0.join(name)).unwrap();
    }
    let provider = CombinedAutocompleteProvider::new(
        vec![],
        tree.authored(),
        None,
        NativeAutocompleteOperations::with_environment(|_| Some("en_US".into())),
    );
    for (text, expected) in NATIVE_PATHS {
        let result = query(&provider, text).expect("provider succeeds");
        let expected = expected.map(|(prefix, items)| maestro_tui::AutocompleteSuggestions {
            prefix: prefix.into(),
            items: items
                .iter()
                .map(|(value, label)| maestro_tui::AutocompleteItem {
                    value: (*value).into(),
                    label: (*label).into(),
                    description: None,
                })
                .collect(),
        });
        assert_eq!(result, expected, "{text}");
    }
}

/// Direct-native query expectations, including all candidate fields.
type NativePath = (
    &'static str,
    Option<(&'static str, &'static [(&'static str, &'static str)])>,
);

/// Original native input order and corrected directory classification.
const NATIVE_PATHS: &[NativePath] = &[
    (
        "./",
        Some((
            "./",
            &[
                ("./.git/", ".git/"),
                ("./dir-link/", "dir-link/"),
                ("\"./z dir/\"", "z dir/"),
                ("./.hidden", ".hidden"),
                ("./a", "a"),
                ("./A", "A"),
                ("./broken-link", "broken-link"),
                ("./e\u{301}", "e\u{301}"),
                ("./\u{e9}", "\u{e9}"),
                ("./file-link", "file-link"),
                ("./file.txt", "file.txt"),
            ],
        )),
    ),
    (
        "\"",
        Some((
            "\"",
            &[
                ("\".git/\"", ".git/"),
                ("\"dir-link/\"", "dir-link/"),
                ("\"z dir/\"", "z dir/"),
                ("\".hidden\"", ".hidden"),
                ("\"a\"", "a"),
                ("\"A\"", "A"),
                ("\"broken-link\"", "broken-link"),
                ("\"e\u{301}\"", "e\u{301}"),
                ("\"\u{e9}\"", "\u{e9}"),
                ("\"file-link\"", "file-link"),
                ("\"file.txt\"", "file.txt"),
            ],
        )),
    ),
    ("./dir", Some(("./dir", &[("./dir-link/", "dir-link/")]))),
    (
        "./broken",
        Some(("./broken", &[("./broken-link", "broken-link")])),
    ),
    ("./none", None),
];

/// Controlled enumeration over the native locale operation.
struct LocaleFiles<E> {
    /// Native comparison with an injected environment.
    native: NativeAutocompleteOperations<E>,
    /// Entries whose input order the test owns.
    entries: Vec<maestro_tui::autocomplete::DirectoryEntry>,
    /// Controlled enumeration failure before locale is needed.
    fail_listing: bool,
}
impl<E: Fn(&str) -> Option<String>> maestro_tui::autocomplete::AutocompleteOperations
    for LocaleFiles<E>
{
    type Signal = ();
    fn is_aborted(&self, (): &Self::Signal) -> bool {
        false
    }
    fn run_fd<'a>(
        &'a self,
        _: &'a str,
        _: &'a [String],
        (): &'a Self::Signal,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<Vec<u8>>> + 'a>> {
        Box::pin(async { Err(std::io::Error::other("no executable")) })
    }
    fn home_dir(&self) -> std::io::Result<String> {
        self.native.home_dir()
    }
    fn read_dir(&self, _: &str) -> std::io::Result<Vec<maestro_tui::autocomplete::DirectoryEntry>> {
        if self.fail_listing {
            return Err(std::io::Error::other("listing failed"));
        }
        Ok(self
            .entries
            .iter()
            .map(|entry| maestro_tui::autocomplete::DirectoryEntry {
                name: entry.name.clone(),
                kind: entry.kind,
            })
            .collect())
    }
    fn is_directory(&self, path: &str) -> std::io::Result<bool> {
        self.native.is_directory(path)
    }
    fn compare(&self, left: &str, right: &str) -> std::io::Result<std::cmp::Ordering> {
        self.native.compare(left, right)
    }
}

/// Exercise a controlled listing through the provider.
fn locale_query<E: Fn(&str) -> Option<String>>(
    provider: &CombinedAutocompleteProvider<LocaleFiles<E>>,
    text: &str,
    force: bool,
) -> Result<Option<maestro_tui::AutocompleteSuggestions>, maestro_tui::autocomplete::CompletionError>
{
    let lines = [text.to_owned()];
    block_on(provider.get_suggestions(
        &lines,
        CursorPosition {
            line: 0,
            col: text.len(),
        },
        CompletionOptions {
            signal: &(),
            force: Some(force),
        },
    ))
}

#[test]
fn native_collation_matches_locale_ordering() {
    use maestro_tui::autocomplete::{DirectoryEntry, DirectoryEntryKind};
    for case in LOCALES {
        let native = NativeAutocompleteOperations::with_environment(|key| {
            case.environment
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| (*value).to_owned())
        });
        let operations = LocaleFiles {
            native,
            entries: LOCALE_NAMES
                .iter()
                .map(|name| DirectoryEntry {
                    name: (*name).into(),
                    kind: DirectoryEntryKind::Other,
                })
                .collect(),
            fail_listing: false,
        };
        let provider = CombinedAutocompleteProvider::new(vec![], "/work".into(), None, operations);
        let found = locale_query(&provider, "", true)
            .expect("provider succeeds")
            .unwrap();
        assert_eq!(found.prefix, "");
        let expected: Vec<_> = case
            .expected
            .iter()
            .map(|name| maestro_tui::AutocompleteItem {
                value: (*name).into(),
                label: (*name).into(),
                description: None,
            })
            .collect();
        assert_eq!(found.items, expected, "{:?}", case.environment);
    }
}

/// Locale input with exactly the sorted output fields consumed by the test.
struct LocaleCase {
    /// Present environment operands.
    environment: &'static [(&'static str, &'static str)],
    /// Stable ordered candidate spellings.
    expected: &'static [&'static str],
}
/// Enumeration order shared by all locale observations.
const LOCALE_NAMES: &[&str] = &[
    "\u{e4}",
    "z",
    "a",
    "A",
    "\u{e5}",
    "a2",
    "a10",
    "\u{e9}",
    "e\u{301}",
    "\u{130}",
    "I",
    "i",
    "\u{131}",
    "Z",
    "\u{c6}",
    "\u{f8}",
    "\u{3072}",
    "\u{30a2}",
    "\u{1f600}",
];
/// Recorded locale observations without runtime provenance fields.
const LOCALES: &[LocaleCase] = &[
    LocaleCase {
        environment: &[
            ("LC_ALL", "sv_SE.UTF-8"),
            ("LC_MESSAGES", "tr_TR.UTF-8"),
            ("LANG", "en_US.UTF-8"),
        ],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "a10",
            "a2",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "z",
            "Z",
            "\u{e5}",
            "\u{e4}",
            "\u{c6}",
            "\u{f8}",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[
            ("LC_ALL", ""),
            ("LC_MESSAGES", "sv_SE.UTF-8"),
            ("LANG", "en_US.UTF-8"),
        ],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LC_MESSAGES", "sv_SE.UTF-8"), ("LANG", "en_US.UTF-8")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "a10",
            "a2",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "z",
            "Z",
            "\u{e5}",
            "\u{e4}",
            "\u{c6}",
            "\u{f8}",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "sv_SE.UTF-8")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "a10",
            "a2",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "z",
            "Z",
            "\u{e5}",
            "\u{e4}",
            "\u{c6}",
            "\u{f8}",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LC_COLLATE", "sv_SE.UTF-8"), ("LANG", "en_US.UTF-8")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "C")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "en_US.UTF-8")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "de_DE.UTF-8")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "tr_TR.UTF-8")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "\u{131}",
            "I",
            "i",
            "\u{130}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "ja_JP.UTF-8")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "da_DK.UTF-8")],
        expected: &[
            "\u{1f600}",
            "A",
            "a",
            "a10",
            "a2",
            "\u{e9}",
            "e\u{301}",
            "I",
            "i",
            "\u{130}",
            "\u{131}",
            "Z",
            "z",
            "\u{c6}",
            "\u{e4}",
            "\u{f8}",
            "\u{e5}",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "POSIX")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "bogus")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "de_DE@collation=phonebook")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "en_US.UTF-8@euro")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "sv-SE")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "a10",
            "a2",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "z",
            "Z",
            "\u{e5}",
            "\u{e4}",
            "\u{c6}",
            "\u{f8}",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "C.UTF-8")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", "")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
    LocaleCase {
        environment: &[("LANG", ".")],
        expected: &[
            "\u{1f600}",
            "a",
            "A",
            "\u{e5}",
            "\u{e4}",
            "a10",
            "a2",
            "\u{c6}",
            "\u{e9}",
            "e\u{301}",
            "i",
            "I",
            "\u{130}",
            "\u{131}",
            "\u{f8}",
            "z",
            "Z",
            "\u{30a2}",
            "\u{3072}",
        ],
    },
];

#[test]
fn locale_selection_is_lazy_cached_and_uses_the_first_present_variable() {
    use maestro_tui::autocomplete::{DirectoryEntry, DirectoryEntryKind};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    assert_locale_reads_avoided().expect("avoided locale reads");
    let changed = Rc::new(Cell::new(false));
    let reads = Rc::new(RefCell::new(Vec::new()));
    let native = {
        let changed = Rc::clone(&changed);
        let reads = Rc::clone(&reads);
        NativeAutocompleteOperations::with_environment(move |key| {
            reads.borrow_mut().push(key.to_owned());
            match key {
                "LC_ALL" => None,
                "LC_MESSAGES" => Some(if changed.get() { "en_US" } else { "tr_TR" }.into()),
                _ => panic!("later operand must not be read"),
            }
        })
    };
    let operations = LocaleFiles {
        native,
        entries: ["i", "I"]
            .into_iter()
            .map(|name| DirectoryEntry {
                name: name.into(),
                kind: DirectoryEntryKind::Other,
            })
            .collect(),
        fail_listing: false,
    };
    let provider = CombinedAutocompleteProvider::new(vec![], "/work".into(), None, operations);
    let first = locale_query(&provider, "", true)
        .expect("provider succeeds")
        .unwrap();
    assert_eq!(
        first
            .items
            .iter()
            .map(|item| item.label.as_str())
            .collect::<Vec<_>>(),
        ["I", "i"]
    );
    assert_eq!(*reads.borrow(), ["LC_ALL", "LC_MESSAGES"]);
    changed.set(true);
    assert_eq!(
        locale_query(&provider, "", true)
            .expect("provider succeeds")
            .unwrap(),
        first
    );
    assert_eq!(*reads.borrow(), ["LC_ALL", "LC_MESSAGES"]);
}

/// Prove the command and listing branches that do not need collation.
fn assert_locale_reads_avoided() -> Result<(), maestro_tui::autocomplete::CompletionError> {
    use maestro_tui::autocomplete::{Command, DirectoryEntry, DirectoryEntryKind};
    use std::cell::Cell;
    for (fail_listing, kinds) in [
        (true, vec![]),
        (false, vec![]),
        (false, vec![DirectoryEntryKind::Other]),
        (
            false,
            vec![DirectoryEntryKind::Directory, DirectoryEntryKind::Other],
        ),
    ] {
        let reads = Cell::new(0);
        let native = NativeAutocompleteOperations::with_environment(|_| {
            reads.set(reads.get() + 1);
            Some("en_US".into())
        });
        let entries = kinds
            .into_iter()
            .enumerate()
            .map(|(i, kind)| DirectoryEntry {
                name: format!("entry{i}"),
                kind,
            })
            .collect();
        let operations = LocaleFiles {
            native,
            entries,
            fail_listing,
        };
        let provider = CombinedAutocompleteProvider::new(
            vec![Command::AutocompleteItem(maestro_tui::AutocompleteItem {
                value: "help".into(),
                label: "help".into(),
                description: None,
            })],
            "/work".into(),
            None,
            operations,
        );
        assert_eq!(
            locale_query(&provider, "/he", false)?
                .ok_or("command candidates")?
                .items[0]
                .value,
            "help"
        );
        let _ = locale_query(&provider, "", true)?;
        assert_eq!(reads.get(), 0);
    }
    Ok(())
}
