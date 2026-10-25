# Let the user drive the state model

Build one self-contained HTML file for a business-rule, transition or data-shape question.
If the uncertainty is appearance, use [UI](UI.md) instead. Follow the
[prototype lifecycle](SKILL.md) and option-page rules from `/skill:maestro-show-options`.

## Separate logic from the shell

State the question visibly at the top. Keep the logic in a pure, liftable module:
a reducer for discrete actions, an explicit state machine for legal transitions,
functions for transformations, or an owned-state module when the question requires it.
The DOM calls the module; the logic never reaches into DOM or button handlers.
The shell is disposable even if the validated logic is worth keeping.

## Page structure

1. Title and a one-line explanation of the question.
2. Full relevant state in readable labelled fields, updated after every action.
   Call out changed fields when that helps a non-developer understand the transition.
3. Free-play buttons, one per action, usable in any order.
4. Scenario tabs with a situation, what to watch for and actual step buttons.
   Starting a scenario resets known state, so it is repeatable.

Include a normal path, an awkward edge case and an attempt at an illegal transition.
Use domain labels, not reducer names or an unexplained raw JSON dump. Keep the state
and buttons more prominent than decoration. Add actions or scenarios when feedback
reveals a missing question.

Use inline HTML/CSS/script with no framework, bundler, server or real database unless
that dependency is the question. The file should open directly and remain shareable.
After the user reacts, record the answer and retain the file on the prototype branch.
Move only validated logic into a properly built production module; the HTML shell
stays outside main.
