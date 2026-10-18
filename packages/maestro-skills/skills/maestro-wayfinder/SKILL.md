---
name: maestro-wayfinder
description: "Use when an effort is too large or uncertain to specify in one session."
disable-model-invocation: true
---

# Name the destination first

Establish what reaching the destination means through `/skill:maestro-grilling` and
`/skill:maestro-domain-modeling`: a spec, a decision or a change. The destination fixes
scope. This is planning by default, not permission to implement the whole effort.
Only an explicit override in the map's Notes carries execution into the map.

## Map decisions, not implementation steps

Use the [tracker's wayfinding operations](../../references/issue-tracker.md).
One issue labelled `wayfinder:map` indexes child decision tickets. Use only
`wayfinder:` labels for this map and its tickets, not implementation triage states.
Refer to every ticket by its linked title, not a bare number. The map is an index,
not a duplicate store: the resolution lives once in the ticket.

Keep these map sections:

- Destination: the concrete endpoint in one or two lines.
- Notes: domain, standing preferences and skills sessions should consult.
- Decisions so far: linked closed-ticket titles with a one-line gist.
- Not yet specified: in-scope questions too foggy to phrase as tickets yet.
- Out of scope: consciously excluded work and the reason it is outside this effort.

Open tickets are discovered as children, not copied into a second map inventory.
Each ticket asks one decision-sized question and carries `wayfinder:<type>`.
A sharp question is a ticket even if blocked. Fog means the question cannot yet be
phrased, not merely that its answer is unknown. Excluded work never graduates from fog.

## Ticket types

- `wayfinder:research` is AFK: the controller uses `/skill:maestro-research` to
  dispatch one background researcher. The worker reads sources and returns its cited
  file without invoking dispatching skills. Link its artifacts from the ticket. Where research
  uses a throwaway branch, retain it as `research/<name>` without merging or opening
  a request; pushing still follows the user's permission rules.
- `wayfinder:prototype` is HITL: use `/skill:maestro-prototype` so the human can react
  to a concrete artifact. Link that artifact as evidence.
- `wayfinder:grilling` is HITL: use grilling and domain modeling to settle the question
  with the human, never by simulating their answer.
- `wayfinder:task` is HITL or AFK: do prerequisite work that makes a decision possible,
  such as provisioning access. Give a precise human checklist where needed. Record
  resulting facts and authorized credential locations, not secrets.

## Chart a new map

1. Name the destination, then grill breadth-first across the space. If the route is
   already clear with no fog, ask how to proceed instead of manufacturing a map.
2. Create the map and all currently sharp child questions. Leave the rest as fog.
3. In a second pass, wire native blocking edges and cross-references using real ids.
   Use the tracker's fallback only when native blocking is unavailable. Never publish
   placeholder issue links that could point to unrelated tickets.
4. Launch independent research with self-contained briefs and artifact locations.
   Stop after charting; do not hand-resolve the human decisions in the same session.

## Work through an existing map

Read the map and Notes first, then the selected ticket's label and body. Use a named
ticket when supplied; otherwise take the first open, unblocked, unclaimed child in
map order, then number order. Claim by assignment as the first write before work.
A frontier ticket has no open blockers and no assignee. Other sessions may be active.

Work one non-research ticket per session; independent research is the exception.
Fetch related resolutions only as needed. Apply the ticket type's method and let the
human speak for themselves. Record the answer as a resolution comment, close the
issue and append a named pointer in Decisions so far. Graduate newly sharp fog into
new tickets using create-then-wire; remove its old fog entry.

If a decision invalidates tickets, update or remove them. If a ticket proves out of
scope, close it with the reason and link it from Out of scope, not Decisions so far.
When no fog or open decisions remain, hand off to `/skill:maestro-to-spec` and
`/skill:maestro-to-tickets` rather than silently starting a large build. Requests that
need ordinary implementation triage can enter through `/skill:maestro-triage` when
the user chooses that next phase.
