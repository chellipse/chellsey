/* `>>` on a signed value is arithmetic: -16 >> 2 = -4 (sign bit replicated),
   exit status 256 - 4 = 252. */
int main(void) { return -16 >> 2; }
