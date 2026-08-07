/* Explicit casts (6.5.4): the same conversions assignment performs, spelled
   at an expression position — narrowing re-extension, same-width sign flips,
   and the bool `!= 0` rule. */
int main(void) {
    if ((char)300 != 44) { /* narrow to the low byte, sign-extended */
        return 1;
    }
    if ((unsigned char)-1 != 255) { /* same width, zero-extended */
        return 2;
    }
    if ((short)70000 != 4464) {
        return 3;
    }
    int n = 300; /* runtime, not const-folded */
    if ((char)n != 44) {
        return 4;
    }
    /* (unsigned)-1 is UINT_MAX: as a u32 pattern it compares above any
       small signed value under the unsigned comparison UAC picks */
    unsigned u = (unsigned)-1;
    if (u < 5u) {
        return 5;
    }
    /* widening: the sign travels (int -1 -> long -1) */
    long l = (long)n - 301; /* -1 */
    if ((int)l != -1) {
        return 6;
    }
    /* casting to bool tests against zero, it does not truncate */
    if ((bool)256 != 1) {
        return 7;
    }
    int big = 512;
    if ((bool)big != 1) {
        return 8;
    }
    /* nesting: innermost first — (char)300 is 44, then widened back */
    if ((long)(char)300 != 44) {
        return 9;
    }
    /* precedence: the cast binds the operand, not the sum */
    if ((char)n + 1 != 45) {
        return 10;
    }
    /* a cast in an argument and in a condition */
    if ((short)(65536 + 7) != 7) {
        return 11;
    }
    return 70;
}
