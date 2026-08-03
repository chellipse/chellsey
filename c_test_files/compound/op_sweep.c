/* Sweep operand pairs and fold every operator's result into one rolling
   hash, so whole truth tables are verified against gcc rather than spot
   values. Kept UB-free: the hash is re-masked to 15 bits each round (no
   signed overflow), shifts only see non-negative in-range operands, and the
   divisor is never zero. Only low-bit-stable ops (* + & | ^ <<) feed the
   hash, so the 8-bit exit code is width-independent. */
int main(void) {
    int h = 0;
    for (int a = -6; a <= 6; a = a + 1) {
        for (int b = 1; b <= 5; b = b + 1) {
            h = (h * 31 + a / b) & 0x7FFF;
            h = (h * 31 + a % b) & 0x7FFF;
            h = (h * 31 + (a * b - (a & b) + (a | b) - (a ^ b) + ~a)) & 0x7FFF;
            h = (h * 31 + ((a < b) + (a <= b) + (a > b) + (a >= b) + (a == b) + (a != b) + !a))
                & 0x7FFF;
            h = (h * 31 + (((a & 31) << b) - ((b << 4) >> 2))) & 0x7FFF;
            h = (h * 31 + ((a < b ? a : b) + (a > b ? a - b : b - a))) & 0x7FFF;
            h = (h * 31 + ((a && b) * 2 + (a || b))) & 0x7FFF;
        }
    }
    return h & 0xFF;
}
