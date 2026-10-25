# Compare structurally different UI variants

Prefer an existing populated page. Real surrounding navigation, density and data make
trade-offs visible. Keep its fetching, route parameters and authorization intact;
vary only the rendered subtree through a shareable, reload-stable `?variant=` key.
A new section can still live inside its natural host page.

Use a new throwaway route only when no existing host fits. Follow the repository's
routing convention and name it visibly as a prototype. Do not invent another top-level
structure. For logic questions use [LOGIC](LOGIC.md).

## Draft the variants

Write the question and location before coding. Default to three variants and stop at
five. They must disagree about layout, information hierarchy or primary affordance,
not only colour or copy. Keep each free to choose its structure; sharing the whole
layout would erase the comparison. Follow the project's component and styling system.

Use the same data in every variant. Read-only production data can remain read-only;
route mutations to controlled stubs rather than changing real user state. Show the
relevant state when switching. Give each variant a clear key and human-readable name.

## One bottom switcher

Use one shared floating bottom switcher for either route shape. Include previous,
current key/name and next. Previous/next wrap around and update `?variant=` through
the existing router so copying the URL or reloading preserves the selection.
Support left/right arrow keys without intercepting them in inputs, textareas or
contenteditable regions. Make the switcher visibly separate from the candidate design
and hide it in production builds through the host's existing build-mode check.

Surface the URL and variant keys. Let the user combine useful parts as well as choose
one winner; record the reason with the question. Use `/skill:maestro-show-options`
when an offline comparison page is the better question surface.

## Retain the exploration, promote the decision

Follow the [prototype lifecycle](SKILL.md). Rebuild the selected design with normal
production checks rather than shipping throwaway code directly. On an existing page,
keep the properly implemented winner and remove losers and the switcher from main.
For a new route, promote the winner to a real route and remove the throwaway route.
Retain every variant and switcher on `prototype/<name>`, linked from the implementation
issue as a primary source.
