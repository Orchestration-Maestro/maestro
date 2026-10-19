---
name: maestro-wizard
description: "Use when a person must provision credentials, follow dashboard steps or perform a manual setup, migration or cutover."
---

# Author a human-guided Bash procedure

Use this only for actions a person must perform, not work the agent can complete itself. Keep the wizard
ephemeral unless the user wants a committed repeatable setup path.

1. Read real configuration, environment templates, setup documentation and CI secret/variable references.
   For transitions, establish current state, target and irreversible actions. Show ordered stages and confirm
   them with the user. Every captured value needs a source, destination and secret/public classification.
2. Verify each dashboard URL, click path and command from current documentation or the user. State gaps;
   do not invent a dashboard step. Some stages are actions and capture no value.
3. Copy [template.sh](template.sh). Keep its library above STAGES unchanged. Replace only example stages
   inside `run_wizard`, in dependency order; set `TOTAL_STAGES` accurately and keep the final invocation.
   Use `stage`, instructions and `open_url` before asking for values. Use `ask_secret` for secrets, `write_env`
   for local persistence and `set_secret` only for names CI actually uses. Ordinary CI variables use `set_var`.
   Confirm irreversible actions. Keep each stage focused so required instructions remain visible.
4. Run `bash -n` on the generated script and installed shellcheck if available; report diagnostics honestly.
   Make it executable. Statically trace every captured value to its agreed destination and check CI names.
   Hand off the exact Bash invocation without running the interactive provisioning procedure yourself.

The template requires Bash, including arrays, hidden reads and its quoting rules. Windows needs a Bash
environment; this is not portable POSIX sh. Its opener tries wslview, explorer.exe, xdg-open and open, then
prints a manual URL warning. It clears stages only on a terminal and falls back when colour is unavailable.

The helpers retain hidden secret input, existing-value defaults, EOF failure, default-no confirmation,
quoted rerunnable environment upserts and mode 0600 when creating a file. Existing modes and symlink targets
are preserved. Secrets reach `gh` over stdin; missing, unauthenticated or failed `gh` writes produce warnings
and skipped-work summaries. Completion names keys, never secret values. The included dashboard stage is an
illustration to replace, not a claim that the service was visited or configured. For repeatable setup, link the
finished script from the project's setup guide; otherwise remove the scratch procedure after use.
