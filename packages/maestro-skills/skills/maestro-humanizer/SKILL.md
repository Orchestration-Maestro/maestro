---
name: maestro-humanizer
description: "Use when editing or reviewing prose that sounds AI-written because of inflated claims, sales language, vague sources, repetitive structure, stock phrases, passive voice, filler or chatbot artifacts, while preserving its meaning and the writer's voice."
---

# Review prose without changing its claims

Rewrite AI-sounding text so it reads like the writer, not a chatbot. Do not change what it says or make up details.

The patterns below come from Wikipedia's ["Signs of AI writing"](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing), maintained by WikiProject AI Cleanup.

## What to do

When given text to humanize:

1. **Find AI patterns.** Check the text against the patterns below.
2. **Keep every claim.** You may shorten dull parts, expand useful parts, and merge or split paragraphs. Keep the information even when you change the structure.
3. **Do not invent facts.** Do not add a fact, name, number, date, quote, or citation unless it comes from the source or the user. If a sentence needs a missing detail, ask for it or use a simpler sentence. You may add an opinion or reaction when the writer's voice calls for one, but you may not add a factual claim. Fiction is exempt because invented details are part of the task.
4. **Match the voice.** Use the right tone for the text, such as formal, casual, or technical. Add personality only when the text and the writer call for it.

