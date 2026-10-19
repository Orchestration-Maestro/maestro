//! Direct metadata retains raw bytes without interpreting directory values.

use maestro_terminal::Terminal;

#[test]
fn editing_set_title_accepts_its_current_value() {
    let mut t = Terminal::new(5, 1, 0).unwrap();
    t.set_title(b"Maestro");
    let current = t.get_title().unwrap().to_vec();
    t.set_title(&current);
    assert_eq!(t.get_title(), Some(b"Maestro".as_slice()));
}

#[test]
fn editing_set_and_get_pwd() {
    let mut t = Terminal::new(80, 24, 0).unwrap();
    assert_eq!(t.get_pwd(), None);
    t.set_pwd(b"file:///home/user");
    assert_eq!(t.get_pwd(), Some(b"file:///home/user".as_slice()));
    t.set_pwd(b"file:///other");
    assert_eq!(t.get_pwd(), Some(b"file:///other".as_slice()));
    t.set_pwd(b"");
    assert_eq!(t.get_pwd(), None);
}

#[test]
fn editing_set_pwd_accepts_its_current_value() {
    let mut t = Terminal::new(5, 1, 0).unwrap();
    t.set_pwd(b"file:///tmp");
    let current = t.get_pwd().unwrap().to_vec();
    t.set_pwd(&current);
    assert_eq!(t.get_pwd(), Some(b"file:///tmp".as_slice()));
}

#[test]
fn direct_metadata_setters_keep_arbitrary_owned_bytes() {
    let mut t = Terminal::new(5, 1, 0).unwrap();
    for len in [1023, 1024, 1025] {
        let mut value = vec![b't'; len];
        value[..2].copy_from_slice(&[0xff, 0]);
        t.set_title(&value);
        assert_eq!(t.get_title(), Some(value.as_slice()));
        let current = t.get_title().unwrap().to_vec();
        t.set_title(&current);
        assert_eq!(t.get_title(), Some(value.as_slice()));
    }
    for len in [4095, 4096, 4097] {
        let mut value = vec![b'd'; len];
        value[..2].copy_from_slice(&[0xff, 0]);
        t.set_pwd(&value);
        assert_eq!(t.get_pwd(), Some(value.as_slice()));
        let current = t.get_pwd().unwrap().to_vec();
        t.set_pwd(&current);
        assert_eq!(t.get_pwd(), Some(value.as_slice()));
    }
    t.set_pwd(b"file:///work%20space");
    assert_eq!(t.get_pwd(), Some(b"file:///work%20space".as_slice()));
}
