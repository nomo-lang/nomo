/* Non-decisional semantic-C diagnostic for the frozen spectral-norm workload.
 *
 * BSD-3-Clause derivative of spectralnorm-gcc-8 and the Nomo transliteration.
 * This control keeps heap-backed length-carrying arrays and checked indexing to
 * approximate the semantic work visible in Nomo. It is not a C/C++ comparator.
 */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

typedef struct {
    double *data;
    size_t len;
} double_array;

static void bounds_fail(void)
{
    fputs("semantic-c array bounds failure\n", stderr);
    abort();
}

static double_array array_new(size_t len)
{
    double_array array = {calloc(len, sizeof(double)), len};
    if (array.data == NULL && len != 0)
        abort();
    return array;
}

static double array_get(const double_array *array, size_t index)
{
    if (index >= array->len)
        bounds_fail();
    return array->data[index];
}

static void array_set(double_array *array, size_t index, double value)
{
    if (index >= array->len)
        bounds_fail();
    array->data[index] = value;
}

static void array_drop(double_array *array)
{
    free(array->data);
    array->data = NULL;
    array->len = 0;
}

static double eval_a(size_t i, size_t j)
{
    size_t sum = i + j;
    return 1.0 / (double)(sum * (sum + 1) / 2 + i + 1);
}

static void eval_a_times_u(size_t n, const double_array *u, double_array *au)
{
    for (size_t i = 0; i < n; ++i) {
        array_set(au, i, 0.0);
        for (size_t j = 0; j < n; ++j)
            array_set(au, i, array_get(au, i) + eval_a(i, j) * array_get(u, j));
    }
}

static void eval_at_times_u(size_t n, const double_array *u, double_array *au)
{
    for (size_t i = 0; i < n; ++i) {
        array_set(au, i, 0.0);
        for (size_t j = 0; j < n; ++j)
            array_set(au, i, array_get(au, i) + eval_a(j, i) * array_get(u, j));
    }
}

static void eval_ata_times_u(size_t n, const double_array *u, double_array *atau)
{
    double_array temporary = array_new(n);
    eval_a_times_u(n, u, &temporary);
    eval_at_times_u(n, &temporary, atau);
    array_drop(&temporary);
}

int main(int argc, char **argv)
{
    size_t n = argc == 2 ? (size_t)strtoull(argv[1], NULL, 10) : 100;
    double_array u = array_new(n);
    double_array v = array_new(n);
    for (size_t i = 0; i < n; ++i)
        array_set(&u, i, 1.0);
    for (size_t i = 0; i < 10; ++i) {
        eval_ata_times_u(n, &u, &v);
        eval_ata_times_u(n, &v, &u);
    }
    double vbv = 0.0;
    double vv = 0.0;
    for (size_t i = 0; i < n; ++i) {
        vbv += array_get(&u, i) * array_get(&v, i);
        vv += array_get(&v, i) * array_get(&v, i);
    }
    printf("%0.9f\n", sqrt(vbv / vv));
    array_drop(&v);
    array_drop(&u);
    return 0;
}
