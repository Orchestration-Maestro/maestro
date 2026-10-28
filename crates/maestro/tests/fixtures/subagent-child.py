#!/usr/bin/env python3
"""Controlled JSON-print child: validates literal launch operands, never calls a model."""
import json
import os
import sys

args = sys.argv[1:]
assert args[:4] == ["--mode", "json", "-p", "--no-session"], args
assert args[args.index("--model") + 1] == "controlled-model", args
assert args[args.index("--tools") + 1] == "read,ls", args
with open(args[args.index("--append-system-prompt") + 1], encoding="utf-8") as prompt:
    role = prompt.read()
assert role == "Private role prompt", role
if args[-1] == "Task: fail":
    sys.stderr.write("controlled failure")
    sys.exit(7)
if args[-1] in ("Task: provider-error", "Task: provider-abort"):
    print(json.dumps({"type":"message_end","message":{"role":"assistant","content":[],"usage":{"totalTokens":19}}}))
    reason = "error" if args[-1].endswith("error") else "aborted"
    error = "controlled model failure" if reason == "error" else "controlled model abort"
    print(json.dumps({"type": "message_end", "message": {"role": "assistant", "content": [], "stopReason": reason, "errorMessage": error}}))
    sys.exit(0)
print("not JSON\n\n")
print(json.dumps({"type": "tool_result_end", "message": {"role": "toolResult", "content": [{"type": "text", "text": "read result"}]}}))
for index in range(2):
    usage = {"input": 10 if index == 0 else 2, "output": 3 if index == 0 else 5, "cacheRead": 2, "cacheWrite": 3, "cost": {"total": 0.001 if index == 0 else 0.002}, "totalTokens": 19}
    text = "earlier" if index == 0 else f"{args[-1]};role={role};cwd={os.path.basename(os.getcwd())}"
    event = {"type": "message_end", "message": {"role": "assistant", "content": [{"type": "text", "text": text}, {"type": "text", "text": "not the first block"}], "usage": usage, "model": "reported-model", "stopReason": "stop"}}
    print(json.dumps(event), end="\n" if index == 0 else "", flush=True)
sys.stderr.write("controlled stderr")
