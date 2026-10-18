# Present an architecture comparison

Write a uniquely named `architecture-review-<timestamp>.html` in the OS temporary
directory: `$TMPDIR` with `/tmp` fallback, or `%TEMP%` on Windows. Follow
`/skill:maestro-show-options` for offline assets, platform opening and recorded selection.
Use inline HTML/CSS/SVG rather than CDN styling, diagram scripts or a rendering engine.
Report the absolute path; keep the review artifact outside the repository.

## Compact header

Show repository, date and a small legend: solid box for module, dashed line for seam,
labelled arrow for leakage and a heavy box for a deep module. Take colours and type
from the project's tokens when available. Keep meaning legible without colour alone.

## Candidate cards

Each card has a stable anchor and contains:

- A short name for the deepening and strength: `Strong`, `Worth exploring` or `Speculative`.
- Dependency category: `in-process`, `local-substitutable`, `ports & adapters` or `mock`.
- Files and modules involved, with domain names from the glossary.
- Side-by-side before/after diagrams that show the actual friction and its proposed
  concentration behind a smaller interface.
- A sentence each for problem and solution, plus short benefits naming locality,
  leverage and the tests that improve.
- An ADR callout only when concrete friction warrants revisiting that decision.

## Draw the relationship that matters

Use labelled boxes and SVG arrows for dependencies, stacked bands for shallow call
layers, interface/implementation rectangles for relative depth, or a call tree that
collapses into one deep module. Fade internal calls in the after diagram. Keep the
legend consistent across cards. Draw a different structure when the candidate differs,
not a decorative copy of the same graph. A diagram needing a paragraph to explain
should be simplified until its relationship reads directly.

Use sparse prose and the exact architecture vocabulary: module, interface,
implementation, depth, deep, shallow, seam, adapter, leverage and locality. Keep
benefits concrete, such as 'one interface exercises all order intake paths', rather
than 'cleaner architecture'. End with a linked top recommendation and one sentence
explaining why it should go first. Let the user pick before designing the interface.
