#define _GNU_SOURCE
#include <errno.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <unistd.h>

static uint64_t compute(uint64_t count) {
    uint64_t x = 0x123456789abcdefULL;
    for (uint64_t i = 0; i < count; i++) {
        x ^= x << 13; x ^= x >> 7; x ^= x << 17;
    }
    return x;
}

int main(int argc, char **argv) {
    if (argc != 3) return 125;
    unsigned mode = (unsigned)strtoul(argv[1], NULL, 10);
    uint64_t count = strtoull(argv[2], NULL, 10);
    if (mode == 1) {
        for (int i = 0; i < 2; i++) {
            pid_t pid = fork();
            if (pid < 0) return 126;
            if (!pid) { volatile uint64_t x = compute(count); _exit(x == 0); }
        }
        for (int i = 0; i < 2; i++) {
            int status;
            if (wait(&status) < 0 || !WIFEXITED(status) || WEXITSTATUS(status)) return 127;
        }
        puts("children-completed");
    } else if (mode == 2) {
        size_t size = 32 * 1024 * 1024;
        unsigned char *data = malloc(size);
        if (!data) return 124;
        for (size_t i = 0; i < size; i++) data[i] = (unsigned char)i;
        uint64_t x = 1;
        for (uint64_t i = 0; i < count; i++) {
            x = (x * 1664525 + 1013904223) & (size - 1);
            data[x]++; x ^= data[x];
        }
        printf("%llu\n", (unsigned long long)x);
        free(data);
    } else if (mode == 3) {
        abort();
    } else {
        printf("%llu\n", (unsigned long long)compute(count));
    }
    return 0;
}
