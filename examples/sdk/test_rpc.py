"""Check the subprocess example without credentials or a model server."""

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

CLIENT = Path(__file__).with_name("rpc.py")


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

    def test_refusal_and_premature_eof_fail_without_waiting_for_agent_end(self):
        for reply in [
            '{"type": "response", "id": "state", "success": False, "error": "refused"}',
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
