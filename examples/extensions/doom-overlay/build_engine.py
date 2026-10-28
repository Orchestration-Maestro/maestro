#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Build a separate headless engine from an external pinned source checkout."""
import argparse
import os
from pathlib import Path
import re
import shutil
import subprocess
from tempfile import TemporaryDirectory

PIN = "e257a09527062bfb986b6e5e0780839b6cbd5b71"


def main():
    """Keep third-party source and composed artifacts outside the shipped workspace."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    options = parser.parse_args()
    here = Path(__file__).resolve().parent
    repo = here.parents[2]
    source = options.source.resolve()
    output = options.output.resolve()
    if source.is_relative_to(repo) or output.is_relative_to(repo):
        raise SystemExit("Keep engine source and output outside the shipped workspace.")
    sdk = os.environ.get("WASI_SDK_PATH")
    compiler = shutil.which("clang", path=str(Path(sdk) / "bin")) if sdk else None
    if not compiler:
        raise SystemExit("Set WASI_SDK_PATH to wasi-sdk 34.")
    revision = subprocess.check_output(["git", "-C", str(source), "rev-parse", "HEAD"], text=True).strip()
    if revision != PIN:
        raise SystemExit(f"Check out engine revision {PIN} before building.")
    engine = source / "doomgeneric"
    objects = re.search(r"^SRC_DOOM\s*=\s*(.*)$", (engine / "Makefile").read_text(), re.MULTILINE)
    if objects is None:
        raise SystemExit("The pinned engine has no source object list.")
    sources = [str(engine / (name[:-2] + ".c")) for name in objects.group(1).split() if name != "doomgeneric_xlib.o"]
    output.parent.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(prefix="maestro-doom-build-") as directory:
        work = Path(directory)
        subprocess.run(["wit-bindgen", "c", "--world", "game", "--out-dir", str(work), str(here / "engine.wit")], check=True)
        wraps = ["I_Error", "exit", "printf", "puts", "putchar", "fprintf", "vfprintf", "fwrite"]
        subprocess.run([compiler, "--target=wasm32-wasip2", "-mexec-model=reactor",
                        "-std=gnu17", "-O2", "-DNDEBUG", "-DNORMALUNIX", "-D_DEFAULT_SOURCE", "-fno-builtin",
                        "-DDOOMGENERIC_RESX=640", "-DDOOMGENERIC_RESY=400",
                        "-mllvm", "-wasm-enable-sjlj", "-mllvm", "-wasm-use-legacy-eh=false",
                        f"-ffile-prefix-map={work}=.", f"-ffile-prefix-map={repo}=.", f"-ffile-prefix-map={source}=.",
                        f"-I{here}", f"-I{work}", f"-I{engine}", *sources,
                        str(here / "engine_glue.c"), str(here / "engine_output.c"),
                        str(work / "game.c"), str(work / "game_component_type.o"),
                        "-lm", "-lsetjmp", *[f"-Wl,--wrap={name}" for name in wraps], "-o", str(work / "raw.wasm")], check=True)
        subprocess.run(["wasm-tools", "strip", str(work / "raw.wasm"), "-o", str(output)], check=True)
        subprocess.run(["wasm-tools", "validate", str(output)], check=True)
    print(output)


if __name__ == "__main__":
    main()