The input type controls what you return. See [How to return the result](#how-to-return-the-result). Use the same rewrite process in every mode.

## Match the writer's voice

If the user provides a writing sample (their own previous writing), analyze it before rewriting:

1. Read the sample first. Note its sentence length, word choice, paragraph openings, punctuation, repeated phrases, and transitions.
2. Match those habits. Do not replace casual words with formal ones or remove deliberate quirks.
3. If there is no sample, use the guidance below.

A writing sample takes priority over these style rules. If the sample uses em dashes, keep them at about the same rate. Do not apply §14 as a ban.

## Add personality only when it fits

Removing AI patterns is only half the job. The result should still sound like a person.

Use personality in blog posts, essays, opinions, and personal writing when it fits the writer. Keep reference, technical, legal, and factual text neutral. Do not add opinions or first-person language where they do not belong.

When personality fits, keep the writer's opinions, uncertainty, mixed feelings, humor, asides, and uneven rhythm. Never invent facts to make the text feel personal.

All examples below are illustrative, not historical claims. In a real edit, retain only facts
provided by the input; an example is not permission to invent a citation.

## Content patterns

### 1. Inflated claims about importance and legacy

**Words to watch:** historic breakthrough, defining achievement, lasting influence, central contribution, sweeping change. Check whether the source actually supports the scale claimed.
**Problem:** AI writing often claims that ordinary details mark a major change, prove a legacy, or reflect a broad trend.
**Before:**
> The example library opened a catalogue desk in May, a defining achievement that reshaped access to local knowledge.
**After:**
> The example library opened a catalogue desk in May.

### 2. Name-dropping to prove importance

**Words to watch:** independent coverage, local/regional/national media outlets, written by a leading expert, active social media presence
**Problem:** AI writing often lists well-known publications or follower counts to prove that a person matters. The list usually gives no useful context.
**Before:**
> The example consultant appears in several famous magazines and has an enormous audience, proving her authority. The supplied interview quotes her advice to retain an audit trail.
**After:**
> The supplied interview quotes the consultant's advice to retain an audit trail.

If the source explains what the person said and where, keep that useful citation. Do not invent context for a shorter version.

### 3. Shallow analysis with -ing phrases

**Words to watch:** highlighting/underscoring/emphasizing..., ensuring..., reflecting/symbolizing..., contributing to..., cultivating/fostering..., encompassing..., showcasing...
**Problem:** AI writing often adds an -ing phrase to make a simple fact sound deeper than it is.
**Before:**
> The example form uses blue headings, reflecting a commitment to clarity and fostering a culture of confident decisions.
**After:**
> The example form uses blue headings.

### 4. Sales language

**Words to watch:** boasts a, vibrant, rich (figurative), profound, enhancing its, showcasing, exemplifies, commitment to, natural beauty, nestled, in the heart of, groundbreaking (figurative), renowned, breathtaking, must-visit, stunning
**Problem:** AI writing often sounds like an advertisement, especially when it describes places, culture, products, or organizations.
**Before:**
> Our breathtaking workspace boasts a revolutionary dashboard for effortless team success.
**After:**
> The workspace has a dashboard showing team assignments.

### 5. Vague sources

**Words to watch:** Industry reports, Observers have cited, Experts argue, Some critics argue, several sources/publications (when few cited)
**Problem:** AI writing often assigns a claim to unnamed experts, critics, reports, or observers.
**Before:**
> Observers say the example queue is vital to organizational resilience. The supplied operations report says it retains jobs after a restart.
**After:**
> The supplied operations report says the example queue retains jobs after a restart.

Name a real source when the source text provides one. Otherwise, remove the unsupported claim. Never invent a source.

### 6. Formulaic challenges and outlook sections

**Words to watch:** Despite its... faces several challenges..., Despite these challenges, Challenges and Legacy, Future Outlook
**Problem:** AI articles often add a stock section about challenges, future prospects, or continued growth. These sections usually repeat vague claims instead of adding facts.
**Before:**
> Despite these challenges, the example club looks ahead to a dynamic future of continued growth. Its room booking still fails on Mondays.
**After:**
> The example club cannot book its room on Mondays.

Add details such as dates or public actions only when they come from the source or the user.

## Language and grammar patterns

### 7. Overused AI words

**Words to inspect in clusters:** delve, tapestry, pivotal, showcase, underscore. Keep established technical vocabulary, including literal gate/gating terminology; frequency and context matter more than any one word.
**Problem:** AI writing uses these words much more often than most people do, especially in groups.
**Before:**
> Additionally, the portal showcases an intricate tapestry of workflows, underscoring its pivotal role in helping staff find receipts.
**After:**
> Staff use the portal to find receipts.

### 8. Avoiding is and are

**Words to watch:** serves as/stands as/marks/represents [a], boasts/features/offers [a]
**Problem:** AI writing often replaces simple verbs such as *is*, *are*, and *has* with longer phrases.
**Before:**
> The example ledger serves as the record of payments and boasts a search field.
**After:**
> The example ledger is the record of payments and has a search field.

### 9. Not X but Y and clipped negative endings
**Problem:** AI writing overuses forms such as "Not only...but..." and "It's not just X, it's Y."

It also adds clipped endings such as "no guessing" instead of writing a clear clause.
**Before:**
> The example alert is not just a message; it is a promise of awareness, not merely a notification but a transformation.
**After:**
> The example alert tells the operator that a job failed.
**Before (tailing negation):**
> The selected account supplies the address, no guessing.
**After:**
> The selected account supplies the address, so the user does not have to guess it.

### 10. Forced groups of three
**Problem:** AI writing often forces ideas into groups of three to sound complete.
**Before:**
> The example meeting promises knowledge, momentum, and connection, alongside a demonstration and questions.
**After:**
> The meeting includes a demonstration and questions.

### 11. Changing names and repeating sentence openings
**Problem:** AI writing handles repetition by rule instead of by ear. It may keep renaming the same person or thing. It may also start several sentences with the same subject, often *she* or *he*.

Use one clear name for the same subject. For repeated openings, merge sentences, change the subject when that helps, or begin with the action.
**Before (synonym cycling):**
> The reader opens a record. The consumer checks its status. The client then returns the entry.
**After:**
> The reader opens a record, checks its status, and returns it.
**Before (repeated openings):**
> He read the label. He read the date. He read the note.
**After:**
> He read the label, date, and note.

Do not ban the repeated word. Fix the repeated sentence pattern. The remaining sentence may still start with "She."

### 12. False from X to Y ranges
**Problem:** AI writing often uses "from X to Y" when X and Y do not form a real range.
**Before:**
> This guide takes us from source files to limitless creativity.
**After:**
> This guide covers source files and gives examples of using them.

### 13. Passive voice and missing subjects
**Problem:** AI writing often hides who acts or drops the subject. Use active voice when it makes the actor and action clearer.
**Before:**
> No password needed. The entry is saved automatically.
**After:**
> You do not need a password. The example application saves the entry automatically.

## Style patterns

### 14. Em and en dashes

**Rule:** The final rewrite must not contain em dashes (—) or en dashes (–), unless the writer's sample uses them. Replace a dash with a period, comma, colon, or parentheses, or rewrite the sentence. Also check for spaced dashes (` — `) and double hyphens (` -- `) used as dashes.
**Before:**
> The setting—enabled by default—applies to new records—unless the owner opts out.
**After:**
> The setting is enabled by default and applies to new records unless the owner opts out.
**Before:**
> The form — changed yesterday — now accepts empty notes. Existing records -- still readable -- need no edits.
**After:**
> The form, changed yesterday, now accepts empty notes. Existing records are still readable and need no edits.

Before returning the rewrite, search for `—` and `–`. Remove each one unless the writer's sample uses that mark. In that case, match the sample's rate.

### 15. Too much bold text
**Problem:** AI chatbots often bold words and phrases without a clear reason.
**Before:**
> The example view includes **search**, **sorting**, and **filters**.
**After:**
> The example view includes search, sorting, and filters.

### 16. Lists with bold mini-headings
**Problem:** AI writing often uses vertical lists in which every item starts with a bold label and a colon.
**Before:**
> - **Labels:** Labels can be edited.
> - **Dates:** Dates can be sorted.
> - **Notes:** Notes can be searched.
**After:**
> You can edit labels, sort dates, and search notes.

### 17. Title case in headings
**Problem:** AI chatbots often capitalize every main word in a heading.
**Before:**
> ## How To Read The Example Ledger
**After:**
> ## How to read the example ledger

### 18. Emojis
**Problem:** AI chatbots often add emojis to headings and list items as decoration.
**Before:**
> 🚀 Launch the example
> ✅ Check the saved record
**After:**
> Launch the example and check the saved record.

### 19. Curly quotation marks
**Problem:** An assistant may use curly quotes (“...”) where the writer or target format uses straight quotes ("...").
**Before:**
> The note says “record saved” after the write.
**After:**
> The note says "record saved" after the write.

## Chatbot patterns

### 20. Chatbot text left in the answer

**Words to watch:** I hope this helps, Of course!, Certainly!, You're absolutely right!, Would you like..., Want me to...?, Want me to give examples?, Should I continue?, let me know, here is a...
**Problem:** A chatbot's greeting, offer, or closing sometimes remains in text that should stand on its own.
**Before:**
> Of course! The example file format contains a header and ordered records. Let me know if you want more details!
**After:**
> The example file format contains a header and ordered records.

### 21. Knowledge-limit disclaimers and guesses

**Words to watch:** my training ended, sources are sparse, apparently lived, perhaps attended, probably began. Check whether an admission of missing evidence is followed by an unsupported biography or other guess.
**Problem:** Older models may mention the date when their knowledge ends. A model may also explain that it could not find a source, then fill the gap with a plausible guess. State what the source does not show, or remove the sentence. Do not present a guess as a fact.
**Before (cutoff disclaimer):**
> As of my last update, the example group may have formed around the time the workshop opened.
**After:**
> The supplied notes do not give the example group's founding date.
**Before (speculative gap-fill):**
> The example author shares little about childhood, perhaps because she values privacy and grew up far from a city.
**After:**
> The supplied notes contain no account of the example author's childhood.

### 22. Overly agreeable tone
**Problem:** AI assistants often praise the user or agree before giving the answer.
**Before:**
> Excellent point! You are absolutely right that the example error does not identify the missing field.
**After:**
> The example error does not identify the missing field.

## Filler and hedging

### 23. Filler phrases

**Before → After:**
- "For the purpose of reading records" → "To read records"
- "On account of the missing file" → "Because the file is missing"
- "During the present period" → "Now"
- "Should a failure be encountered" → "If it fails"
- "The reader possesses the capacity to parse" → "The reader can parse"
- "A point worth mentioning is that the test fails" → "The test fails"

### 24. Too many qualifiers

**Phrases to watch:** to be fair, it's also possible, could potentially, might arguably, in some cases it may, this is an inference
**Problem:** Repeated editing can add one qualifier after another until every claim sounds uncertain. Keep a qualifier only when the source supports it and the meaning needs it. Remove caveats that only repair an earlier overstatement.
**Before:**
> The change could potentially perhaps reduce delays in some cases.
**After:**
> The change may reduce delays.

### 25. Generic positive endings
**Problem:** AI writing often ends with vague optimism instead of the last useful fact.
**Before:**
> This improvement begins an exciting journey toward a brighter future for the example project.
**After:**
> (Delete the send-off. End with the last supported result or concrete plan.)

### 26. Too many hyphenated word pairs

**Words to watch:** third-party, cross-functional, client-facing, data-driven, decision-making, well-known, high-quality, real-time, long-term, end-to-end
**Problem:** AI writing often hyphenates these pairs everywhere. Keep the hyphen before a noun when grammar needs it, as in `a high-quality report`. Drop it after the noun, as in `the report is high quality`.
**Before:**
> The well-tested parser is well-tested, and the real-time view updates in real-time.
**After:**
> The well-tested parser has tests, and the real-time view updates in real time.

### 27. Pretending to reveal a deeper truth

**Phrases to watch:** The real question is, at its core, in reality, what really matters, fundamentally, the deeper issue, the heart of the matter
**Problem:** AI writing uses these phrases to make an ordinary point sound like a hidden truth.
**Before:**
> At its core, the deeper truth is that the example import needs a filename.
**After:**
> The example import needs a filename.

### 28. Announcing the next point

**Phrases to watch:** Let's dive in, let's explore, let's break this down, here's what you need to know, now let's look at, without further ado, heads up, quick note, before I forget
**Problem:** AI writing often announces the next point instead of stating it. A casual phrase such as "one thing that bit me" can have the same problem. Remove the announcement, not just its formal tone.
**Before:**
> Let us explore what you need to know about the example reader.
**After:**
> The example reader accepts a path and returns ordered records.
**Before (casual register):**
> One thing that caught me out, so listen closely: the example cache stores successful lookups only.
**After:**
> The example cache stores successful lookups only.

### 29. A heading repeated in the first sentence

**Signs to watch:** A heading followed by a one-line paragraph that simply restates the heading before the real content begins.
**Problem:** AI writing often follows a heading with a sentence that only repeats the heading. Remove the repeated sentence.
**Before:**
> ## Errors
>
> Errors matter.
>
> A missing field returns its name to the caller.
**After:**
> ## Errors
>
> A missing field returns its name to the caller.

### 30. Writing about the previous version
**Problem:** Documentation and comments should describe the current behavior. Mention the previous version only in change logs, release notes, migration guides, and other documents about change.
**Before:**
> This lookup replaced the old loop and was added to improve the previous implementation.
**After:**
> The lookup uses an index to avoid scanning every record.

### 31. Forced punchlines and dramatic fragments
**Problem:** AI writing often turns each sentence into a dramatic closing line. One short sentence can add emphasis. A row of short fragments usually feels forced.
**Before:**
> Then the import ran. No pauses. No hesitation. No looking back. The old world was gone.
**After:**
> The import completed without pausing for manual approval.

### 32. Formulaic sayings

**Words to watch:** X is the Y of Z, X becomes a trap, X is not a tool but a mirror, the language of, the currency of, the architecture of
**Problem:** AI writing often turns an ordinary claim into a saying that sounds deep but adds no detail. Replace the saying with the specific claim.
**Before:**
> Simplicity is the currency of confidence. The form is a mirror, not a tool.
**After:**
> The form has fewer fields, so the reviewer has less to check.

### 33. Fake-candid openings

**Phrases to watch:** Honestly?, Look, Here's the thing, The thing is, Let's be honest, Real talk, when used as standalone hooks or fake-candid pauses before an ordinary point.
**Problem:** AI writing often starts with a staged pause or claim of honesty before making a routine point. State the point directly.
**Before:**
> Is the example worth keeping? Honestly? That depends on whether anyone uses it.
**After:**
> Whether the example is worth keeping depends on whether anyone uses it.

### 34. Answering objections no one raised

**Phrases to watch:** This isn't (mainly/really) about, I'm not saying/arguing/trying to, To be clear, Don't get me wrong, This is not to say, You could argue/frame this differently but, Some might say... but
**Problem:** AI writing may answer an objection that does not appear in the text. Watch for an unattributed statement about what the writer does not mean, especially when the topic appears nowhere else. A direct claim such as "the API is not thread-safe" is not this pattern.
**Before:**
> To be clear, I am not saying that documentation has no value. This is not about perfection. The link points to a missing file.
**After:**
> The link points to a missing file.

Remove only the unsupported defense. If it contains a real claim, state that claim directly. Keep an objection when the text names its source or answers it in full.

### 35. Rejecting fake alternatives

**Phrases to watch:** A tempting option/approach would be, One might be tempted to, An obvious approach would be, You might think... but, It would be easy to just, Some would suggest
**Problem:** AI writing may introduce an option that no reader would consider, reject it in a clause, and never mention it again. This often leaves an old drafting idea in the final text. Remove the fake option and state the real constraint directly.
**Before:**
> An obvious alternative would be to restart the whole machine every time, but the example only needs to reload the changed file.
**After:**
> The example reloads only the changed file.

One rejected option may be valid. Several short, unrelated rejections are a stronger sign. Ask what new information each sentence adds. If it only records an earlier edit, rewrite the paragraph around its main point.

## Check for false positives

### What not to flag

A person may use some of these patterns. Do not treat any item below as proof by itself:

- **Perfect grammar and consistent style.** Many writers are professionals or have been edited. Polish does not equal AI.
- **Mixed casual and formal styles.** This can reflect the writer's field, age, or personal habits.
- **"Bland" or "robotic" prose.** AI prose has *specific* tells. Generic dryness without those tells is just dry writing.
- **Formal or academic words.** §7 lists specific words that AI writing overuses. Do not simplify every formal word.
- **Letter-style opening or closing on a comment.** Salutations and sign-offs predate language-model assistants.
- **Common transition words in isolation.** *Additionally*, *moreover*, *consequently* are AI-coded only when piled up. One *however* is not a tell.
- **Curly quotes alone.** macOS, Word, Google Docs, and most CMSes auto-curl by default. Curly quotes only count when stacked with other tells.
- **Em dashes alone.** Many editors and journalists use them often. Em dashes are evidence only when paired with formulaic sales-y rhythm.
- **One short sentence for emphasis.** Flag dramatic fragments only when several appear in a row.
- **Deliberate repeated openings.** Writers may repeat an opening to build rhythm or pressure, as in "She came. She saw. She conquered." Change it only when the repetition adds nothing.
- **"Honestly" or "look" mid-sentence.** These are ordinary in casual writing. The tell is the standalone theatrical opener, not the word itself.
- **Useful limits and disclaimers.** Keep scope statements, legal and safety notices, real corrections, named objections, replies, and FAQ answers.
- **Real alternatives.** Keep options that a reader may consider in a design document, tutorial, or argument. Remove only an unlikely option that the text dismisses and never uses again.
- **Unsourced claims.** Most of the web is unsourced. Lack of citations doesn't prove anything.
- **Correct, complex formatting.** Visual editors and templates produce clean output without any AI.
- **Secondhand text.** Do not rewrite watched phrases inside quotations, titles, proper names, or examples where the phrase is being discussed rather than used.

When unsure, look for several patterns together. One em dash proves nothing. Several stock patterns in the same passage are stronger evidence.

### Human details to keep

These details often carry the writer's voice. Keep them unless they hurt the meaning:

- **Specific, unusual details.** Keep a real address, an odd quote, or a phrase such as "the lawyer who used to work upstairs from my dentist."
- **Mixed feelings and unresolved tension.** Keep lines such as "I think this is mostly good, but it bothers me, and I can't fully explain why."
- **Dated, era-bound references.** Slang, memes, or in-jokes that map to a specific year and subculture. Models lag by a year or more.
- **Deliberate first-person choices.** Keep a cut or word choice when the writer can explain why it belongs.
- **Variety in sentence length.** Real writing alternates short and long. AI writing tends toward an even, mid-length cadence.
- **Genuine asides, parentheticals, or self-corrections.** "(I keep wanting to say 'almost' here, but it really was certain.)" Models rarely interrupt themselves like this.
- **Older edits.** Text predating November 30, 2022, the public launch of widely used chat assistants, is very unlikely to have been generated by one.

---

## How to return the result

**Pasted text (default).** Return the draft, a short list of remaining AI patterns, and the final rewrite.

**File mode.** When the user names a file, run the full rewrite process but write only the final text to the file. Change prose only. Keep code blocks, YAML metadata, data, and link targets unchanged. Then give the user a short summary.

**Embedded mode.** When another task uses this skill for a pull request, commit message, or document, return only the final text.

## Rewrite process

1. Read the source and mark each AI pattern.
2. Write a draft. Read it aloud. Check the rhythm, details, simple verbs such as *is* and *has*, and the right level of formality.
3. Ask two questions:
   - **"What still sounds AI-generated?"**
   - **"Did the rewrite add or remove any fact, name, number, date, quote, citation, ranking, or other claim?"**
   Treat any unsupported addition or lost claim as an error.
4. Write the final version. State each point naturally instead of patching one flagged phrase at a time. If a sentence stays awkward, rewrite the paragraph around its main point. Apply the dash rule in §14.

Return the result required by [How to return the result](#how-to-return-the-result).

## Source

This skill is based on [Wikipedia: Signs of AI writing](https://en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing), maintained by WikiProject AI Cleanup. Its patterns come from reviews of AI-generated text on Wikipedia.

Treat predictable stock phrasing as a prompt to inspect the text, not as proof of who wrote it.
