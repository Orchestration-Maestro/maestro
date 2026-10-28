# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The browser UI, exported session viewer and OAuth completion page are web
surfaces. The terminal UI is a separate cell-based surface, not a native mobile
application. This platform value selects browser guidance for the web surfaces;
it does not make terminal widgets browser components.

## Users

Developers running model-backed coding sessions and SDK users embedding the
engine in their own applications.

## Product Purpose

Maestro runs model conversations with tools, retained sessions and replaceable
adapters. The [foundation specification](docs/specs/maestro-port.md) defines
capabilities and delivery boundaries; it is not a claim that every planned
surface is shipped.

## Operating Context

The terminal UI supports interactive sessions. The browser UI supports browser
chat. The exported viewer reads a stored session offline. The OAuth completion
page reports the result of authentication in a browser. Browser and terminal
frontends remain separate callers of the application layer.

## Capabilities and Constraints

Screen layouts and behaviour follow the established port contract.
The design system governs colour, type, the mark, glyph use, spacing where the
surface has it, and presentation of existing component states. It does not
redesign workflows or add states, controls or motion. The user's terminal font
remains under their control.

## Brand Commitments

Read [Maestro identity](docs/identity.md) for voice and visual rules,
[the brand pack](assets/brand/brand.json) for values and
[the design system](DESIGN.md) for semantic roles and component-state mappings.
These links are the authorities; this record defines no palette or font values.

## Evidence on Hand

The repository contains the terminal toolkit, shipped dark/light themes and the
OAuth completion page. The browser UI and exported viewer inventory describes
work to port, not a delivered browser application. Shared brand assets and the
theme library supply presentation data; font files are not bundled in the brand
pack.

## Accessibility & Inclusion

Follow the identity guide's [text contrast](docs/identity.md#text-contrast),
[mark and border contrast](docs/identity.md#mark-and-border-contrast) and
[glyph guidance](docs/identity.md#live-status-and-glyphs). Colour does not replace
existing text or shape status cues. Preserve terminal keybindings and browser
zoom/reflow; this record does not certify whole-interface accessibility.
