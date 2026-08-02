/* Kitchen-sink expression: arithmetic (+ - * / %), unary minus, bitwise
   (& | ^ ~), shifts (<< >>), comparisons (< > <= >= == !=), logical !, and
   decimal/hex/octal literals, all in one returned expression with parentheses
   forcing precedence. Grows as operators/features are added. */
int main(void) {
    return (7 * 6 - 100 / 9) % 40 /* arithmetic + % */
        + (((1 << 4) | 5) & 0x1E) /* shift, |, & */
        + (~0xF0 & 0xFF)          /* ~ then mask -> 0x0F */
        - (3 ^ 6)                 /* xor */
        + (010 >> 1)              /* octal 8 >> 1 */
        + (5 <= 5) + (9 > 2)      /* relational */
        + (4 == 4) + (7 != 8)     /* equality */
        + !0 - !5                 /* logical not */
        - -3;                     /* unary minus */
}
