#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Regenerate the controlled engine components or verify its committed bytes."""
import argparse
import os
import shutil
import subprocess
from pathlib import Path
from tempfile import TemporaryDirectory


def main():
    """Use the configured native SDK; ordinary tests need only the fixture."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    check = parser.parse_args().check
    sdk = os.environ.get("WASI_SDK_PATH")
    if not sdk or not (Path(sdk) / "bin/clang").is_file():
        message = "set WASI_SDK_PATH to wasi-sdk 34 to rebuild the engine fixture"
        if check:
            print(f"SKIP: {message}.")
            return
        raise SystemExit(message)
    here = Path(__file__).resolve().parent
    repo = here.parents[3]
    with TemporaryDirectory(prefix="maestro-doom-fixture-") as directory:
        work = Path(directory)
        subprocess.run(["wit-bindgen", "c", "--world", "game", "--out-dir", str(work),
                        str(repo / "examples/extensions/doom-overlay/engine.wit")], check=True)
        for name in ("doom_files_engine", "doom_sjlj_engine"):
            source = work / f"{name}.c"
            fixture = here / f"{name.replace('_', '-')}.wasm"
            shutil.copyfile(here / source.name, source)
            flags = []
            if name == "doom_sjlj_engine":
                flags = ["-mllvm", "-wasm-enable-sjlj", "-mllvm",
                         "-wasm-use-legacy-eh=false", "-lsetjmp", "-Wl,--wrap=I_Error", "-Wl,--wrap=exit"]
            subprocess.run([str(Path(sdk) / "bin/clang"), "--target=wasm32-wasip2",
                            "-mexec-model=reactor", "-O2", "-DNDEBUG", *flags,
                            f"-ffile-prefix-map={work}=.", f"-ffile-prefix-map={repo}=.",
                            f"-I{work}", f"-I{repo / 'examples/extensions/doom-overlay'}", str(source), str(work / "game.c"),
                            str(work / "game_component_type.o"), "-o", str(work / "raw.wasm")], check=True)
            output = work / fixture.name
            subprocess.run(["wasm-tools", "strip", str(work / "raw.wasm"), "-o", str(output)], check=True)
            if check:
                if output.read_bytes() != fixture.read_bytes():
                    raise SystemExit(f"{fixture.name} differs. Regenerate with the documented tool versions.")
                print(f"PASS: {fixture.name} matches its source byte for byte.")
            else:
                shutil.copyfile(output, fixture)



if __name__ == "__main__":
    main()
