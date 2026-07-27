#define _POSIX_C_SOURCE 200809L

#include <stdio.h>
#include <string.h>

#ifdef _WIN32
#include <fcntl.h>
#include <io.h>
#include <windows.h>
static void nomo_fixture_sleep(unsigned long millis) { Sleep((DWORD)millis); }
static void nomo_fixture_binary_stdio(void) {
    _setmode(_fileno(stdin), _O_BINARY);
    _setmode(_fileno(stdout), _O_BINARY);
    _setmode(_fileno(stderr), _O_BINARY);
}
#else
#include <time.h>
static void nomo_fixture_sleep(unsigned long millis) {
    struct timespec duration;
    duration.tv_sec = (time_t)(millis / 1000UL);
    duration.tv_nsec = (long)(millis % 1000UL) * 1000000L;
    while (nanosleep(&duration, &duration) != 0) {
    }
}
static void nomo_fixture_binary_stdio(void) {}
#endif

static int echo_once(void) {
    char line[4096];
    if (fgets(line, sizeof(line), stdin) == NULL) {
        return 3;
    }
    printf("async:%s", line);
    fflush(stdout);
    return 0;
}

static int echo_pipe(void) {
    char line[4096];
    while (fgets(line, sizeof(line), stdin) != NULL) {
        printf("O:%s", line);
        fprintf(stderr, "E:%s", line);
        fflush(stdout);
        fflush(stderr);
    }
    return ferror(stdin) ? 4 : 0;
}

int main(int argc, char **argv) {
    nomo_fixture_binary_stdio();
    if (argc < 2) {
        return 2;
    }
    if (strcmp(argv[1], "hold") == 0) {
        nomo_fixture_sleep(10000UL);
        return 0;
    }
    if (strcmp(argv[1], "async") == 0) {
        return echo_once();
    }
    if (strcmp(argv[1], "pipe") == 0) {
        return echo_pipe();
    }
    return 2;
}
