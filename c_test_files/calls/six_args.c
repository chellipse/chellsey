/* All six integer argument registers exercised, and out of order so a
   wrong register mapping would show. */
int weigh(int a, int b, int c, int d, int e, int f) {
    return a * 1 + b * 2 + c * 3 + d * 4 + e * 5 + f * 6;
}

int main(void) {
    return weigh(1, 2, 3, 4, 5, 6); /* 1+4+9+16+25+36 = 91 */
}
