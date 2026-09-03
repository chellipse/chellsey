# CHELLSEY, or CHELL's C Compiler

Based on ISO C Standard N3220 Working
Draft. [link](https://www.open-std.org/jtc1/sc22/wg14/www/docs/n3220.pdf)
Unless stated otherwise, comments referring to clauses / subclauses
refer to N3220.

Although, *Note:* we do not intend to implement exclusively N3220,
because I wanna compile a Real Program. Which real program? Linux.

This leads me to, given Linux wants gnu11, between there and c23, we
get this matrix:

|    | c   | gnu |
|----|-----|-----|
| 11 | ❌  | ❌  |
| 17 | ❌  | ❌  |
| 23 | 🚧  | ❌  |

We currently only support `x86-64`, however I would like to support
`AArch64` as well. For `x64` the blessed path performance-wise will
probably be `x86-64-v3`.

For the currently implemented feature set, you'll wanna look at which
[test files](c_test_files/) are passing by running
[run_tests.py](run_tests.py), a convenience harness around
[run.py](run.py), which diffs artifact execution outputs against gcc.
