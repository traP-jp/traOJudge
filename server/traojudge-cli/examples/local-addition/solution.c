#include <stdio.h>

int main(void) {
    int left, right;
    if (scanf("%d %d", &left, &right) != 2) {
        return 1;
    }
    printf("%d\n", left + right);
    return 0;
}
