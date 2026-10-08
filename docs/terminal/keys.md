# Terminal keys

`maestro_tui::keys` turns the raw text a terminal sends into key identifiers, typed
characters and key events. It reads no device: a caller passes each chunk of input it
receives and learns what the chunk means. Its only state is one protocol flag, and the
backspace rule under legacy ambiguities consults the terminal environment variables
named there. The crate root re-exports everything below except `decode_printable_key`.

## Key identifiers

A key identifier names one key and the modifiers held with it: `ctrl+c`, `shift+tab`,
`alt+enter`, `escape`, `f5`. The modifiers are `shift`, `ctrl`, `alt` and `super`, joined
by `+` before the key in any order and in any ASCII case. The keys are:

- `escape` (also `esc`), `enter` (also `return`), `tab`, `space`, `backspace`;
- `delete`, `insert`, `clear`, `home`, `end`, `pageUp`, `pageDown`;
- `up`, `down`, `left`, `right`, and `f1` to `f12`;
- the letters `a` to `z` and the digits `0` to `9`;
- these symbols: ``` ` - = [ ] \ ; ' , . / ! @ # $ % ^ & * ( ) _ + | ~ { } : < > ? ```.

A trailing `+` is the plus key, so `+` and `ctrl++` name plus without and with ctrl.
Words before the key that are not modifiers are ignored. Any other text names no key
and never matches. `KeyId` is a `String`; `Key` spells the supported identifiers as
constants (`Key::ESCAPE`, `Key::PAGE_UP`, `Key::SLASH`) and builds modified ones in the
order written (`Key::ctrl("c")`, `Key::shift_ctrl("p")`, `Key::super_key("k")`).

```rust
use maestro_tui::Key;

assert_eq!(Key::ctrl("c"), "ctrl+c");
assert_eq!(Key::shift_ctrl(Key::PIPE), "shift+ctrl+|");
```

## Matching and parsing

`matches_key(data, id)` is true when `data` is the key `id` held with exactly the
modifiers `id` names. One key arrives in several encodings and each is recognized:

- legacy bytes and sequences: control bytes for ctrl and a letter or symbol, `ESC`
  before a key for alt, and the CSI, SS3, double-bracket and rxvt forms of the
  navigation and function keys;
- the enhanced protocol: `CSI code[:shifted[:base]] [; modifier[:event]] u`, arrow and
  home/end reports `CSI 1 ; modifier[:event] A` (also `B`, `C`, `D`, `H`, `F`), and
  tilde reports `CSI number [; modifier[:event]] ~`;
- the xterm modifyOtherKeys form `CSI 27 ; modifier ; code ~`.

`parse_key(data)` names the key an input reports, with modifiers in the order shift,
ctrl, alt, super, or returns `None`.

```rust
use maestro_tui::{Key, matches_key, parse_key};

assert!(matches_key("\x03", "ctrl+c"));
assert!(matches_key("\x1b[99;5u", &Key::ctrl("c")));
assert!(matches_key("\x1b[27;5;99~", "ctrl+c"));
assert!(!matches_key("\x1b[99;5u", "ctrl+shift+c"));
assert_eq!(parse_key("\x1b[1;5A").as_deref(), Some("ctrl+up"));
assert_eq!(parse_key("\x1b[107;14u").as_deref(), Some("shift+ctrl+super+k"));
assert_eq!(parse_key("\x1b[99;17u"), None);
```

The enhanced forms follow these rules.

- The modifier field is one more than the bits shift (1), alt (2), ctrl (4) and super (8).
  Caps Lock (64) and Num Lock (128) are ignored. Any other bit leaves the key
  unnamed, and a modifier field of zero is not a report.
- A letter, digit or symbol names itself. The base-layout key identifies only keys that
  are none of these, so Ctrl+С on a Cyrillic layout, reported with base key `c`, is
  `ctrl+c`, while a Dvorak Ctrl+K reported with base key `v` stays `ctrl+k`.
- A shifted capital letter is `shift+` the lowercase letter.
- Keypad codes 57399 to 57426 name the digit, symbol or navigation key they stand for,
  except Numpad Enter (57414), which is `enter`.
- A release report matches the key it releases. A caller that wants presses only
  checks `is_key_release` first.

