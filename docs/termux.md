# Maestro on Android with Termux

Maestro has no verified Android route yet. No build, install or first-session
sequence has run on a device, so this page publishes none. [Run Maestro on Android
under Termux](https://github.com/Orchestration-Maestro/maestro/issues/2818) owns
that work. What this page covers is what Maestro does when it detects Termux, and
the device setup you can prepare now: apps, packages, storage permission and
project instructions.

## Contents

1. [Support status](#support-status)
2. [Prerequisites](#prerequisites)
3. [Grant shared storage](#grant-shared-storage)
4. [Use the Android clipboard](#use-the-android-clipboard)
5. [Add Termux instructions](#add-termux-instructions)
6. [Troubleshooting](#troubleshooting)

## Support status

Maestro detects Termux from a nonempty `TERMUX_VERSION` in its environment.

| Capability | Status | Where it is defined |
| --- | --- | --- |
| Build, install and start Maestro on Android | Not qualified. No command is published. | [#2818](https://github.com/Orchestration-Maestro/maestro/issues/2818) |
| Copy text to the Android clipboard | Implemented. Maestro runs `termux-clipboard-set` before any display helper. | [Copying text](application/clipboard.md) |
| Paste an image from the clipboard | Not available on Termux. Maestro skips the read. | [Image paste](chat/interactive-mode.md#image-paste) |
| `find` and `grep` search programs | Maestro does not download them on Android. It prints the `pkg install` line. | [Search tools](tools-search.md#search-programs) |
| Screen redraw when the keyboard changes the height | Implemented. A height change keeps the scrollback. | [Terminal rendering](terminal/rendering.md#defaults) |

An npm command from another tool is not a Maestro installer. Maestro is a Rust
executable; do not install it from a package registry.

## Prerequisites

1. Install Termux from [GitHub or F-Droid](https://github.com/termux/termux-app#installation).
   The Google Play build is deprecated.
2. Install [Termux:API](https://github.com/termux/termux-api#installation) from the
   same source as Termux. It is the Android app behind the `termux-*` commands.
3. In Termux, update the packages and add the command-line side of the API and the
   search programs:

   ```sh
   pkg update && pkg upgrade
   pkg install termux-api fd ripgrep
   ```

Termux:API is optional. You need it for the Termux clipboard route and for any
shell command that reaches an Android device service.

## Grant shared storage

Termux cannot read shared Android storage until you approve it. Run this once and
accept the Android prompt:

```sh
termux-setup-storage
```

Shared storage then appears at `/storage/emulated/0` and under `~/storage/`. Every
Maestro tool, including `bash`, runs with the permissions of the Termux process,
so this grant covers Maestro too. Approve it only when Maestro should be able to
change those files.

## Use the Android clipboard

Copying works through `termux-clipboard-set`. Maestro writes the text to that
command's standard input and tries it before `wl-copy`, `xclip` and `xsel`. If it
fails, those helpers are tried next, and then a terminal clipboard sequence
(OSC 52). The error `Failed to copy to clipboard` appears only when every helper
failed and the text is too large for that sequence. The full routing is in
[Copying text](application/clipboard.md).

Check the Termux side outside Maestro first:

```sh
printf 'clipboard check' | termux-clipboard-set
termux-clipboard-get
```

The second command prints `clipboard check`. If it prints nothing or fails, fix the
Termux:API install before looking at Maestro.

Maestro does not read text from the clipboard. Paste text with Termux's own paste
action. The Termux clipboard holds text only, so the image paste key
(`app.clipboard.pasteImage`, Ctrl+V by default) attaches nothing and leaves the
editor unchanged.

## Add Termux instructions

Maestro detects Termux but cannot know what you want it to do on the device. Put
the details that matter in the global instruction file at
`~/.maestro/agent/AGENTS.md`. In Termux, `~` is `/data/data/com.termux/files/home`.
A small example:

```markdown
# Termux environment

- This session runs in Termux on Android.
- Shared storage is under `/storage/emulated/0`. Touch it only when the task needs it.
- Open a URL with `termux-open-url "https://example.com"`.
- Open a file with `termux-open <path>`.
- Copy text with `termux-clipboard-set`.
```

Enter `/reload` in a running session to pick up the change, or restart Maestro.
[Context files](application/resources.md#context-files) lists the file names and
their order, and [reload](chat/commands.md#reload) lists what else it refreshes.

## Troubleshooting

### The copy command fails

Copying a long text can fail with `Failed to copy to clipboard`, and a short text
can reach the clipboard through the terminal sequence when `termux-clipboard-set`
fails. Confirm that both parts are installed: the Termux:API app and the
`termux-api` package. Then run the two clipboard commands from
[Use the Android clipboard](#use-the-android-clipboard) outside Maestro. If they
fail there, the fault is in Termux:API.

### A shared-storage path is denied

Run `termux-setup-storage`, accept the Android prompt, and retry the path under
`~/storage/` or `/storage/emulated/0`.

### Maestro reports that `fd` or `ripgrep` is missing

The line reads `fd not found. Install with: pkg install fd`, or the same with
`ripgrep`. Run the `pkg install` command it shows, then repeat the search.

### Maestro does not start

No Android build is qualified, so a failure to build or start is a known gap, not a
configuration fault. Follow
[#2818](https://github.com/Orchestration-Maestro/maestro/issues/2818) for the route.
