/* gcc compiles this (returns (int)1.5 == 1); we must reject the float literal
   rather than miscompile it. */
int main(void) { return 1.5; }
