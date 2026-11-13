#![cfg(unix)]
//! The interactive mode's read-only view: its regions in layout order, each apart from
//! the others, and the editor text.
use crate::interactive_mode_support::{World, isolation::isolated, started};
use maestro_chat::Region;
use maestro_extensions::{ExtensionWidgetOptions, WidgetPlacement, types::WidgetContent};

/// Every region, from the top of the screen to the bottom.
const REGIONS: [Region; 8] = [
    Region::Header,
    Region::Chat,
    Region::PendingMessages,
    Region::Status,
    Region::WidgetsAbove,
    Region::Editor,
    Region::WidgetsBelow,
    Region::Footer,
];

/// Where the first painted row of `region` starts in `frame`.
fn position(world: &World, frame: &str, region: Region) -> usize {
    let rows = world.region(region);
    let painted = rows.iter().find(|row| !row.trim().is_empty());
    let row = painted.unwrap_or_else(|| panic!("{region:?} paints no row"));
    frame
        .find(row.as_str())
        .unwrap_or_else(|| panic!("{region:?} is not on the terminal: {row:?}"))
}

#[test]
fn view_reads_the_regions_the_terminal_shows_in_layout_order() {
    const TEST: &str = "view_reads_the_regions_the_terminal_shows_in_layout_order";
    isolated(TEST, async |directory| {
        let world = started(directory).await;
        let frame = world.terminal.writes().concat();
        let header = position(&world, &frame, Region::Header);
        let editor = position(&world, &frame, Region::Editor);
        let footer = position(&world, &frame, Region::Footer);
        assert!(header < editor, "the header is above the editor");
        assert!(editor < footer, "the footer is below the editor");
    });
}

/// Shows a one-line widget under `key` at `placement`.
fn show_widget(world: &World, key: &str, placement: WidgetPlacement) {
    let context = world.host.session().extension_runner().create_context();
    let options = ExtensionWidgetOptions {
        placement: Some(placement),
    };
    let lines = WidgetContent::Lines(vec![key.to_owned()]);
    let ui = context.ui().unwrap();
    ui.set_widget(key.to_owned(), Some(lines), Some(options))
        .unwrap();
}

/// Asserts `marker` is shown by `owner` and by no other region.
fn assert_only_in(world: &World, owner: Region, marker: &str) {
    for region in REGIONS {
        let shown = world.region(region).join("\n").contains(marker);
        assert_eq!(shown, region == owner, "{marker} in {region:?}");
    }
}

#[test]
fn view_keeps_each_region_and_the_editor_text_apart() {
    const TEST: &str = "view_keeps_each_region_and_the_editor_text_apart";
    isolated(TEST, async |directory| {
        let world = started(directory).await;
        show_widget(&world, "WIDGET_ABOVE", WidgetPlacement::AboveEditor);
        show_widget(&world, "WIDGET_BELOW", WidgetPlacement::BelowEditor);
        world.type_text("DRAFT_TEXT");
        assert_eq!(world.mode.view().editor_text(), "DRAFT_TEXT");
        world
            .held("PROMPT_TEXT", async |_| {
                world.host.session().steer("QUEUED_TEXT", &[]).unwrap();
                let marked = [
                    (Region::Chat, "PROMPT_TEXT"),
                    (Region::PendingMessages, "QUEUED_TEXT"),
                    (Region::WidgetsAbove, "WIDGET_ABOVE"),
                    (Region::Editor, "DRAFT_TEXT"),
                    (Region::WidgetsBelow, "WIDGET_BELOW"),
                ];
                for (owner, marker) in marked {
                    assert_only_in(&world, owner, marker);
                }
                let working = world.region(Region::Status);
                assert!(!working.is_empty(), "the working row runs while streaming");
            })
            .await;
    });
}
