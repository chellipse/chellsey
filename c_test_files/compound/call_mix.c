/* Functions composed with loops, branches, recursion, and the operator
   tower — the whole feature set cooperating across call boundaries. */
int gcd(int a, int b) {
    while (b != 0) {
        int t = a % b;
        a = b;
        b = t;
    }
    return a;
}

int pow2(int e) {
    if (e == 0) {
        return 1;
    }
    return 2 * pow2(e - 1);
}

int clamp(int x, int lo, int hi) {
    return x < lo ? lo : (x > hi ? hi : x);
}

int main(void) {
    int acc = 0;
    for (int i = 1; i <= 6; i = i + 1) {
        acc = acc + gcd(pow2(i), 24) * clamp(i, 2, 4);
    }
    /* gcd(2,24)=2*2 + gcd(4,24)=4*2 + gcd(8,24)=8*3 + gcd(16,24)=8*4
       + gcd(32,24)=8*4 + gcd(64,24)=8*4 = 4+8+24+32+32+32 = 132 */
    return acc + (gcd(0, 7) == 7); /* gcd(0,7)=7 -> +1 = 133 */
}
