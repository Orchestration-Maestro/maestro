# Generic artifact views

`maestro_web::tools::artifacts::create_artifact_element` constructs an owned
`Box<dyn maestro_web::ArtifactElement>` for an unrecognized filename suffix.
Recognized specialized formats return a presentation error until their viewers
are installed; they never silently become generic downloads.

The element exposes literal filename/content getters and setters, independent
`get_header_buttons` and `render` operations, and call-scoped `ArtifactView`
localization and brand operands. Retain the element across frames. Setters request
repaint after drawing; they do not retain borrowed rendering inputs.

The generic body is selectable native text in the supplied bounded viewport:
a file glyph, literal filename and localized no-preview guidance. Body drawing
never decodes content. The existing attachment registry owns placement and native
teardown; edits keep the same published factory/root and update only changed text
nodes, preserving unrelated selection and scroll. Dropping the element removes
its own surface.

The independent header decodes the original base64 before drawing its download
control. Standard alphabet, optional valid padding, nonzero unused trailing bits
and ASCII TAB/LF/FF/CR/SPACE are admitted; malformed padding, URL-safe alphabet and
other whitespace fail. A case-sensitive `data:` prefix with a nonempty suffix
after its first `base64,` marker selects the complete suffix, including line
breaks. No text fallback is used. Reached save failures propagate without replacing
the body. MIME selection uses the last literal dot-delimited segment with Unicode
lowercase, not path normalization or a MIME database. The native byte operation
preserves the literal name and selected bytes.

`GenericArtifact` is also available through its module for direct construction.
Its initial filename and content are empty. Caller-supplied packs and localization
apply on each draw; there is no global theme or new runtime.

The [local application caller](../application/artifacts.md) can retain a supplied
presentation adapter that owns these views. This library does not install an
incomplete application presentation implementation.

The tabbed/overlay shell and retained visibility remain in
[#1108](https://github.com/Orchestration-Maestro/maestro/issues/1108).
Specialized viewers remain in
[#795](https://github.com/Orchestration-Maestro/maestro/issues/795),
[#797](https://github.com/Orchestration-Maestro/maestro/issues/797),
[#798](https://github.com/Orchestration-Maestro/maestro/issues/798),
[#816](https://github.com/Orchestration-Maestro/maestro/issues/816),
[#818](https://github.com/Orchestration-Maestro/maestro/issues/818) and
[#819](https://github.com/Orchestration-Maestro/maestro/issues/819).
HTML runtime-provider composition remains in
[#753](https://github.com/Orchestration-Maestro/maestro/issues/753).
