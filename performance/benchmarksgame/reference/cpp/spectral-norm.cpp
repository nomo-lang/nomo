/* Derived line-for-line from the Computer Language Benchmarks Game
 * spectralnorm-gcc-8 program in ../c/spectral-norm.c.
 *
 * BSD-3-Clause. Standard C++20 spelling only; the algorithm, loop order,
 * arithmetic, contiguous per-array storage, initialization, and output are
 * unchanged. Each runtime-sized C VLA maps to one standard RAII dynamic array
 * with the same lexical lifetime and per-call allocation frequency. The
 * stack-to-dynamic difference is documented in SOURCES-v2.md.
 */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <memory>

double eval_A(int i, int j) { return 1.0 / ((i + j) * (i + j + 1) / 2 + i + 1); }

void eval_A_times_u(int N, const double u[], double Au[])
{
    int i, j;
    for (i = 0; i < N; i++) {
        Au[i] = 0;
        for (j = 0; j < N; j++)
            Au[i] += eval_A(i, j) * u[j];
    }
}

void eval_At_times_u(int N, const double u[], double Au[])
{
    int i, j;
    for (i = 0; i < N; i++) {
        Au[i] = 0;
        for (j = 0; j < N; j++)
            Au[i] += eval_A(j, i) * u[j];
    }
}

void eval_AtA_times_u(int N, const double u[], double AtAu[])
{
    std::unique_ptr<double[]> v(new double[N]);
    eval_A_times_u(N, u, v.get());
    eval_At_times_u(N, v.get(), AtAu);
}

int main(int argc, char *argv[])
{
    int i;
    const int N = ((argc == 2) ? atoi(argv[1]) : 100);
    std::unique_ptr<double[]> u(new double[N]);
    std::unique_ptr<double[]> v(new double[N]);
    double vBv, vv;
    for (i = 0; i < N; i++)
        u[i] = 1;
    for (i = 0; i < 10; i++) {
        eval_AtA_times_u(N, u.get(), v.get());
        eval_AtA_times_u(N, v.get(), u.get());
    }
    vBv = vv = 0;
    for (i = 0; i < N; i++) {
        vBv += u[i] * v[i];
        vv += v[i] * v[i];
    }
    printf("%0.9f\n", sqrt(vBv / vv));
    return 0;
}
