/* Conversions at every assignment-shaped site: initializers, assignment's
   own value, compound assignment computing in the common type, ++ on an
   unsigned at its wrap point, and the ternary balancing its arms. */
unsigned bump(unsigned v) {
    return v + 1;
}

int main(void) {
    /* assignment value is the *converted* value */
    int t;
    long assigned = (t = 4294967338L); /* t = 42, and the value is 42 */
    if (assigned != 42 || t != 42) {
        return 1;
    }
    /* compound assignment computes in the common type: i is widened to
       long for the multiply, so no 32-bit wrap, then converts back */
    long acc = 100000;
    acc *= 100000; /* 1e10, exact */
    if (acc / 1000000000L != 10) {
        return 2;
    }
    /* ++ wraps an unsigned at 2^32 - 1 like `+= 1` would */
    unsigned u = 4294967295u;
    u++;
    if (u != 0u) {
        return 3;
    }
    /* a call converts arguments and re-extends the returned value */
    if (bump(4294967295u) != 0u) {
        return 4;
    }
    /* ?: balances int/long arms to long */
    int pick = 1;
    long merged = pick ? 42 : 4294967296L;
    return merged; /* 42 */
}
