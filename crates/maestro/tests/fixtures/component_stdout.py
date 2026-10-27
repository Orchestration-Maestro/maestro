"""Exercise the shipped loader while its real terminal output is blocked."""
import json
import os
import pty
import socket
import subprocess
import sys

binary, component, generation, endpoint = sys.argv[1:]
init = b"INIT\x00\xff\x1b[0m"
payload = b"\x1b[36m" + b"guest\x00\xff" * 300_000 + b"\x1b[0m"
environment = dict(os.environ, MAESTRO_WASI_ROOT_OUTPUT=endpoint)
environment["MAESTRO_WASI_STARTUP_OUTPUT"] = json.dumps(
    {"generation": generation, "bytes": list(init)}
)
master, slave = pty.openpty()
listener = socket.socket(socket.AF_UNIX)
listener.bind(endpoint)
listener.listen()
process = subprocess.Popen(
    [binary, "--offline", "--extension", component],
    stdin=slave, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=environment,
)
assert process.stdout is not None
assert process.stderr is not None
stdout, stderr = process.stdout, process.stderr
os.close(slave)
output = bytearray()


def through(marker):
    """Wait for an actual terminal observation, never a startup sleep."""
    while True:
        byte = stdout.read(1)
        assert byte, (process.poll(), bytes(output), stderr.read())
        output.extend(byte)
        if output.endswith(marker):
            return


try:
    through(b"ROOT-STDOUT-READY\x07")
    through(b"\x1b[?2026l")
    assert init in output
    for attempt in range(2):
        os.write(master, f"/stdout-probe {generation}\r".encode())
        # Do not drain stdout until the guest has finished its write attempt.
        connection, _ = listener.accept()
        with connection:
            reply = bytearray()
            while chunk := connection.recv(4096):
                reply.extend(chunk)
        through(b"ROOT-STDOUT-DONE\x07")
        observed = json.loads(reply)
        assert observed == {"terminal": False}, observed
        assert output.count(payload) == attempt + 1, "guest byte write was split or lost"
        if attempt == 0:
            os.write(master, b"/stdout-probe reload\r")
            through(b"ROOT-STDOUT-READY\x07")
            assert output.count(init) == 2, "reload did not retain current stdout routing"
    os.write(master, b"/quit\r")
    output.extend(stdout.read())
    assert process.wait() == 0, stderr.read()
    # Each synchronized host frame remains whole, with no guest bytes inside it.
    for frame in bytes(output).split(b"\x1b[?2026h")[1:]:
        body, _ = frame.split(b"\x1b[?2026l", 1)
        assert b"guest\x00\xff" not in body, "guest output split a host frame"
    assert b"\x1b[?2004l" in output
finally:
    if process.poll() is None:
        process.kill()
        process.wait()
    listener.close()
    os.close(master)
