/* Right associativity: a ? b : c ? d : e is a ? b : (c ? d : e). The
   classic banding chain, swept over its inputs. */
int main(void) {
    int sum = 0;
    for (int score = 0; score <= 100; score = score + 10) {
        sum = sum + (score >= 90 ? 4 : score >= 70 ? 3 : score >= 50 ? 2 : 1);
    }
    /* 0..40 -> 1 (x5), 50,60 -> 2 (x2), 70,80 -> 3 (x2), 90,100 -> 4 (x2) */
    return sum; /* 5 + 4 + 6 + 8 = 23 */
}
