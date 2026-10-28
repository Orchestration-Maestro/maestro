"""Inspect a Maestro RPC session, or print one prompted run as JSONL."""

import argparse
import json
import subprocess
import sys


def send(child, record):
    child.stdin.write((json.dumps(record, ensure_ascii=False) + "\n").encode("utf-8"))
    child.stdin.flush()


def receive(child, request_id, *, wait_for_end=False):
    accepted = False
    ended = not wait_for_end
    while not (accepted and ended):
        # Binary readline splits on LF only, preserving U+2028 and U+2029.
        line = child.stdout.readline()
        if not line:
            raise RuntimeError("EOF before the requested response or agent_end")
        record = json.loads(line.decode("utf-8"))
        print(json.dumps(record, ensure_ascii=False), flush=True)
        kind = record.get("type")
        if kind == "extension_ui_request" and record["method"] in (
            "select", "confirm", "input", "editor"
        ):
            send(child, {"type": "extension_ui_response", "id": record["id"], "cancelled": True})
        elif kind == "response" and record.get("id") == request_id:
            if not record["success"]:
                raise RuntimeError(record["error"])
            accepted = True
        elif kind == "agent_end":
            ended = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--prompt", help="Send plain prompt text; this can incur model charges")
    parser.add_argument("command", nargs=argparse.REMAINDER, help="-- maestro --mode rpc [options]")
    options = parser.parse_args()
    command = options.command
    if command[:1] == ["--"]:
        command = command[1:]
    if not command:
        parser.error("supply -- maestro --mode rpc [options]")
    with subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE) as child:
        completed = False
        try:
            send(child, {"type": "get_state", "id": "state"})
            receive(child, "state")
            if options.prompt is not None:
                send(child, {"type": "prompt", "id": "prompt", "message": options.prompt})
                receive(child, "prompt", wait_for_end=True)
            child.stdin.close()
            status = child.wait()
            if status:
                raise RuntimeError(f"Maestro exited with status {status}")
            completed = True
        finally:
            if not completed and child.poll() is None:
                child.terminate()


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, RuntimeError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
