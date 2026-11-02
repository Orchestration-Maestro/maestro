//! Owner-private cache observations alongside the renderer-facing witnesses.

use crate::tests::kitty_terminal::kitty_terminal as terminal;

#[test]
fn empty_generation_requires_observing_no_stored_placements() {
    let mut terminal = terminal();
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| true)
            .is_empty()
    );
    assert_eq!(terminal.kitty_projection.empty_generation, None);
    terminal.write(b"\x1b_Ga=t,f=24,i=1,s=1,v=2;////////\x1b\\");
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| true)
            .is_empty()
    );
    let generation = terminal.screens.active().kitty_images.generation;
    assert_eq!(terminal.kitty_projection.empty_generation, Some(generation));
    terminal.write(b"plain text");
    assert_eq!(
        terminal.screens.active().kitty_images.generation,
        generation
    );
    terminal.write(b"\x1b_Ga=p,i=1,p=1,c=1,r=1,C=1\x1b\\");
    let placed = terminal.screens.active().kitty_images.generation;
    assert!(placed > generation);
    assert_eq!(
        terminal
            .kitty_image_placements_with_data_filter(|_| true)
            .len(),
        1
    );
    for _ in 0..20 {
        terminal.write(b"line\r\n");
    }
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| true)
            .is_empty()
    );
    assert_ne!(terminal.kitty_projection.empty_generation, Some(placed));
    assert!(terminal.kitty_projection.fingerprints.is_empty());
}

#[test]
fn fingerprints_follow_image_generations_and_live_placement_pruning() {
    let mut terminal = terminal();
    terminal.write(b"\x1b_Ga=T,f=32,i=7,p=3,s=1,v=1,c=1,r=1,C=1;/wAA/w==\x1b\\");
    let first = terminal.kitty_image_placements_with_data_filter(|_| true);
    let original = terminal.kitty_projection.fingerprints[&7];
    assert_ne!(original.generation, 0);
    assert_eq!(
        original.generation,
        terminal.screens.active().kitty_images.images[&7].generation
    );
    terminal.write(b"\x1b_Ga=p,i=7,p=4,c=2,r=1,C=1\x1b\\");
    terminal.kitty_image_placements_with_data_filter(|_| false);
    assert_eq!(
        original.generation,
        terminal.kitty_projection.fingerprints[&7].generation
    );
    terminal.write(b"\x1b_Ga=t,f=32,i=7,s=1,v=1;AAAAAA==\x1b\\");
    let second = terminal.kitty_image_placements_with_data_filter(|_| false);
    assert_ne!(first[0].data_fingerprint, second[0].data_fingerprint);
    assert!(terminal.kitty_projection.fingerprints[&7].generation > original.generation);
    terminal.write(b"\x1b_Ga=d,d=a\x1b\\");
    assert!(
        terminal
            .kitty_image_placements_with_data_filter(|_| true)
            .is_empty()
    );
    assert!(terminal.kitty_projection.fingerprints.is_empty());
    assert!(
        terminal
            .screens
            .active()
            .kitty_images
            .images
            .contains_key(&7)
    );
}

#[test]
fn generation_stamps_never_recur_across_screens_and_reset_projection() {
    let mut terminal = terminal();
    let upload = b"\x1b_Ga=T,f=32,i=1,p=1,s=1,v=1,c=1,r=1,C=1;/wAA/w==\x1b\\";
    terminal.write(upload);
    terminal.kitty_image_placements_with_data_filter(|_| false);
    let main = terminal.kitty_projection.fingerprints[&1].generation;
    let main_storage = terminal.screens.active().kitty_images.generation;
    terminal.write(b"\x1b[?1049h");
    assert_eq!(terminal.screens.active().kitty_images.generation, 0);
    terminal.write(upload);
    terminal.kitty_image_placements_with_data_filter(|_| false);
    let alternate = terminal.kitty_projection.fingerprints[&1].generation;
    assert!(alternate > main_storage);
    terminal.write(b"\x1b[?1049l");
    terminal.kitty_image_placements_with_data_filter(|_| false);
    assert_eq!(terminal.kitty_projection.fingerprints[&1].generation, main);
    terminal.write(b"\x1bc");
    terminal.write(upload);
    terminal.kitty_image_placements_with_data_filter(|_| false);
    assert!(terminal.kitty_projection.fingerprints[&1].generation > alternate);
}
