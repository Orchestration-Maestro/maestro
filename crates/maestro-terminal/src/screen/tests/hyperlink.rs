use super::{page, pin, screen, split_rows, write_row};
use crate::hyperlink::HyperlinkId;

#[test]
fn screen_hyperlink_start_end() {
    let mut s = screen(5, 5);
    assert_eq!(s.cursor.hyperlink_id, 0);
    assert_eq!(page(&s).hyperlink_set.count(), 0);
    s.start_hyperlink(b"http://example.com", None).unwrap();
    assert_ne!(s.cursor.hyperlink_id, 0);
    assert_eq!(page(&s).hyperlink_set.count(), 1);
    s.end_hyperlink();
    assert_eq!(s.cursor.hyperlink_id, 0);
    assert_eq!(page(&s).hyperlink_set.count(), 0);
    s.end_hyperlink();
    assert!(s.cursor.hyperlink.is_none());
}

#[test]
fn screen_hyperlink_accepts_its_current_values() {
    let mut s = screen(5, 5);
    s.start_hyperlink(b"http://example.com", Some(b"current"))
        .unwrap();
    // Rust borrows require an owned snapshot before mutating the cursor.
    let current = s.cursor.hyperlink.clone().unwrap();
    let HyperlinkId::Explicit(id) = &current.id else {
        panic!("expected explicit identity");
    };
    s.start_hyperlink(&current.uri, Some(id)).unwrap();
    assert_eq!(
        s.cursor.hyperlink.as_ref().unwrap().uri,
        b"http://example.com"
    );
    assert_eq!(
        s.cursor.hyperlink.as_ref().unwrap().id,
        HyperlinkId::Explicit(b"current".to_vec())
    );
}

#[test]
fn screen_implicit_hyperlink_id_wraps() {
    let mut s = screen(5, 5);
    s.cursor.hyperlink_implicit_id = u32::MAX;
    s.start_hyperlink(b"http://example.com", None).unwrap();
    assert_eq!(s.cursor.hyperlink_implicit_id, 0);
    assert_eq!(
        s.cursor.hyperlink.as_ref().unwrap().id,
        HyperlinkId::Implicit(u32::MAX)
    );
    s.start_hyperlink(b"http://example.com", None).unwrap();
    assert_eq!(
        s.cursor.hyperlink.as_ref().unwrap().id,
        HyperlinkId::Implicit(0)
    );
    assert_eq!(s.cursor.hyperlink_implicit_id, 1);
}

#[test]
fn screen_hyperlink_reuses_live_values_without_id_assumptions() {
    let mut s = screen(5, 5);
    assert_eq!(page(&s).hyperlink_set.count(), 0);
    s.start_hyperlink(b"http://example.com", Some(b"shared"))
        .unwrap();
    write_row(&mut s, "A");
    let retained = s.cursor.hyperlink_id;
    s.start_hyperlink(b"http://example.com", Some(b"shared"))
        .unwrap();
    assert_eq!(s.cursor.hyperlink_id, retained);
    assert_eq!(page(&s).hyperlink_set.count(), 1);
    assert_eq!(page(&s).hyperlink_set.ref_count(retained), 2);
    s.end_hyperlink();
    assert_eq!(s.cursor.hyperlink_id, 0);
    assert_eq!(page(&s).hyperlink_set.ref_count(retained), 1);
    assert_eq!(
        pin(&s).hyperlink_uri(&s.pages).unwrap(),
        b"http://example.com"
    );
}

#[test]
fn screen_cursor_copy_hyperlink_deref() {
    let source = screen(10, 10);
    let mut target = screen(10, 10);
    target
        .start_hyperlink(b"https://example.test", None)
        .unwrap();
    assert_eq!(page(&target).hyperlink_set.count(), 1);
    target.cursor_copy(&source.cursor, true).unwrap();
    assert_eq!(page(&target).hyperlink_set.count(), 0);
    assert_eq!(target.cursor.hyperlink_id, 0);
}

#[test]
fn screen_cursor_copy_hyperlink_deref_new_page() {
    let source = screen(10, 10);
    let mut target = screen(10, 10);
    let (first, second) = split_rows(&mut target, 5);
    target.cursor_absolute(2, 9);
    assert_eq!(pin(&target).node, second);
    target
        .start_hyperlink(b"https://example.test", None)
        .unwrap();
    assert_eq!(page(&target).hyperlink_set.count(), 1);
    target.cursor_copy(&source.cursor, true).unwrap();
    assert_eq!(target.pages.page(second).unwrap().hyperlink_set.count(), 0);
    assert_eq!(pin(&target).node, first);
    assert_eq!((target.cursor.x, target.cursor.y), (0, 0));
    assert_eq!(target.cursor.hyperlink_id, 0);
}

#[test]
fn screen_cursor_copy_hyperlink_copy() {
    let mut source = screen(10, 10);
    source
        .start_hyperlink(b"https://example.test", Some(b"id"))
        .unwrap();
    let mut target = screen(10, 10);
    target.cursor_copy(&source.cursor, true).unwrap();
    assert_eq!(target.cursor.hyperlink, source.cursor.hyperlink);
    assert_eq!(page(&target).hyperlink_set.count(), 1);
    assert_eq!(
        page(&target)
            .hyperlink_set
            .get(target.cursor.hyperlink_id)
            .uri,
        b"https://example.test"
    );
    source.end_hyperlink();
    assert_eq!(
        page(&target)
            .hyperlink_set
            .ref_count(target.cursor.hyperlink_id),
        1
    );
}

