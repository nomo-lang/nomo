#include <stdlib.h>

void *nomo_example_allocate(void) {
    return malloc(1);
}

void nomo_example_release(void *handle) {
    free(handle);
}
