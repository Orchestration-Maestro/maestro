#!/usr/bin/env bash
# Controlled interactive walkthrough of the built maestro binary in an 80x24 tmux pane.
# Usage: walkthrough.sh <maestro binary> <output dir>
set -euo pipefail
binary=$1
out=$2
mkdir -p "$out"
root=$(mktemp -d "${RUNNER_TEMP:-/tmp}/maestro-walk.XXXXXX")
mkdir -p "$root/agent" "$root/project"

cat > "$root/provider.py" <<'PYTHON'
import http.server, json, sys
class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = self.rfile.read(int(self.headers.get('content-length', 0)))
        with open(sys.argv[2], 'ab') as log:
            log.write(body + b'\n')
        chunk = {"id": "controlled", "choices": [{"index": 0, "delta": {"content": "walkthrough-reply-42"}, "finish_reason": "stop"}],
                 "usage": {"prompt_tokens": 20, "completion_tokens": 4, "total_tokens": 24}}
        events = f"data: {json.dumps(chunk)}\n\ndata: [DONE]\n\n".encode()
        self.send_response(200)
        self.send_header('content-type', 'text/event-stream')
        self.send_header('content-length', str(len(events)))
        self.send_header('connection', 'close')
        self.end_headers()
        self.wfile.write(events)
    def log_message(self, *args):
        pass
server = http.server.HTTPServer(('127.0.0.1', 0), Handler)
with open(sys.argv[1], 'w') as port:
    port.write(str(server.server_address[1]))
server.serve_forever()
PYTHON
python3 "$root/provider.py" "$root/port" "$out/provider-requests.jsonl" &
provider=$!
cleanup() {
  tmux kill-session -t maestro-test 2>/dev/null || true
  kill "$provider" 2>/dev/null || true
}
trap cleanup EXIT
for _ in $(seq 100); do [ -s "$root/port" ] && break; sleep 0.1; done
port=$(cat "$root/port")
printf '{"providers":{"startup":{"baseUrl":"http://127.0.0.1:%s/v1","apiKey":"controlled","api":"openai-completions","models":[{"id":"controlled","contextWindow":200000,"maxTokens":8192}]}}}\n' "$port" > "$root/agent/models.json"

# Poll the pane for a marker; fail with the last capture after the bound.
wait_for() {
  local marker=$1 name=$2
  for _ in $(seq 300); do
    tmux capture-pane -t maestro-test -p > "$out/$name.txt"
    if grep -qF -- "$marker" "$out/$name.txt"; then return 0; fi
    sleep 0.2
  done
  echo "missing marker '$marker'" >&2
  cat "$out/$name.txt" >&2
  return 1
}

tmux new-session -d -s maestro-test -x 80 -y 24 -c "$root/project" \
  env -i HOME="$root" PATH="$PATH" TERM=xterm-256color MAESTRO_CODING_AGENT_DIR="$root/agent" MAESTRO_OFFLINE=1 \
  "$binary" --provider startup --model controlled
wait_for controlled startup
tmux send-keys -t maestro-test 'controlled fixture prompt' Enter
wait_for walkthrough-reply-42 reply
tmux send-keys -t maestro-test Escape
tmux send-keys -t maestro-test C-o
sleep 1
tmux capture-pane -t maestro-test -p > "$out/keys.txt"
grep -c 'controlled fixture prompt' "$out/provider-requests.jsonl"
session=$(find "$root/agent/sessions" -name '*.jsonl' -print -quit)
echo "SESSION $session"
grep -F walkthrough-reply-42 "$session" > /dev/null
echo 'WALKTHROUGH passed: startup screen, prompt sent, reply rendered, session persisted'
for name in startup reply keys; do echo "--- $name"; cat "$out/$name.txt"; done