#[test]
fn screen_cursor_copy_hyperlink_copy_disabled() {
    let mut source = screen(10, 10);
    source
        .start_hyperlink(b"https://example.test", None)
        .unwrap();
    let mut target = screen(10, 10);
    target.start_hyperlink(b"https://old.test", None).unwrap();
    target.cursor_copy(&source.cursor, false).unwrap();
    assert_eq!(target.cursor.hyperlink_id, 0);
    assert!(target.cursor.hyperlink.is_none());
    assert_eq!(page(&target).hyperlink_set.count(), 0);
    assert_eq!(page(&source).hyperlink_set.count(), 1);
}

#[test]
fn screen_cursor_set_hyperlink_long_uri_after_hyperlink_map_exhaustion() {
    let mut s = screen(160, 2);
    let uri = vec![b'u'; 2040];
    s.start_hyperlink(&uri, None).unwrap();
    let before = page(&s).capacity.hyperlink_bytes;
    let count = page(&s).hyperlink_capacity() + 1;
    assert!(count <= 320);
    for n in 0..count {
        s.cursor_absolute(
            u16::try_from(n % 160).unwrap(),
            u16::try_from(n / 160).unwrap(),
        );
        s.cursor_set_hyperlink().unwrap();
        assert_eq!(pin(&s).hyperlink_uri(&s.pages).unwrap(), uri);
    }
    assert!(page(&s).capacity.hyperlink_bytes > before);
    assert_eq!(page(&s).hyperlink_count(), count);
    assert_eq!(s.cursor.hyperlink.as_ref().unwrap().uri, uri);
    assert_eq!(
        page(&s).hyperlink_set.ref_count(s.cursor.hyperlink_id),
        u16::try_from(count + 1).unwrap()
    );
}

#[test]
fn screen_hyperlink_admits_uri_larger_than_initial_string_storage() {
    let mut s = screen(160, 2);
    let uri = vec![b'u'; 4000];
    s.start_hyperlink(&uri, Some(b"long")).unwrap();
    assert_eq!(s.cursor.hyperlink.as_ref().unwrap().uri, uri);
    s.cursor_set_hyperlink().unwrap();
    assert_eq!(pin(&s).hyperlink_uri(&s.pages).unwrap(), uri);
}

#[test]
fn screen_cross_page_style_growth_retains_exactly_one_cursor_link() {
    for grow in [false, true] {
        for explicit in [false, true] {
            check_cross_page_style_growth(grow, explicit.then_some(b"cursor".as_slice()));
        }
    }
}

fn check_cross_page_style_growth(grow: bool, id: Option<&[u8]>) {
    let explicit = id.is_some();
    let mut s = screen(10, 3);
    let (first, second) = split_rows(&mut s, 2);
    if grow {
        super::fill_styles(s.pages.page_mut(second).unwrap());
    }
    s.cursor_absolute(0, 1);
    s.set_attribute(crate::sgr::Attribute::Bold).unwrap();
    s.start_hyperlink(b"https://example.test", id).unwrap();
    s.cursor_down(1);
    assert_eq!(s.cursor.y, 2);
    assert_eq!(pin(&s).node != second, grow);
    assert_eq!(page(&s).hyperlink_set.ref_count(s.cursor.hyperlink_id), 1);
    assert_eq!(s.pages.page(first).unwrap().hyperlink_set.count(), 0);
    let expected = id.map_or(HyperlinkId::Implicit(1), |id| {
        HyperlinkId::Explicit(id.to_vec())
    });
    assert_eq!(s.cursor.hyperlink.as_ref().unwrap().id, expected);
    assert_eq!(s.cursor.hyperlink_implicit_id, if explicit { 0 } else { 2 });
    let target = pin(&s).node;
    s.cursor_up(1);
    assert_eq!(s.pages.page(target).unwrap().hyperlink_set.count(), 0);
    assert_eq!(page(&s).hyperlink_set.ref_count(s.cursor.hyperlink_id), 1);
    if !explicit {
        assert_eq!(
            s.cursor.hyperlink.as_ref().unwrap().id,
            HyperlinkId::Implicit(2)
        );
        assert_eq!(s.cursor.hyperlink_implicit_id, 3);
    }
    s.end_hyperlink();
    assert_eq!(page(&s).hyperlink_set.count(), 0);
}

#[test]
fn screen_hyperlink_cursor_state_on_resize() {
    for explicit in [false, true] {
        let mut s = screen(5, 10);
        s.no_scrollback = true;
        s.set_attribute(crate::sgr::Attribute::Bold).unwrap();
        s.start_hyperlink(b"http://example.com", explicit.then_some(b"id".as_slice()))
            .unwrap();
        assert_ne!(s.cursor.hyperlink_id, 0);
        assert_eq!(page(&s).hyperlink_set.count(), 1);
        s.resize(super::super::Resize {
            cols: std::num::NonZeroU16::new(10).unwrap(),
            rows: std::num::NonZeroU16::new(10).unwrap(),
            reflow: true,
            prompt_redraw: crate::osc::semantic_prompt::Redraw::False,
        })
        .unwrap();
        assert_ne!(s.cursor.hyperlink_id, 0);
        assert_eq!(page(&s).hyperlink_set.count(), 1);
        assert_eq!(page(&s).hyperlink_set.ref_count(s.cursor.hyperlink_id), 1);
        assert_eq!(
            s.cursor.hyperlink.as_ref().unwrap().uri,
            b"http://example.com"
        );
        assert_eq!(page(&s).styles.ref_count(s.cursor.style_id), 1);
        s.end_hyperlink();
        assert_eq!(s.cursor.hyperlink_id, 0);
        assert_eq!(page(&s).hyperlink_set.count(), 0);
    }
}
