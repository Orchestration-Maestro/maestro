"""Check the subprocess example without credentials or a model server."""

import json
import re
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

CLIENT = Path(__file__).with_name("rpc.py")


class RpcGuideTests(unittest.TestCase):
    def test_streaming_guide_uses_rpc_updates_and_message_end_completion(self):
        guide = (CLIENT.parents[2] / "docs/rpc.md").read_text(encoding="utf-8")
        section = guide.split("## Render a streamed answer\n", 1)[1].split("## Events", 1)[0]
        table = section.split("| Inner event |", 1)[1].split("\n\n", 1)[0]
        rows = "\n".join(row.split("|")[1] for row in table.splitlines()[2:])
        self.assertEqual(
            set(re.findall(r"`([a-z_]+)`", rows)),
            {"text_start", "text_delta", "text_end", "thinking_start", "thinking_delta",
             "thinking_end", "toolcall_start", "toolcall_delta", "toolcall_end"},
        )
        for field in ["message_end.message.stopReason", "error", "aborted", "toolUse", "agent_end"]:
            self.assertIn(f"`{field}`", section)
        self.assertNotRegex(section, r"`(?:done|error)`\s+with")


class RpcExampleTests(unittest.TestCase):
    def run_peer(self, script, *options):
        with tempfile.TemporaryDirectory() as root:
            peer = Path(root, "peer.py")
            peer.write_text("import json, sys\n" + script, encoding="utf-8")
            return subprocess.run(
                [sys.executable, str(CLIENT), *options, "--", sys.executable, str(peer)],
                capture_output=True,
            )

    def test_lf_records_keep_unicode_separators_and_cancel_startup_dialog(self):
        result = self.run_peer('''
print(json.dumps({"type": "extension_ui_request", "id": "dialog", "method": "input"}), flush=True)
state = json.loads(sys.stdin.buffer.readline())
answer = json.loads(sys.stdin.buffer.readline())
assert state == {"type": "get_state", "id": "state"}
assert answer == {"type": "extension_ui_response", "id": "dialog", "cancelled": True}
record = {"type": "response", "id": "state", "success": True, "data": {"sessionName": "a\\u2028b\\u2029c"}}
sys.stdout.buffer.write(json.dumps(record, ensure_ascii=False).encode() + b"\\r\\n")
sys.stdout.buffer.flush()
assert sys.stdin.buffer.read() == b""
''')
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        records = [json.loads(line) for line in result.stdout.split(b"\n") if line]
        self.assertEqual(records[-1]["data"]["sessionName"], "a\u2028b\u2029c")

    def test_prompt_waits_for_completion_and_correlated_acceptance(self):
        result = self.run_peer('''
state = json.loads(sys.stdin.buffer.readline())
print(json.dumps({"type": "response", "id": state["id"], "success": True}), flush=True)
prompt = json.loads(sys.stdin.buffer.readline())
assert prompt == {"type": "prompt", "id": "prompt", "message": "Check fixture"}
for record in [
    {"type": "response", "id": "unrelated", "success": True},
    {"type": "extension_ui_request", "id": "notice", "method": "notify", "message": "running"},
    {"type": "message_update", "assistantMessageEvent": {"type": "text_delta", "delta": "Checked"}},
    {"type": "agent_end", "messages": []},
    {"type": "response", "id": "prompt", "success": True},
]:
    print(json.dumps(record), flush=True)
assert sys.stdin.buffer.read() == b""
''', "--prompt", "Check fixture")
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        records = [json.loads(line) for line in result.stdout.split(b"\n") if line]
        self.assertEqual(records[-1]["id"], "prompt")
        self.assertIn("agent_end", [record["type"] for record in records])

    def test_prompt_keeps_reading_after_acceptance_until_agent_end(self):
        result = self.run_peer('''
state = json.loads(sys.stdin.buffer.readline())
print(json.dumps({"type": "response", "id": state["id"], "success": True}), flush=True)
prompt = json.loads(sys.stdin.buffer.readline())
assert prompt["id"] == "prompt"
for record in [
    {"type": "response", "id": "prompt", "success": True},
    {"type": "message_update", "assistantMessageEvent": {"type": "text_delta", "delta": "Later"}},
    {"type": "agent_end", "messages": []},
]:
    print(json.dumps(record), flush=True)
assert sys.stdin.buffer.read() == b""
''', "--prompt", "Check fixture")
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        records = [json.loads(line) for line in result.stdout.split(b"\n") if line]
        kinds = [record["type"] for record in records]
        self.assertEqual(kinds[-1], "agent_end")
        self.assertLess(kinds.index("response", 1), kinds.index("agent_end"))

    def test_prompt_accepted_then_eof_fails_without_agent_end(self):
        result = self.run_peer('''
state = json.loads(sys.stdin.buffer.readline())
print(json.dumps({"type": "response", "id": state["id"], "success": True}), flush=True)
sys.stdin.buffer.readline()
print(json.dumps({"type": "response", "id": "prompt", "success": True}), flush=True)
''', "--prompt", "Check fixture")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"EOF", result.stderr)

    def test_nonzero_child_exit_fails_after_a_successful_response(self):
        result = self.run_peer('''
request = json.loads(sys.stdin.buffer.readline())
print(json.dumps({"type": "response", "id": request["id"], "success": True}), flush=True)
assert sys.stdin.buffer.read() == b""
sys.exit(3)
''')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(b"status 3", result.stderr)

    def test_refusal_and_premature_eof_fail_without_waiting_for_agent_end(self):
        for reply in [
            '{"type": "response", "id": "state", "success": false, "error": "refused"}',
            None,
        ]:
            with self.subTest(reply=reply):
                script = 'sys.stdin.buffer.readline()\n'
                if reply:
                    script += f'print({reply!r}, flush=True)\n'
                result = self.run_peer(script)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b"refused" if reply else b"EOF", result.stderr)


if __name__ == "__main__":
    unittest.main()
