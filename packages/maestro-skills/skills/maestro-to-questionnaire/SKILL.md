---
name: maestro-to-questionnaire
description: "Use when decisions or facts must come from someone other than the sender, in writing or a shared meeting."
disable-model-invocation: true
---

# Ask the sender what they need back

Interview the sender about the recipient and the required answer, not about a subject the sender cannot know.

1. Through `ask_user_question`, establish the recipient's role, expertise and relationship to the sender.
   Know what context they hold that the sender lacks.
2. Establish the concrete decisions or facts the sender needs back. Stop for the answer; do not fill gaps
   with invented project details.
3. Write `to-questionnaire-<slug>.md` in the current directory, deriving the slug from the topic. Report the
   path. Every required outcome must have a focused question.

## Document shape

Use a discovery-questionnaire title and the following sections:

- Purpose: why it exists and the decision depending on it.
- From, To and How answers will be used: identify sender, recipient and answer destination.
- Context: one short orienting paragraph for someone outside the sender's current thinking.
- How to answer: deadline, approximate effort and guidance that partial answers, uncertainty and "I don't
  know" are useful. Obtain missing timing details instead of inventing a deadline.
- Themes: important questions first, grouped under headings when there are more than a handful.

Each question asks one idea, has an answer stub immediately beneath it and, only when needed, a short reason
that prevents misunderstanding or a throwaway answer. Use a final "Anything else?" catch-all for missed facts.
Question examples are illustrative, never assertions about the current project's load, staffing or constraints.

For subsequent discussion, suggest `/skill:maestro-grilling` to the human rather than automatically invoking
another type-only entry point.
