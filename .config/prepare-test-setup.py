#!/usr/bin/env python3
"""Stage reusable component files, or export their relocated paths to nextest."""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def prepare(output):
    """Run the existing component-discovery owner in a separate native build."""
    output.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory() as temporary:
        environment = Path(temporary) / "environment"
        env = {key: value for key, value in os.environ.items()
               if not re.fullmatch(r"MAESTRO_[A-Z0-9_]+_COMPONENT", key)}
        env["NEXTEST_ENV"] = str(environment)
        subprocess.run([
            os.environ.get("CARGO", "cargo"), "run", "--quiet", "--locked",
            "-p", "maestro-extensions-wasm", "--example", "prepare_author_component",
            "--target-dir", os.environ.get("CARGO_TARGET_DIR", "target/test-prepare"),
        ], env=env, check=True)
        entries = {}
        for line in environment.read_text().splitlines():
            variable, source = line.split("=", 1)
            if not re.fullmatch(r"MAESTRO_[A-Z0-9_]+_COMPONENT", variable) or variable in entries:
                raise ValueError(f"invalid or duplicate component export: {variable}")
            name = variable + ".wasm"
            shutil.copyfile(source, output / name)
            entries[variable] = name
        if not {"MAESTRO_AUTHOR_COMPONENT", "MAESTRO_CONTEXT_COMPONENT"} <= entries.keys():
            raise ValueError("preparation did not export the author and context components")
        (output / "components.json").write_text(json.dumps(entries, sort_keys=True))


def export(output):
    """Export only staged component paths, with no compiler or native host."""
    entries = json.loads((output / "components.json").read_text())
    lines = []
    for variable, name in entries.items():
        if not re.fullmatch(r"MAESTRO_[A-Z0-9_]+_COMPONENT", variable) or name != variable + ".wasm":
            raise ValueError(f"invalid prepared component entry in {output}: {variable}")
        path = output / name
        if not path.is_file():
            raise ValueError(f"missing prepared component: {path}")
        lines.append(f"{variable}={path}\n")
    Path(os.environ["NEXTEST_ENV"]).write_text("".join(lines))


if __name__ == "__main__":
    try:
        directory = Path(os.environ["TEST_PREPARE_OUTPUT"]).resolve()
        if sys.argv[1:] == ["prepare"]:
            prepare(directory)
        elif sys.argv[1:] == ["export"]:
            export(directory)
        else:
            raise ValueError("usage: prepare-test-setup.py prepare|export")
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        sys.exit(f"test setup: {error}")
