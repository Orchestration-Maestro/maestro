---
name: maestro-prototype
description: "Use when a design question needs runnable logic or visible interface variants before commitment."
---

# Answer one design question with throwaway code

Choose [logic/state](LOGIC.md) when the uncertainty is transitions, rules or data shape;
choose [visible UI](UI.md) when the uncertainty is what the user should see. Ask when
ambiguous. If the user is absent, choose from the surrounding context and state the
assumption visibly. Use `/skill:maestro-git-workflow` before creating a branch.

Keep the prototype visibly throwaway, close to the module or page it explores and
trivial to launch. Use in-memory state unless persistence itself is the question;
then use an explicitly disposable store. Skip production abstractions, polish and
a test suite for the throwaway shell. Keep enough handling to make it runnable.
Render the relevant state after each action or variant switch.

Use `/skill:maestro-show-options` for visible choices. After selection, capture the
question, verdict and rationale on the implementation issue. Retain the prototype
and its alternatives on `prototype/<name>` as a linked primary source, outside main.
Main keeps the validated decision and a properly implemented winner, not the discarded
shell. Follow repository permission rules when committing or publishing that branch.