```rust
use maestro_tui::{is_key_release, matches_key, parse_key};

let release = "\x1b[1089::99;5:3u";
assert!(matches_key(release, "ctrl+c"));
assert!(is_key_release(release));
assert_eq!(parse_key("\x1b[57400u").as_deref(), Some("1"));
```

### Numbers

Fields hold ASCII decimal digits and may have leading zeros. Text with extra, missing or
misplaced separators, signs, non-ASCII digits, or anything before or after it is not a
report at all. A report whose code point is a surrogate or beyond Unicode, whose number
does not fit in 32 bits, or whose modifier field is zero names no key, matches nothing
and types nothing; it is never wrapped onto another key. The event functions still read
its event field.

### Legacy ambiguities

Some bytes mean more than one key, and the decoder keeps them apart only where the
terminal does.

- Byte 8 is backspace and also `ctrl+h`. In a local Windows Terminal, meaning
  `WT_SESSION` is set and not empty while `SSH_CONNECTION`, `SSH_CLIENT` and `SSH_TTY`
  are all unset or empty, it is `ctrl+backspace`. Byte 127 is always `backspace`.
- Linefeed is `enter`. While the enhanced protocol is active, linefeed and `ESC` plus
  carriage return are `shift+enter` instead; otherwise `ESC` plus carriage return is
  `alt+enter`.
- Alt with a letter, digit or space and ctrl with alt are `ESC` before the key, but only
  while the enhanced protocol is inactive. `ESC` before `b`, `f`, `n` or `p` is alt with
  left, right, down or up in both states.
- Control bytes name the lowercase letter, and byte 31 is `ctrl+-` (also `ctrl+_`).
  Raw uppercase letters name themselves, and a raw capital matches `shift+` the letter.
- Numeric home and end (`CSI 1 ~`, `CSI 4 ~`) are `home` and `end`; modified forms of
  them are not recognized.
- A plain `tab` or `enter` in the modifyOtherKeys form is not matched, though it parses.

## Typed text

`decode_kitty_printable` returns the character an enhanced `CSI u` report types: the
shifted code point when shift is held and the report carries one, otherwise the code
point, with keypad codes mapped to the digit or symbol they type. It returns `None`
when alt, ctrl, super or any other modifier is held, for control characters (`C0`,
`DEL` and `C1`), for navigation keypad codes and for the protocol's reserved functional
range `U+E000` to `U+F8FF`. `decode_printable_key` also accepts the modifyOtherKeys
form, which keeps private-use characters as text. A release or repeat report types its
character too; the caller decides whether to insert it.

```rust
use maestro_tui::decode_kitty_printable;
use maestro_tui::keys::decode_printable_key;

assert_eq!(decode_kitty_printable("\x1b[97:65;2u"), Some('A'));
assert_eq!(decode_kitty_printable("\x1b[57400u"), Some('1'));
assert_eq!(decode_kitty_printable("\x1b[97;5u"), None);
assert_eq!(decode_printable_key("\x1b[27;2;69~"), Some('E'));
```

## Events

`is_key_release` and `is_key_repeat` are true only when the whole input is one complete
enhanced report whose event field is 3 (release) or 2 (repeat). Any other event number,
leading zeros apart, is a press. Alternate-key fields are not event fields, ordinary
text such as `x:3u` is not a report, and an input containing the bracketed-paste start
`ESC [ 200 ~` is never an event. A report whose key has no name is still an event.
`KeyEventType` names the three phases.

```rust
use maestro_tui::{is_key_release, is_key_repeat};

assert!(is_key_release("\x1b[99;1:3u"));
assert!(is_key_repeat("\x1b[1;1:2A"));
assert!(!is_key_release("\x1b[99:3u"));
assert!(!is_key_release("x:3u"));
```

## Protocol state

When a terminal runs the enhanced protocol, some legacy inputs change meaning, so the
decoder reads one process-wide flag. `set_kitty_protocol_active` sets it for whatever
detects that the terminal runs the protocol, and `is_kitty_protocol_active` reads it.
The flag starts out `false` and every operation reads it afresh.

```rust
use maestro_tui::{is_kitty_protocol_active, matches_key, set_kitty_protocol_active};

assert!(!is_kitty_protocol_active());
assert!(matches_key("\n", "enter"));
set_kitty_protocol_active(true);
assert!(matches_key("\n", "shift+enter"));
assert!(!matches_key("\n", "enter"));
set_kitty_protocol_active(false);
```
