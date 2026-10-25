---
name: maestro-show-options
description: "Use when a decision needs concrete alternatives, especially differences the user can see."
---

# Choose the decision surface per question

Use the browser when alternatives differ in visible layout or appearance. Use the
terminal for conceptual choices whose important differences are textual. Reassess
for each question; one browser choice does not move the entire session into it.
Read project design tokens and identity guidance before rendering project surfaces.

## One offline page

Write one self-contained HTML file with every option side by side and a selector.
Label the question, options, relevant trade-offs and recommendation. Use the project's
design tokens, fonts and mark when available; embed required local assets so opening
the file requires no network. Keep CSS and any selector script inline. Use local
HTML/CSS/SVG for diagrams. No CDN, server, live reload, click log or remote-access
promise belongs to this path.

Keep the comparison fair: equal fidelity, real differences, readable labels and the
states relevant to the decision. A UI state question may use `/skill:maestro-prototype`
for runnable variants. A non-trivial comparison page gets the existing
`/skill:maestro-verify` doubt pass before presentation.

## Obtain and record the choice

Open the file through the shell tool with `open` on macOS, `explorer.exe` on Windows
or `xdg-open` elsewhere. Print its path. If SSH, a container or a headless environment
prevents browser access, leave the artifact available and ask in the terminal.
A selector can help comparison; it does not replace the user's decision.

Use `ask_user_question` to obtain the choice, with one call for the interview round.
Record the answer, reason and artifact pointer in the owning decision document.
Do not infer acceptance from a page opening, a hover or an unrecorded click.
