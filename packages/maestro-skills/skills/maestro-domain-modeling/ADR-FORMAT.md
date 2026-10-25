# Record a significant decision

Follow the repository's existing ADR convention first. Otherwise use sequential files in `docs/adr/`, creating
the directory only for the first decision. Inspect existing numbers before choosing the next one.

Use a short decision title and a paragraph giving context, the decision and why it was chosen.
Add alternatives, consequences and status only where they explain real trade-offs or future surprises.
Status may be proposed, accepted, deprecated or superseded with a link to the replacement.

An ADR is justified only when the decision is hard to reverse, surprising without context and the result of a
real trade-off. Keep records short when a paragraph suffices; sections are not a quota.
When the decision changes, preserve the old record and mark it superseded. Never erase the reason it was once right.
