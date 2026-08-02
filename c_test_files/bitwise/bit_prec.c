/* Precedence, tightest to loosest: + > & > ^ > |.
   3 + 4 = 7; 6 & 7 = 6; 5 ^ 6 = 3; 16 | 3 = 19. */
int main(void) { return 16 | 5 ^ 6 & 3 + 4; }
