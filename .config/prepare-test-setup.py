#!/usr/bin/env python3
"""Stage reusable component files and the setup example, or run that example on them."""

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

EXAMPLE = "prepare_author_component"


def inputs():
    """Hash source contents and names, not checkout paths or transfer timestamps."""
    files = subprocess.check_output([
        "git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--",
        "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates",
        "examples", "assets", ".config/prepare-test-setup.py",
        ".config/prepare-author-component.sh",
    ]).split(b"\0")
    digest = hashlib.sha256()
    for name in sorted(set(files) - {b""}):
        path = Path(os.fsdecode(name))
        digest.update(name + b"\0")
        digest.update(hashlib.sha256(path.read_bytes()).digest() if path.is_file() else b"missing")
    return digest.hexdigest()


def prepare(output):
    """Run the existing component-discovery owner in a separate native build."""
    output.mkdir(parents=True, exist_ok=True)
    manifest = output / "components.json"
    manifest.unlink(missing_ok=True)
    fingerprint = inputs()
    with tempfile.TemporaryDirectory() as temporary:
        environment = Path(temporary) / "environment"
        env = {key: value for key, value in os.environ.items()
               if not re.fullmatch(r"MAESTRO_[A-Z0-9_]+_COMPONENT", key)}
        env["NEXTEST_ENV"] = str(environment)
        target = os.environ.get("CARGO_TARGET_DIR", "target/test-prepare")
        subprocess.run([
            os.environ.get("CARGO", "cargo"), "run", "--quiet", "--locked",
            "-p", "maestro-extensions-wasm", "--example", EXAMPLE, "--target-dir", target,
        ], env=env, check=True)
        entries = {}
        for line in environment.read_text().splitlines():
            variable, source = line.split("=", 1)
            if not re.fullmatch(r"MAESTRO_[A-Z0-9_]+_COMPONENT", variable) or variable in entries:
                raise ValueError(f"invalid or duplicate component export: {variable}")
            name = variable + ".wasm"
            shutil.copyfile(source, output / name)
            entries[variable] = {
                "file": name,
                "sha256": hashlib.sha256((output / name).read_bytes()).hexdigest(),
            }
        if not {"MAESTRO_AUTHOR_COMPONENT", "MAESTRO_CONTEXT_COMPONENT"} <= entries.keys():
            raise ValueError("preparation did not export the author and context components")
        if inputs() != fingerprint:
            raise ValueError(f"inputs changed during preparation: {output}")
        shutil.copyfile(Path(target) / "debug/examples" / EXAMPLE, output / EXAMPLE)
        (output / EXAMPLE).chmod(0o755)
        manifest.write_text(json.dumps({
            "inputs": fingerprint,
            "components": entries,
            "example": hashlib.sha256((output / EXAMPLE).read_bytes()).hexdigest(),
        }, sort_keys=True))


def export(output, directory):
    """Validate the staged files, install the example and write the component paths."""
    manifest = json.loads((output / "components.json").read_text())
    if manifest["inputs"] != inputs():
        raise ValueError(f"stale prepared component inputs: {output}")
    entries = manifest["components"]
    if not {"MAESTRO_AUTHOR_COMPONENT", "MAESTRO_CONTEXT_COMPONENT"} <= entries.keys():
        raise ValueError(f"missing prepared author or context component: {output}")
    lines = []
    for variable, entry in entries.items():
        name = entry["file"]
        if not re.fullmatch(r"MAESTRO_[A-Z0-9_]+_COMPONENT", variable) or name != variable + ".wasm":
            raise ValueError(f"invalid prepared component entry in {output}: {variable}")
        path = output / name
        if not path.is_file():
            raise ValueError(f"missing prepared component: {path}")
        if hashlib.sha256(path.read_bytes()).hexdigest() != entry["sha256"]:
            raise ValueError(f"stale prepared component bytes: {path}")
        lines.append(f"{variable}={path}\n")
    staged = output / EXAMPLE
    if not staged.is_file() or hashlib.sha256(staged.read_bytes()).hexdigest() != manifest["example"]:
        raise ValueError(f"missing or stale prepared setup example: {staged}")
    installed = directory / "examples" / EXAMPLE
    installed.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(staged, installed)
    installed.chmod(0o755)
    Path(os.environ["NEXTEST_ENV"]).write_text("".join(lines))


if __name__ == "__main__":
    try:
        destination = os.environ["TEST_PREPARE_OUTPUT"]
        if not destination or "\n" in destination or "\r" in destination:
            raise ValueError("TEST_PREPARE_OUTPUT needs a nonempty, single-line path")
        directory = Path(destination).resolve()
        if sys.argv[1:] == ["prepare"]:
            prepare(directory)
        elif len(sys.argv) == 3 and sys.argv[1] == "export":
            export(directory, Path(sys.argv[2]))
        else:
            raise ValueError("usage: prepare-test-setup.py prepare | export ARTIFACT_DIRECTORY")
    except (OSError, ValueError, KeyError, subprocess.CalledProcessError) as error:
        sys.exit(f"test setup: {error}")
