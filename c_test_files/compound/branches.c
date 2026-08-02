/* Deeper if/else nesting with fall-through and computed conditions/results,
   stressing CFG construction and block joins alongside mixed operators. */
int main(void) {
    if (3 * 3 > 8) {
        if (7 % 2 == 0) {
            return 100;
        } else {
            if ((1 << 3) <= 8) {
                return (255 ^ 0xF0) & 0x3F;
            }
            return 50;
        }
    }
    return 200;
}
