/* bool: conversion is a test against zero (6.3.1.2), not a truncation — 256
   has an all-zero low byte but still converts to 1. `true`/`false` are
   constants of type bool (6.4.4.6). */
bool is_pos(int x) {
    return x > 0;
}

bool squash(long x) {
    return x; /* the return converts long to bool */
}

int pass_through(bool b) {
    return b;
}

int main(void) {
    bool b = 5; /* constant conversion folds to 1 */
    if (b != 1) {
        return 1;
    }
    int big = 256;
    b = big; /* runtime conversion: low byte is 0, the value isn't */
    if (b != 1) {
        return 2;
    }
    b = big - 256; /* actually zero */
    if (b != 0) {
        return 3;
    }
    if (true != 1 || false != 0) {
        return 4;
    }
    /* promotion: bool arithmetic happens at int width */
    b = true;
    if (b + b != 2) {
        return 5;
    }
    /* 2^32 through the return conversion: truncation would say false */
    if (!squash(4294967296L)) {
        return 6;
    }
    if (!is_pos(3) || is_pos(-3)) {
        return 7;
    }
    if (pass_through(true) != 1) {
        return 8;
    }
    b = -1; /* negative but nonzero */
    if (b != 1) {
        return 9;
    }
    /* ++/-- and compound assignment write back through the bool conversion */
    b = false;
    b--; /* (bool)(0 - 1): nonzero, so true */
    if (b != 1) {
        return 10;
    }
    b = false;
    b |= 256; /* (bool)(0 | 256): truncation would say false */
    if (b != 1) {
        return 11;
    }
    return 70;
}
