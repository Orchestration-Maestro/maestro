#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Check build-time resolution overrides without compiling external game code."""
import importlib.util
import os
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest.mock import patch

sys.dont_write_bytecode = True

HERE = Path(__file__).resolve().parent
BUILDER = HERE.parents[3] / "examples/extensions/doom-overlay/build_engine.py"


class Resolution(unittest.TestCase):
    """The compiler receives the selected, validated framebuffer dimensions."""

    def build(self, environment):
        """Run the real recipe with only native tool invocations replaced."""
        spec = importlib.util.spec_from_file_location("doom_builder", BUILDER)
        assert spec is not None and spec.loader is not None
        builder = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(builder)
        with TemporaryDirectory(prefix="maestro-doom-build-check-") as directory:
            root = Path(directory)
            (root / "doomgeneric").mkdir()
            (root / "doomgeneric/Makefile").write_text("SRC_DOOM = game.o doomgeneric_xlib.o\n")
            args = [str(BUILDER), "--source", str(root), "--output", str(root / "game.wasm")]
            with patch.dict(os.environ, environment, clear=True), patch.object(sys, "argv", args), \
                    patch.object(builder.shutil, "which", return_value="clang"), \
                    patch.object(builder.subprocess, "check_output", return_value=builder.PIN), \
                    patch.object(builder.subprocess, "run") as run:
                builder.main()
                return run.call_args_list[1].args[0]

    def test_default_resolution(self):
        """No override keeps the original 640 by 400 default."""
        command = self.build({"WASI_SDK_PATH": "/sdk"})
        self.assertIn("-DDOOMGENERIC_RESX=640", command)
        self.assertIn("-DDOOMGENERIC_RESY=400", command)

    def test_nondefault_resolution(self):
        """Both documented environment overrides reach the compiler."""
        command = self.build({"WASI_SDK_PATH": "/sdk", "DOOM_RESX": "320", "DOOM_RESY": "200"})
        self.assertIn("-DDOOMGENERIC_RESX=320", command)
        self.assertIn("-DDOOMGENERIC_RESY=200", command)

    def test_invalid_resolution(self):
        """Invalid and unrepresentable frame sizes stop before invoking a tool."""
        for width, height in [("0", "400"), ("-1", "400"), ("abc", "400"), ("65536", "65536")]:
            with self.subTest(width=width, height=height), self.assertRaises(SystemExit):
                self.build({"WASI_SDK_PATH": "/sdk", "DOOM_RESX": width, "DOOM_RESY": height})


if __name__ == "__main__":
    unittest.main()
