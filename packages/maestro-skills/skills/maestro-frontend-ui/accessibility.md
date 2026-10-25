# Accessibility checklist

Use native controls before custom widgets. Verify WCAG 2.1 AA with automated inspection plus keyboard and
screen-reader use; an automated pass alone cannot establish usability.

## Keyboard and focus

- Every action is reachable by keyboard. Use a button for an action and a link for navigation.
- Keep logical tab order and visible focus; avoid positive `tabindex`. Use zero for ordinary participation
  and minus one for programmatic focus where appropriate.
- Support the native Enter/Space activation semantics and Escape closing for custom widgets. Prefer a
  native select to a custom dropdown without a complete keyboard/ARIA implementation.
- Provide a skip-to-content link visible on focus. Avoid keyboard traps outside modal interaction.
- On opening a modal, move focus inside, keep it there while open and return it to the invoker on close.
  Manage focus when dynamic content changes; test both opening and closing.

## Names, structure and announcements

- Supply meaningful image alternatives; decorative images use empty alt text.
- Associate visible labels with form controls. Name icon-only controls accessibly; links describe destinations.
- Give the page a descriptive title and language. Use one main heading and ordered heading levels.
- Use table headers with scope and semantic lists. Label multiple navigation landmarks distinctly.
- Announce status with a polite live region or `role=status`; reserve assertive announcements or
  `role=alert` for urgent errors. Mark loading regions busy. Avoid announcing every incidental change.

## Visual and interaction checks

- Normal text contrast is at least 4.5:1; large text and UI components need at least 3:1.
  Large text means at least 18 pt, or 14 pt when bold.
- Convey state with text, icons or patterns as well as colour. Do not remove focus outlines without a replacement.
- Resize text to 200% without lost content or broken controls. Avoid flashing more than three times a second.
- Make mobile touch targets at least 44 by 44 pixels. Provide media controls rather than uncontrolled autoplay.
- Empty states explain the absence and next action; blank screens are not a useful state.

## Forms

Mark required fields beyond colour. Use appropriate type and autocomplete for known fields. Explain errors
specifically, associate them with the field and provide a focusable submission-error summary. Preserve entered
values when retrying. Do not use placeholders as the only label.

## Verify

Tab through the whole flow, operate controls without a mouse, close/reopen dialogs and check focus restoration.
Inspect the accessibility tree and use the platform screen reader. Run the project's accessibility audit against
the actual page. Check small viewports, zoom, dynamic announcements, contrast and meaningful error states.
Record what was exercised and what still needs manual verification rather than equating zero audit findings
with full accessibility.
