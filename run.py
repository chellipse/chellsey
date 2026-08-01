#!/usr/bin/env python3
"""Differential test runner: compile <file.c> both with gcc (the reference)
and with this compiler, run both, and compare exit status + stdout + stderr.

Every stage echoes the exact command it runs and streams that command's
output, followed by a green SUCCESS / red FAILURE status line — so when a
stage breaks (especially our own compiler), its diagnostics are right there.

The reference is *unwrapped* gcc — no NixOS cc-wrapper flag injection. The
dev shell exports REFCC / REFCC_FLAGS (see flake.nix): REFCC_FLAGS only
teaches the unwrapped compiler where glibc and mold live; every other
setting the reference runs with is chosen right here.

Exit codes are distinct per failure stage so callers can tell what broke:
  0  OK (match)
  1  usage error
  2  reference (gcc) failed to compile the source
  3  our compiler failed to produce the .o
  4  linking our .o failed
  5  behavioural mismatch (status/stdout/stderr differ)
"""

import difflib
import os
import shlex
import subprocess
import sys
import tempfile
from pathlib import Path

TIMEOUT = 5  # seconds each compiled binary gets to run

# Invoked outside the dev shell (no REFCC in the environment)? Re-enter it.
if "REFCC" not in os.environ:
    me = Path(__file__).resolve()
    os.execvp("nix", ["nix", "develop", str(me.parent), "--command", str(me), *sys.argv[1:]])

COLOR = sys.stdout.isatty() and "NO_COLOR" not in os.environ
C_HDR, C_OK, C_BAD, C_OFF = (
    ("\033[1;34m", "\033[1;32m", "\033[1;31m", "\033[0m") if COLOR else ("",) * 4
)


def hdr(msg):
    print(f"{C_HDR}==> {msg}{C_OFF}")


def ok(msg):
    print(f"{C_OK}SUCCESS{C_OFF} {msg}")


def bad(msg):
    print(f"{C_BAD}FAILURE{C_OFF} {msg}")


def show(data: bytes, indent: str = "  "):
    """Print captured output, indented, if non-empty."""
    if data:
        for line in data.decode(errors="replace").splitlines():
            print(indent + line)


def stage(fail_code: int, desc: str, cmd: list[str]):
    """Run one stage: echo the command, show its interleaved stdout+stderr,
    then a status line. Exits the script with `fail_code` on failure."""
    hdr(desc)
    print("  $ " + shlex.join(cmd))
    p = subprocess.run(cmd, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    show(p.stdout)
    if p.returncode != 0:
        bad(desc)
        sys.exit(fail_code)
    ok(desc)


def run_binary(name: str, path: Path):
    """Run a compiled binary; report and return (status, stdout, stderr).
    `status` is an exit code, or a descriptive string for signals/timeouts."""
    try:
        p = subprocess.run(
            [str(path)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=TIMEOUT
        )
        st = p.returncode if p.returncode >= 0 else f"signal {-p.returncode}"
        out, err = p.stdout, p.stderr
    except subprocess.TimeoutExpired as e:
        st = f"timeout after {TIMEOUT}s"
        out, err = e.stdout or b"", e.stderr or b""
    print(f"  {name}: exit={st}")
    if out:
        print(f"  {name} stdout:")
        show(out, "    ")
    if err:
        print(f"  {name} stderr:")
        show(err, "    ")
    return st, out, err


def show_diff(label: str, ref: bytes, ours: bytes):
    bad(f"{label} differs:")
    lines = difflib.unified_diff(
        ref.decode(errors="replace").splitlines(),
        ours.decode(errors="replace").splitlines(),
        fromfile=f"ref {label}",
        tofile=f"ours {label}",
        lineterm="",
    )
    for line in lines:
        print("  " + line)


def main() -> int:
    if len(sys.argv) != 2:
        print(f"usage: {sys.argv[0]} <file.c>", file=sys.stderr)
        return 1
    src = Path(sys.argv[1])
    if not src.is_file():
        print(f"no such file: {src}", file=sys.stderr)
        return 1

    refcc = os.environ["REFCC"]
    refcc_flags = shlex.split(os.environ["REFCC_FLAGS"])
    # Where our compiler drops the object: `<stem>.o` next to the source
    # (this is `path.with_extension("o")` in src/main.rs).
    obj = src.with_suffix(".o")

    with tempfile.TemporaryDirectory() as tmp:
        ref_bin = Path(tmp) / "ref"
        our_bin = Path(tmp) / "ours"

        # fmt: off
        stage(2, "reference compile", [
            refcc, *refcc_flags, "-fuse-ld=mold", "-O0", "-g", str(src), "-o", str(ref_bin),
        ])
        stage(3, "our compile", ["cargo", "run", "--quiet", "--", str(src)])
        stage(4, f"link {obj}", [
            refcc, *refcc_flags, "-fuse-ld=mold", "-z", "noexecstack", str(obj), "-o", str(our_bin),
        ])
        # fmt: on

        hdr("run both binaries")
        ref_st, ref_out, ref_err = run_binary("ref", ref_bin)
        our_st, our_out, our_err = run_binary("ours", our_bin)

    hdr("compare behaviour")
    match = True
    if ref_st != our_st:
        bad(f"exit status: ref={ref_st} ours={our_st}")
        match = False
    if ref_out != our_out:
        show_diff("stdout", ref_out, our_out)
        match = False
    if ref_err != our_err:
        show_diff("stderr", ref_err, our_err)
        match = False

    if not match:
        return 5
    ok("behaviour matches (exit status, stdout, stderr)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
