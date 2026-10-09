//! Independent request barriers and root-exported command construction.
#![cfg(test)]
use maestro_tui::autocomplete::{
    AutocompleteOperations, Command, CompletionOptions, CursorPosition, DirectoryEntry,
};
use maestro_tui::{
    AutocompleteItem, AutocompleteProvider, AutocompleteSuggestions, CombinedAutocompleteProvider,
    SlashCommand,
};
use std::cell::Cell;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

/// Entry and release witnesses owned by the observed requests.
#[derive(Default)]
struct Control {
    /// Independently released requests.
    releases: [Cell<bool>; 2],
    /// Witnesses fired on entry to each search.
    entered: [Cell<bool>; 2],
}
impl Control {
    /// Await the selected request's explicit release before producing its output.
    async fn wait(&self, index: usize, signal: &Cell<bool>) -> io::Result<()> {
        self.entered[index].set(true);
        std::future::poll_fn(|_| {
            self.releases[index]
                .get()
                .then_some(())
                .map_or(Poll::Pending, Poll::Ready)
        })
        .await;
        if signal.get() {
            Err(io::Error::other("cancelled"))
        } else {
            Ok(())
        }
    }
}
/// An adapter generating results from borrowed request operands.
pub(crate) struct Search(Rc<Control>);
impl AutocompleteOperations for Search {
    type Signal = Cell<bool>;
    fn home_dir(&self) -> io::Result<String> {
        unreachable!("no home operand")
    }
    fn read_dir(&self, _: &str) -> io::Result<Vec<DirectoryEntry>> {
        unreachable!("no direct paths")
    }
    fn is_directory(&self, _: &str) -> io::Result<bool> {
        unreachable!("no directory operand")
    }
    fn compare(&self, _: &str, _: &str) -> io::Result<std::cmp::Ordering> {
        unreachable!("no collation")
    }
    fn is_aborted(&self, signal: &Self::Signal) -> bool {
        signal.get()
    }
    fn run_fd<'a>(
        &'a self,
        _: &'a str,
        args: &'a [String],
        signal: &'a Self::Signal,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<u8>>> + 'a>> {
        let query = args.last().unwrap();
        Box::pin(async move {
            self.0.wait(usize::from(query == "beta"), signal).await?;
            Ok(format!("{query}.txt\0").into_bytes())
        })
    }
}
/// A second adapter supplying independently recorded bytes at the same seam.
struct RecordedSearch(Search);
impl AutocompleteOperations for RecordedSearch {
    type Signal = Cell<bool>;
    fn home_dir(&self) -> io::Result<String> {
        self.0.home_dir()
    }
    fn read_dir(&self, p: &str) -> io::Result<Vec<DirectoryEntry>> {
        self.0.read_dir(p)
    }
    fn is_directory(&self, p: &str) -> io::Result<bool> {
        self.0.is_directory(p)
    }
    fn compare(&self, l: &str, r: &str) -> io::Result<std::cmp::Ordering> {
        self.0.compare(l, r)
    }
    fn is_aborted(&self, s: &Self::Signal) -> bool {
        self.0.is_aborted(s)
    }
    fn run_fd<'a>(
        &'a self,
        _: &'a str,
        args: &'a [String],
        signal: &'a Self::Signal,
    ) -> Pin<Box<dyn Future<Output = io::Result<Vec<u8>>> + 'a>> {
        let index = usize::from(args.last().unwrap() == "beta");
        Box::pin(async move {
            self.0.0.wait(index, signal).await?;
            Ok([b"alpha.txt\0".as_slice(), b"beta.txt\0".as_slice()][index].to_vec())
        })
    }
}
/// Swap effects while keeping the public request observer unchanged.
pub(crate) fn observe_pending(recorded: bool) -> Vec<Option<AutocompleteSuggestions>> {
    let control = Rc::new(Control::default());
    let search = Search(Rc::clone(&control));
    if recorded {
        observe_requests(RecordedSearch(search), &control)
    } else {
        observe_requests(search, &control)
    }
}
/// Poll concurrent requests using entry and release witnesses of those requests.
fn observe_requests(
    operations: impl AutocompleteOperations<Signal = Cell<bool>>,
    control: &Control,
) -> Vec<Option<AutocompleteSuggestions>> {
    let provider =
        CombinedAutocompleteProvider::new(vec![], "/work".into(), Some("fd".into()), operations);
    let signals = [Cell::new(false), Cell::new(false)];
    let lines = [["@alpha".into()], ["@beta".into()]];
    let mut first = provider.get_suggestions(
        &lines[0],
        CursorPosition { line: 0, col: 6 },
        CompletionOptions {
            signal: &signals[0],
            force: None,
        },
    );
    let mut second = provider.get_suggestions(
        &lines[1],
        CursorPosition { line: 0, col: 5 },
        CompletionOptions {
            signal: &signals[1],
            force: None,
        },
    );
    let mut context = Context::from_waker(Waker::noop());
    assert!(first.as_mut().poll(&mut context).is_pending());
    assert!(second.as_mut().poll(&mut context).is_pending());
    assert!(control.entered.iter().all(Cell::get));
    control.releases[1].set(true);
    let Poll::Ready(second) = second.as_mut().poll(&mut context) else {
        panic!("released request remains pending")
    };
    let second = second.unwrap().unwrap();
    assert_eq!(second.prefix, "@beta");
    assert_eq!(
        second.items,
        vec![AutocompleteItem {
            value: "@beta.txt".into(),
            label: "beta.txt".into(),
            description: Some("beta.txt".into())
        }]
    );
    assert!(first.as_mut().poll(&mut context).is_pending());
    signals[0].set(true);
    control.releases[0].set(true);
    let Poll::Ready(first) = first.as_mut().poll(&mut context) else {
        panic!("released aborted request remains pending")
    };
    let first = first.unwrap();
    assert!(first.is_none());
    vec![first, Some(second)]
}
/// Build both exported command forms without filesystem access.
pub(crate) fn exported_provider() -> CombinedAutocompleteProvider<Search> {
    let commands = vec![
        Command::SlashCommand(SlashCommand {
            name: "alpha".into(),
            description: None,
            argument_hint: None,
            get_argument_completions: None,
        }),
        Command::AutocompleteItem(AutocompleteItem {
            value: "beta".into(),
            label: "beta".into(),
            description: None,
        }),
    ];
    CombinedAutocompleteProvider::new(commands, "/work".into(), None, Search(Rc::default()))
}
