/* Cross-level precedence and left-associativity over the binary tower:
   * / % tighter than + -, tighter than << >>, tighter than the bitwise
   & ^ | group. gcc is the oracle for the exact fold. */
int main(void) {
    return 1 + 2 * 3 - 8 / 4 % 3 /* * / % before + - */
        + (20 - 4 - 3)          /* left-assoc subtraction */
        + (2 + 1 << 2)          /* + before << */
        + (100 >> 3 >> 1)       /* left-assoc shift */
        + (6 & 3 | 8 ^ 1);      /* & then ^ then | */
}
