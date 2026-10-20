//! Deletion traversal cost through retained page observations.

use crate::{Terminal, kitty::graphics::command::Delete, point::Coordinate};

#[test]
fn nonspatial_deletion_never_reads_placement_pages() {
    for lines in [1_000, 2_000] {
        for selector in [
            Delete::All(true),
            Delete::Id {
                delete: true,
                image_id: 1,
                placement_id: 1,
            },
            Delete::Newest {
                delete: true,
                image_number: 9,
                placement_id: 1,
            },
            Delete::Z { delete: true, z: 3 },
            Delete::Range {
                delete: true,
                first: 1,
                last: 1,
            },
            Delete::AnimationFrames(true),
        ] {
            let mut terminal = Terminal::new(5, 5, 100_000_000).unwrap();
            terminal.enable_kitty_graphics();
            terminal.write(b"\x1b_Ga=T,f=24,i=1,I=0,s=1,v=1,p=1,c=1,r=100000,z=3,C=1;AAAA\x1b\\");
            terminal
                .screens
                .active_mut()
                .kitty_images
                .images
                .get_mut(&1)
                .unwrap()
                .number = 9;
            terminal.write("x\r\n".repeat(lines).as_bytes());
            let screen = terminal.screens.active_mut();
            let image = screen.kitty_images.image_by_id(1).unwrap();
            let placement = screen.kitty_images.placements.values().next().unwrap();
            screen.pages.page_visits.set(0);
            assert!(placement.rect(image, &screen.pages, (10, 20)).is_some());
            assert!(
                screen.pages.page_visits.get() > 0,
                "spatial control must read pages"
            );
            screen.pages.page_visits.set(0);
            screen.kitty_images.delete(
                &mut screen.pages,
                (10, 20),
                Coordinate::default(),
                selector,
            );
            assert_eq!(
                screen.pages.page_visits.get(),
                0,
                "{selector:?}, {lines} retained rows"
            );
            assert_eq!(
                screen.kitty_images.placements.len(),
                usize::from(matches!(selector, Delete::AnimationFrames(_)))
            );
        }
    }
}
