# Glossary format

Use the existing glossary's format and name. With no convention, start `CONTEXT.md` lazily when the first term settles.
Give the context a name and one sentence describing its domain.

For each canonical term, write a tight definition and list competing names under “Avoid”. Pick one spelling.
Record domain relationships explicitly: an Order belongs to a Customer; an Invoice requests payment for delivered work.
Only include concepts specific to this domain. Keep code layout, implementation notes, plans and progress elsewhere.

Group terms only when useful. A single context needs no context map. For several contexts, a map links each
glossary, states its responsibility and describes relationships between contexts. Follow an existing map rather
than inventing a second one. When a term crosses contexts, say whether its meaning is shared or translated.
