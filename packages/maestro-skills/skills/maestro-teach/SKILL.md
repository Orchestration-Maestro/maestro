---
name: maestro-teach
description: "Use when the user wants to learn a topic or skill over multiple sessions in a learning workspace."
disable-model-invocation: true
---

# Teach toward a mission

## Workspace and paths

Resolve learning paths from the directory where this skill was invoked, not the installed skill directory.
Only the [formats sidecar](formats.md) is relative to this skill. Keep one mission per workspace:

| Workspace path | Purpose |
| --- | --- |
| `MISSION.md` | Why the learner wants the skill and observable success |
| `RESOURCES.md` | Annotated trusted sources and communities |
| `lessons/0001-slug.html` | Sequential, self-contained lessons |
| `reference/*.html` | Printable compressed reference material |
| `learning-records/0001-slug.md` | Sequential evidence of non-obvious learning |
| `assets/` | Reusable styles, widgets, diagrams and simulators |
| `NOTES.md` | Teaching preferences and working notes |

Use [formats](formats.md) for mission, resources, learning records and the workspace glossary. Establish a
missing or vague mission before teaching. When it changes, confirm with the learner, update it and write a
learning record. Reuse existing notes and records to avoid reteaching established knowledge.

## Ground the lesson

Build knowledge from trusted resources before writing teaching claims. Curate `RESOURCES.md` first if it is
sparse; do not trust model recall as the source. Prefer primary material and cite claims in lessons.

Teach one mission-linked skill at the learner's demonstrated level. Read learning records for prior knowledge
and misconceptions; if no exact request exists, choose the next relevant challenge that is just achievable.
Teach the needed knowledge before asking the learner to practise. Keep the lesson short enough for working
memory and give one tangible result, not an entire subject overview.

Distinguish immediate fluency from durable storage. Build retrieval from memory, spaced practice and, for
skills, interleaving related tasks. Difficulty should serve retention during practice, not obstruct explanation.

## Produce and practise

Scan existing lessons for the highest sequential number and increment it. Save one self-contained HTML lesson
in `lessons/`, with readable typography and layout. Link related lessons and reference material using anchors.
Recommend the best primary source found and invite follow-up questions when something remains unclear.
Open the file with an available platform command when possible, or report its path.

Inspect `assets/` before authoring; reuse components rather than duplicating them. Create a shared stylesheet
for the course and link each lesson to it. Put any genuinely reusable quiz, simulator or diagram component
there for later lessons. Follow the workspace's existing visual conventions.

Use interactive quizzes or real-world exercises with immediate, preferably automatic feedback. Give quiz
choices the same word count and, when possible, similar character counts. Vary the correct answer position;
formatting, order and answer length must not reveal it. Observe the learner's response before claiming mastery.

Create printable reference documents containing compressed procedures, algorithms, poses, syntax or other
reusable learning units. Keep their terms consistent with the glossary. Record demonstrated understanding,
corrected misconceptions, stated prior knowledge and confirmed mission changes; merely presenting material
or completing an activity does not prove learning.

## Wisdom and preferences

For questions requiring practical wisdom, attempt a grounded answer and recommend a reputable relevant
community where experience can be tested. Respect an opt-out and remember it in resources/preferences.
A community recommendation is not evidence that the learner joined or that a resource was verified.
Use `NOTES.md` for preferred teaching style so future sessions can honour it.
