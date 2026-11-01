//! Extension notifications through the installed host and retained transcript.
use super::super::{isolated, painted, started};
use super::{cases, port};
use maestro_app::NotificationType;
use maestro_tui::Component;
use serde::Deserialize;
use std::rc::Rc;

/// A source-recorded notification operation.
#[derive(Deserialize)]
#[serde(tag = "op", deny_unknown_fields)]
enum Input {
    /// One severity notice.
    #[serde(rename = "notify")]
    Notify {
        /// Absent or supplied severity.
        kind: Option<NotificationType>,
    },
    /// Adjacent status updates separated by a warning.
    #[serde(rename = "notify-sequence")]
    Sequence,
}

/// The source-recorded paint and frame request for a notice.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Notice {
    /// Complete styled transcript rows.
    lines: Vec<String>,
    /// Retained transcript children.
    children: usize,
    /// Whether the notice requested a frame.
    render_requested: bool,
}

/// The source-recorded paint after adjacent and interrupted status updates.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Sequence {
    /// Rows after the adjacent updates.
    adjacent: Vec<String>,
    /// Rows after the warning and final update.
    #[serde(rename = "final")]
    final_rows: Vec<String>,
    /// Whether the operations requested a frame.
    render_requested: bool,
}

#[test]
fn notifications_choose_status_warning_or_error() {
    let test = "notifications_choose_status_warning_or_error";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Notice>(test) {
            let Input::Notify { kind } = input else {
                panic!("a notification")
            };
            let world = started(directory).await;
            world.mode.chat_container.clear();
            port(&world.mode)
                .notify("first\n界".to_owned(), kind)
                .unwrap();
            let theme = world.themes.theme().get().unwrap();
            assert_eq!(
                world.mode.chat_container.render(80),
                painted(&theme, &expected.lines)
            );
            assert_eq!(
                world.mode.chat_container.children().borrow().len(),
                expected.children
            );
            assert_eq!(world.runtime.pending() > 0, expected.render_requested);
        }
    });
}

#[test]
fn notification_status_reuses_only_the_adjacent_pair() {
    let test = "notification_status_reuses_only_the_adjacent_pair";
    isolated(test, async |directory| {
        for (input, expected) in cases::<Input, Sequence>(test) {
            assert!(matches!(input, Input::Sequence));
            let world = started(directory).await;
            world.mode.chat_container.clear();
            let host = port(&world.mode);
            host.notify("first".to_owned(), None).unwrap();
            let first = world.mode.chat_container.children().borrow().clone();
            host.notify("second".to_owned(), Some(NotificationType::Info))
                .unwrap();
            let second = world.mode.chat_container.children().borrow().clone();
            assert_eq!(second.len(), 2);
            assert!(first.iter().zip(&second).all(|(a, b)| Rc::ptr_eq(a, b)));
            let theme = world.themes.theme().get().unwrap();
            assert_eq!(
                world.mode.chat_container.render(80),
                painted(&theme, &expected.adjacent)
            );
            host.notify("break".to_owned(), Some(NotificationType::Warning))
                .unwrap();
            host.notify("third".to_owned(), None).unwrap();
            let rows = world.mode.chat_container.children();
            assert_eq!(rows.borrow().len(), 6);
            assert!(!Rc::ptr_eq(&first[1], &rows.borrow()[5]));
            assert_eq!(
                world.mode.chat_container.render(80),
                painted(&theme, &expected.final_rows)
            );
            assert_eq!(world.runtime.pending() > 0, expected.render_requested);
        }
    });
}
