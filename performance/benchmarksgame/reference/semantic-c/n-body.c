/* Non-decisional semantic-C diagnostic for the frozen n-body workload.
 *
 * BSD-3-Clause derivative of nbody-gcc-8 and the Nomo transliteration. This
 * control uses a heap-backed length-carrying array, checked indexing, value
 * loads, and explicit struct writeback. It is not a C/C++ comparator.
 */

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#define PI 3.141592653589793
#define SOLAR_MASS (4 * PI * PI)
#define DAYS_PER_YEAR 365.24

typedef struct {
    double x, y, z;
    double vx, vy, vz;
    double mass;
} body;

typedef struct {
    body *data;
    size_t len;
} body_array;

static void bounds_fail(void)
{
    fputs("semantic-c array bounds failure\n", stderr);
    abort();
}

static body body_get(const body_array *array, size_t index)
{
    if (index >= array->len)
        bounds_fail();
    return array->data[index];
}

static void body_set(body_array *array, size_t index, body value)
{
    if (index >= array->len)
        bounds_fail();
    array->data[index] = value;
}

static void offset_momentum(body_array *bodies)
{
    double px = 0.0, py = 0.0, pz = 0.0;
    for (size_t i = 0; i < bodies->len; ++i) {
        body current = body_get(bodies, i);
        px += current.vx * current.mass;
        py += current.vy * current.mass;
        pz += current.vz * current.mass;
    }
    body sun = body_get(bodies, 0);
    sun.vx = -px / SOLAR_MASS;
    sun.vy = -py / SOLAR_MASS;
    sun.vz = -pz / SOLAR_MASS;
    body_set(bodies, 0, sun);
}

static double energy(const body_array *bodies)
{
    double value = 0.0;
    for (size_t i = 0; i < bodies->len; ++i) {
        body current = body_get(bodies, i);
        double square = current.vx * current.vx + current.vy * current.vy + current.vz * current.vz;
        value += 0.5 * current.mass * square;
        for (size_t j = i + 1; j < bodies->len; ++j) {
            body other = body_get(bodies, j);
            double dx = current.x - other.x;
            double dy = current.y - other.y;
            double dz = current.z - other.z;
            double distance_squared = dx * dx + dy * dy + dz * dz;
            value -= current.mass * other.mass / sqrt(distance_squared);
        }
    }
    return value;
}

static void advance(body_array *bodies, double dt)
{
    for (size_t i = 0; i < bodies->len; ++i) {
        for (size_t j = i + 1; j < bodies->len; ++j) {
            body current = body_get(bodies, i);
            body other = body_get(bodies, j);
            double dx = current.x - other.x;
            double dy = current.y - other.y;
            double dz = current.z - other.z;
            double distance_squared = dx * dx + dy * dy + dz * dz;
            double magnitude = dt / (distance_squared * sqrt(distance_squared));
            double other_mass = other.mass * magnitude;
            current.vx -= dx * other_mass;
            current.vy -= dy * other_mass;
            current.vz -= dz * other_mass;
            double current_mass = current.mass * magnitude;
            other.vx += dx * current_mass;
            other.vy += dy * current_mass;
            other.vz += dz * current_mass;
            body_set(bodies, i, current);
            body_set(bodies, j, other);
        }
    }
    for (size_t i = 0; i < bodies->len; ++i) {
        body current = body_get(bodies, i);
        current.x += current.vx * dt;
        current.y += current.vy * dt;
        current.z += current.vz * dt;
        body_set(bodies, i, current);
    }
}

int main(int argc, char **argv)
{
    body *storage = malloc(5 * sizeof(body));
    if (storage == NULL)
        abort();
    body_array bodies = {storage, 5};
    storage[0] = (body){0, 0, 0, 0, 0, 0, SOLAR_MASS};
    storage[1] = (body){4.84143144246472090, -1.16032004402742839,
                        -0.103622044471123109, 0.00166007664274403694 * DAYS_PER_YEAR,
                        0.00769901118419740425 * DAYS_PER_YEAR,
                        -0.0000690460016972063023 * DAYS_PER_YEAR,
                        0.000954791938424326609 * SOLAR_MASS};
    storage[2] = (body){8.34336671824457987, 4.12479856412430479,
                        -0.403523417114321381, -0.00276742510726862411 * DAYS_PER_YEAR,
                        0.00499852801234917238 * DAYS_PER_YEAR,
                        0.000023041729757633929 * DAYS_PER_YEAR,
                        0.000285885980666130812 * SOLAR_MASS};
    storage[3] = (body){12.8943695621391310, -15.1111514016986312,
                        -0.223307579892655734, 0.00296460137564761618 * DAYS_PER_YEAR,
                        0.00237847173959480950 * DAYS_PER_YEAR,
                        -0.0000296589568540237558 * DAYS_PER_YEAR,
                        0.0000436624404335156298 * SOLAR_MASS};
    storage[4] = (body){15.3796971148509165, -25.9193146099879641,
                        0.179258772950371181, 0.00268067772490389322 * DAYS_PER_YEAR,
                        0.00162824170038242295 * DAYS_PER_YEAR,
                        -0.0000951592254519715870 * DAYS_PER_YEAR,
                        0.0000515138902046611451 * SOLAR_MASS};
    offset_momentum(&bodies);
    printf("%.9f\n", energy(&bodies));
    size_t iterations = argc > 1 ? (size_t)strtoull(argv[1], NULL, 10) : 1000;
    for (size_t i = 0; i < iterations; ++i)
        advance(&bodies, 0.01);
    printf("%.9f\n", energy(&bodies));
    free(storage);
    return 0;
}
