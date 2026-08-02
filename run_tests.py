#!/usr/bin/env python3
"""Integration test suite: run `run.py` over every C file under c_test_files/
and print one green PASS / red FAIL per file, plus a summary.

Every file is run the same way — a test passes iff our compiler's output
matches gcc. Programs that use a not-yet-supported feature live under tbd/ and
naturally FAIL (our compiler rejects them); the directory and the one-line
reason ("our compiler rejected it") make it obvious that's an unimplemented
feature, not a regression. A real regression shows a different reason
("behaviour differs from gcc").

This is the seed of the regression suite: as the compiler grows, cases found to
miscompile (e.g. while fuzzing against gcc) get dropped in here so a fix can't
silently regress later, and tbd/ cases graduate to a feature directory once
they pass.

Exit status is 0 iff every file passed.
"""

import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TESTS = ROOT / "c_test_files"
RUNNER = ROOT / "run.py"

# One-line reason per run.py failure exit code (see run.py's header).
REASONS = {
    1: "runner usage error",
    2: "gcc rejected the reference program",
    3: "our compiler rejected it",
    4: "linking our object failed",
    5: "behaviour differs from gcc",
}

# Enter the dev shell once for the whole suite; run.py then inherits REFCC and
# won't re-enter per file.
if "REFCC" not in os.environ:
    os.execvp("nix", ["nix", "develop", str(ROOT), "--command", str(__file__), *sys.argv[1:]])

if sys.stdout.isatty() and "NO_COLOR" not in os.environ:
    GREEN, RED, DIM, OFF = "\033[1;32m", "\033[1;31m", "\033[2m", "\033[0m"
else:
    GREEN = RED = DIM = OFF = ""


def main() -> int:
    files = sorted(TESTS.rglob("*.c"))
    if not files:
        print(f"no test files under {TESTS}", file=sys.stderr)
        return 1

    passed, failed = 0, 0
    for f in files:
        rel = f.relative_to(TESTS)
        code = subprocess.run(
            [sys.executable, str(RUNNER), str(f)],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        ).returncode
        if code == 0:
            print(f"{GREEN}PASS{OFF} {rel}")
            passed += 1
        else:
            reason = REASONS.get(code, f"run.py exit {code}")
            print(f"{RED}FAIL{OFF} {rel}  {DIM}{reason}{OFF}")
            failed += 1

    print()
    if failed:
        print(f"{passed} passed, {RED}{failed} failed{OFF} of {len(files)}")
        print(f"{DIM}(run ./run.py <file> for the full diff of a failure){OFF}")
        return 1
    print(f"{GREEN}all {len(files)} passed{OFF}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
