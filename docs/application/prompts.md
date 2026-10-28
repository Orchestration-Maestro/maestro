# Prompts, queues and shell commands

[`AgentSession`](../../crates/maestro-app/src/agent_session/mod.rs) sends prompts,
queues messages while a response streams, delivers custom and user messages, and
runs direct shell commands whose output joins the conversation.

## Sending a prompt

`prompt(text, options)` takes these steps in order and stops at the first one that
ends the prompt:

1. When template expansion is on (the default) and the text starts with `/`, a
   registered extension command named by the text up to the first space runs with
   the rest as its arguments, even while a response streams. Nothing is sent. The
   extension error listeners hear a failing command as `command:<name>` with event
   `command`, and it still counts as run.
2. The extension `input` handlers see the text, the images and the source
   (`interactive` unless the options say otherwise). A handler can mark the input
   handled, which ends the prompt, or replace the text and images. An empty image
   list, whether supplied or set by a handler, is kept as an empty list. A failing
   handler is reported and the prompt goes on.
3. With expansion on, `/skill:<name> <arguments>` becomes the skill block followed
   by a blank line and the trimmed arguments, then `/<template> <arguments>`
   becomes the expanded template. The skill block is
   `<skill name="<name>" location="<file>">`, the line
   `References are relative to <directory>.`, a blank line, the skill body without
   its frontmatter and trimmed, and `</skill>`. The block is read back by the
   application's `parse_skill_block`. The native transcript uses it, and it is
   available to browser builds, which do not call it yet. An unknown
   skill keeps the text. A skill file that cannot be read is reported with the file
   as its path and event `skill_expansion`, and the text is kept.
4. While a response streams, the session queues the prompt instead, as a steering
   message or a follow-up, whichever `streaming_behavior` asks for. Without a choice
   it refuses the prompt with
   `Agent is already processing. Specify streamingBehavior ('steer' or 'followUp')
   to queue the message.`
5. Shell results recorded during the previous response are appended.
6. A model without configured credentials is refused with the login help for its
   provider.
7. When the history holds a response, the compaction check runs before sending.
8. The user message (the text, then the images), the custom messages kept for the
   next turn and the messages the `before_agent_start` handlers add are sent
   together. A system prompt from those handlers replaces the base prompt for this
   turn; otherwise the base prompt is restored.

`preflight_result` hears once whether the prompt was accepted: `true` when a
command ran, a handler handled the input, the prompt was queued or the agent is
about to be prompted, and `false` when a step failed. `prompt` returns after the
agent and any retry settle. The prompt starts when `prompt` is called, so its
steps up to the first wait (a command, the input handlers, queueing while a response
streams, a refusal) have already run before the returned future is first polled.
The prompt belongs to the session: the future only observes the result, and
dropping it leaves the prompt running to the end.

The base prompt is the system prompt construction built, or the one
`set_active_tools_by_name` last built for the active tools.

A failure supplied by an extension handler or listener, by shell operations or by a
credential lookup keeps its name, code and stack when the session returns it.

## Queues

`steer` and `follow_up` queue a user message on the agent after skill and template
expansion. The text of a registered command cannot be queued:
`Extension command "/<name>" cannot be queued. Use prompt() or execute the command
when not streaming.` Slash text that names no command is queued unchanged.

The session mirrors the queued texts. Each change is announced with a
`QueueUpdate` event that carries both lists. `get_steering_messages`,
`get_follow_up_messages` and `pending_message_count` read the mirrors.
`clear_queue` empties both mirrors and the agent's queues, announces the change and
returns the texts that were pending.

## Custom and user messages

`send_custom_message` places a custom message in the first of these that applies:

- kept for the next prompt when delivery is `nextTurn`, even while a response
  streams;
- queued while a response streams, as a follow-up when asked for and otherwise as
  a steering message, without entering the text mirrors;
- sent as a new turn when `trigger_turn` is set;
- appended to the history and the session, with a message start and end announced
  for it.

`send_user_message` always starts a turn. Block content becomes its text blocks
joined with newlines, followed by its images. Commands, skills and templates do not
expand, and the `input` handlers see the source as `extension`. While a response
streams it needs a delivery choice, as a prompt does.

## Direct shell commands

`execute_bash` runs a command in the session directory and records the result
under the command as given. A configured command prefix goes on its own line before
the command that runs. A configured shell path runs it unless the options supply
the shell operations. Progress chunks reach the caller in arrival order.
`abort_bash` stops the running command, which is then recorded as cancelled without
an exit status. `is_bash_running` is true while a command runs. Calling
`execute_bash` starts the command and installs its cancellation signal before the
returned future is polled. The command belongs to the session: dropping even a
never-polled result future leaves it running and its result is still recorded.

`record_bash_result` records a result. While the agent is idle the record is
appended to the history and the session at once. While a response streams it waits
(`has_pending_bash_messages` is true) and is appended before the next prompt's user
message, so a response's tool calls and results stay together. When appending one
fails, that prompt fails and only the results not yet appended stay pending. Shell
records announce no message start or end. `exclude_from_context` is stored with the
record when given, and a truncated result keeps the path of its full output.

## Qualified delivery

Prompt and message delivery, skill and template expansion, and queueing are checked
through the real session by
[`agent_session_prompt_checks`](../../crates/maestro-app/tests/agent_session_prompt_checks.rs),
[`agent_session_queue_checks`](../../crates/maestro-app/tests/agent_session_queue_checks.rs)
and
[`queued_slash_command_followup_checks`](../../crates/maestro-app/tests/queued_slash_command_followup_checks.rs).
Each case compares the fields its recorded operation holds: the session history and
queue mirrors, and, for a case that makes one, the received provider requests, the
tool results a run produced or the error it reported.
