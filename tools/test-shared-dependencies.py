#!/usr/bin/env python3
"""Exercise the standalone guard's actual command against shared syntax fixtures."""
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

TOOLS = Path(__file__).resolve().parent
fixtures = json.loads((TOOLS / "shared-dependency-fixtures.json").read_text())
for fixture in fixtures:
    with tempfile.TemporaryDirectory(prefix="wayscriber-shared-dependencies-") as directory:
        root = Path(directory)
        (root / "tools").mkdir()
        checker = root / "tools/check-shared-dependencies.py"
        shutil.copyfile(TOOLS / checker.name, checker)
        source = root / fixture["path"]
        source.parent.mkdir(parents=True)
        source.write_text(fixture["source"])
        result = subprocess.run(["python3", str(checker)], capture_output=True, text=True)
        expected = 1 if fixture["reject"] else 0
        assert result.returncode == expected, (fixture["name"], result.returncode, result.stderr)
print(f"Standalone shared-dependency syntax fixtures passed ({len(fixtures)}).")
