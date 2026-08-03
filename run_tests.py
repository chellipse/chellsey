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

The compiler is built once up front, then files are tested in parallel (`-j`,
default: CPU count).

Exit status is 0 iff every file passed.
"""

import argparse
import concurrent.futures
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
TESTS = ROOT / "c_test_files"
RUNNER = ROOT / "run.py"
BIN = ROOT / "target" / "debug" / "chells-c-compiler"

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


def run_one(f: Path):
    """Test one file through run.py; return (path-relative-to-TESTS, exit code)."""
    code = subprocess.run(
        [sys.executable, str(RUNNER), str(f)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
    ).returncode
    return f.relative_to(TESTS), code


def main() -> int:
    ap = argparse.ArgumentParser(description="run the C integration suite")
    ap.add_argument(
        "-j", "--jobs", type=int, default=os.cpu_count() or 1,
        help="number of files to test in parallel (default: CPU count)",
    )
    args = ap.parse_args()

    files = sorted(TESTS.rglob("*.c"))
    if not files:
        print(f"no test files under {TESTS}", file=sys.stderr)
        return 1

    # Build the compiler once, up front, so the parallel workers invoke the
    # finished binary directly (via CC_BIN) instead of each racing `cargo run`
    # on the shared target/ build lock.
    print("building compiler ...")
    if subprocess.run(["cargo", "build", "--quiet"]).returncode != 0 or not BIN.is_file():
        print("compiler build failed", file=sys.stderr)
        return 1
    os.environ["CC_BIN"] = str(BIN)

    passed, failed = 0, 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        # Submit in sorted order and read results back in that same order, so
        # output stays stable and streams as the in-order frontier completes.
        for fut in [pool.submit(run_one, f) for f in files]:
            rel, code = fut.result()
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
