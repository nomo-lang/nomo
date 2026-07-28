/* Derived line-for-line from the Computer Language Benchmarks Game
 * fannkuchredux-gcc-8 program in ../c/fannkuch-redux.c.
 *
 * BSD-3-Clause. Standard C++20 spelling only; the algorithm, loop order,
 * contiguous per-array storage, initialization, and output are unchanged.
 * Each runtime-sized C VLA maps to one standard RAII dynamic array with the
 * same lexical lifetime and per-call allocation frequency. The
 * stack-to-dynamic difference is documented in SOURCES-v2.md.
 */

#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <memory>

int fannkuch(int n)
{
    std::unique_ptr<int[]> perm1(new int[n]);
    for (int i = 0; i < n; i += 1)
        perm1[i] = i;
    std::unique_ptr<int[]> perm(new int[n]);
    std::unique_ptr<int[]> count(new int[n]);
    int f = 0, flips = 0, nperm = 0, checksum = 0;
    int i, k, r;

    r = n;
    while (r > 0) {
        i = 0;
        while (r != 1) {
            count[r - 1] = r;
            r -= 1;
        }
        while (i < n) {
            perm[i] = perm1[i];
            i += 1;
        }

        // Count flips and update max and checksum
        f = 0;
        k = perm[0];
        while (k != 0) {
            i = 0;
            while (2 * i < k) {
                int t = perm[i];
                perm[i] = perm[k - i];
                perm[k - i] = t;
                i += 1;
            }
            k = perm[0];
            f += 1;
        }
        if (f > flips)
            flips = f;
        if ((nperm & 0x1) == 0)
            checksum += f;
        else
            checksum -= f;

        // Use incremental change to generate another permutation
        bool more = true;
        while (more) {
            if (r == n) {
                printf("%d\n", checksum);
                return flips;
            }
            int p0 = perm1[0];
            i = 0;
            while (i < r) {
                int j = i + 1;
                perm1[i] = perm1[j];
                i = j;
            }
            perm1[r] = p0;

            count[r] -= 1;
            if (count[r] > 0)
                more = false;
            else
                r += 1;
        }
        nperm += 1;
    }
    return flips;
}

int main(int argc, char *argv[])
{
    int n = argc > 1 ? atoi(argv[1]) : 7;
    printf("Pfannkuchen(%d) = %d\n", n, fannkuch(n));
    return 0;
}
