# Brand projection

`maestro-brand` owns pure brand pack admission and projection. It depends only on
[JSON boundary operations](json.md) and [text operations](text.md) within the
workspace. It performs no file reads or path joining. Native pack and template
loading remains in [the theme library](theme.md#brand-pack); callback pages use
[embedded assets](models/oauth.md).

## Admission

`PackData::parse(text)` decodes the [pack shape](identity.md#pack-shape-and-swapping)
and checks measurements. Declared fields are required and non-null; named map
entries are required only by their consuming operations. Pack and nested records
must be objects, not positional arrays. Final duplicate values are selected before
typing; unknown members are ignored after JSON parsing. Unread values still must
be valid JSON within the decoder's existing nesting limit.

Type sizes must be positive; spacing and radii must be nonnegative. Measurement
errors identify an escaped JSON Pointer. Line height, weight and mark measurements
retain the decoder's finite-number domain without additional range checks.
`BrandMark` retains authored path strings. Standalone record deserialization does
not perform whole-pack admission or measurement checks.

## Projection

`PackData::presentation(BrandMode)` borrows resolved values. It checks the selected
mode's palette targets first, then typography font references, glyph color roles,
ANSI aliases and truecolor aliases. Selected palette colors must be `#` plus six
ASCII hexadecimal digits; their case is retained. Unselected references and unused
palette strings are not resolved; their field shapes were checked during admission.
`role_color` requires the requested role in a resolved color map.

`BrandPresentation::css_properties` returns custom properties, not selectors or
layout rules: `--maestro-color-<role>`, `--maestro-font-<role>`,
`--maestro-type-<role>-font`, `-size`, `-line-height`, `-weight`,
`--maestro-spacing-<role>` and `--maestro-radii-<role>`. Size, spacing and radii use
`px`. For a publicly constructed presentation whose type refers to a missing font,
the type's font property is empty; normal pack projection rejects that reference.

`font_stack` leaves the thirteen ASCII case-insensitive CSS generic family keywords
bare in primary and fallback positions, quoting other names without trimming.
Order, repeats and spelling are retained. CSS strings escape quotes and backslashes;
C0 controls, DEL and less-than use lowercase hexadecimal escapes with a following
space, and NUL becomes the replacement character. Custom-property role names use
the same control escaping plus CSS identifier escaping. Both operations reuse
the [text owner](text.md) for CSS serialization.

`BrandPresentation::fill_mark(template)` substitutes original `{{wordmark}}`,
`var(--role)` and `currentColor` tokens once. Wordmark text is XML-escaped;
`currentColor` resolves the selected `text` role. Inserted values are not rescanned.
A reached color token with no closing parenthesis or an absent role fails.
Geometry, asset selection and reading belong to the consumer.

`BrandError` identifies decode, measurement, missing mode, missing palette color,
invalid color, missing font, missing color role and unterminated template color
failures. Its decoder variant exposes the stored native error as its cause.

## README assets

The [front-page artwork](../assets/readme/) is a static projection of the
[brand pack](../assets/brand/brand.json), following the
[identity rules](identity.md). Hero variants resolve their own canvas, text,
mark roles and font fallbacks; they load no external fonts. Badges are
self-hosted informational labels, not cached build results: the build badge
links to live checks. Icons reuse the browser icon catalog's geometry.

Refresh screenshots from real examples and retained browser components on
`main`, with no reconstructed UI or edited content. Record the source commit,
capture command, scene, terminal/brand profile, viewport and raw/optimized sizes
in the pull request. Keep each screenshot at most 400,000 bytes and total README
assets at most 3,000,000 bytes. Label local simulated replies and component
harnesses explicitly. Inspect the rendered README in both GitHub themes after
refreshing assets; changing terminal defaults does not recolor explicit demo
styles.
