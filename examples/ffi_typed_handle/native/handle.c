#include <stdlib.h>

typedef struct NomoFileHandle {
    int marker;
} NomoFileHandle;

void *nomo_file_open(void) {
    NomoFileHandle *handle = malloc(sizeof(NomoFileHandle));
    if (handle != NULL) {
        handle->marker = 42;
    }
    return handle;
}

void *nomo_file_try_open(int should_open) {
    return should_open ? nomo_file_open() : NULL;
}

int nomo_file_marker(void *handle) {
    NomoFileHandle *file = handle;
    return file == NULL ? -1 : file->marker;
}

void nomo_file_close(void *handle) {
    free(handle);
}

int nomo_apply_callback(int value, int (*callback)(int)) {
    return callback(value);
}
