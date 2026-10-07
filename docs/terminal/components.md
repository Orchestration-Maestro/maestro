# Components

All components implement:

```rust
use maestro_tui::Component;
struct Status;
impl Component for Status {
    fn render(&mut self, _: usize) -> Vec<String> { vec!["ready".into()] }
    fn invalidate(&mut self) {}
}
assert_eq!(Status.render(10), ["ready"]);
```

## Container

Groups child components.

```rust
use std::{cell::RefCell, rc::Rc};
use maestro_tui::{Component, Container, TruncatedText};
let mut container = Container::new();
container.add_child(Rc::new(RefCell::new(
    TruncatedText::new("ready".into(), Some(1), None))));
assert_eq!(container.render(8), [" ready  "]);
container.invalidate();
container.clear();
assert!(container.render(8).is_empty());
```

## TruncatedText

Single-line text that truncates to fit viewport width. Useful for status lines and headers.

```rust
use maestro_tui::{Component, TruncatedText};
let mut text = TruncatedText::new("Hello world\nignored".into(), None, None);
assert_eq!(text.render(8), ["Hello\x1b[0m...\x1b[0m"]);
```

Only the first LF-delimited line is displayed. Absent padding defaults to zero;
vertical padding is symmetric. Horizontal padding is bounded by the viewport,
with left padding allocated before right padding. Content may have zero available columns.
Padding counts are unsigned; insufficient room uses saturating subtraction.
Replace retained text by replacing the component; there is no setter or native
terminal requirement.
