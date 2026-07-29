/* Non-decisional semantic-C diagnostic for the frozen fannkuch-redux workload.
 *
 * BSD-3-Clause derivative of fannkuchredux-gcc-8 and the Nomo transliteration.
 * This control uses u64/i64 values plus heap-backed, length-carrying arrays and
 * checked indexing. It is not a C/C++ comparator.
 */

#include <inttypes.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>

typedef struct {
    uint64_t *data;
    size_t len;
} u64_array;

static void bounds_fail(void)
{
    fputs("semantic-c array bounds failure\n", stderr);
    abort();
}

static u64_array array_new(size_t len)
{
    u64_array array = {calloc(len, sizeof(uint64_t)), len};
    if (array.data == NULL && len != 0)
        abort();
    return array;
}

static uint64_t array_get(const u64_array *array, size_t index)
{
    if (index >= array->len)
        bounds_fail();
    return array->data[index];
}

static void array_set(u64_array *array, size_t index, uint64_t value)
{
    if (index >= array->len)
        bounds_fail();
    array->data[index] = value;
}

static uint64_t fannkuch(uint64_t n)
{
    u64_array perm1 = array_new((size_t)n);
    u64_array perm = array_new((size_t)n);
    u64_array count = array_new((size_t)n);
    for (uint64_t i = 0; i < n; ++i)
        array_set(&perm1, (size_t)i, i);
    uint64_t flips_count = 0;
    uint64_t permutation_index = 0;
    int64_t checksum = 0;
    uint64_t r = n;
    bool done = false;
    while (!done) {
        uint64_t i = 0;
        while (r != 1) {
            array_set(&count, (size_t)(r - 1), r);
            --r;
        }
        while (i < n) {
            array_set(&perm, (size_t)i, array_get(&perm1, (size_t)i));
            ++i;
        }
        uint64_t flips = 0;
        uint64_t k = array_get(&perm, 0);
        while (k != 0) {
            i = 0;
            while (2 * i < k) {
                uint64_t temporary = array_get(&perm, (size_t)i);
                array_set(&perm, (size_t)i, array_get(&perm, (size_t)(k - i)));
                array_set(&perm, (size_t)(k - i), temporary);
                ++i;
            }
            k = array_get(&perm, 0);
            ++flips;
        }
        if (flips > flips_count)
            flips_count = flips;
        checksum += (permutation_index & 1) == 0 ? (int64_t)flips : -(int64_t)flips;
        bool more = true;
        while (more && r < n) {
            uint64_t first = array_get(&perm1, 0);
            i = 0;
            while (i < r) {
                uint64_t next = i + 1;
                array_set(&perm1, (size_t)i, array_get(&perm1, (size_t)next));
                i = next;
            }
            array_set(&perm1, (size_t)r, first);
            array_set(&count, (size_t)r, array_get(&count, (size_t)r) - 1);
            uint64_t remaining = array_get(&count, (size_t)r);
            if (remaining == 0)
                ++r;
            more = remaining == 0;
        }
        done = r == n;
        ++permutation_index;
    }
    printf("%" PRId64 "\n", checksum);
    free(count.data);
    free(perm.data);
    free(perm1.data);
    return flips_count;
}

int main(int argc, char **argv)
{
    uint64_t n = argc > 1 ? strtoull(argv[1], NULL, 10) : 7;
    printf("Pfannkuchen(%" PRIu64 ") = %" PRIu64 "\n", n, fannkuch(n));
    return 0;
}
