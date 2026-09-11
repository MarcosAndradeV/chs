module main

import "c/libc"
import "math"
import . "assert"

fn main() {
    // 1. Constants
    assert(math.PI > 3.14 && math.PI < 3.15);
    assert(math.TAU > 6.28 && math.TAU < 6.29);
    assert(math.E > 2.71 && math.E < 2.72);

    // 2. Integer math functions
    assert(math.abs(-42) == 42);
    assert(math.abs(42) == 42);
    assert(math.min(10, 20) == 10);
    assert(math.max(10, 20) == 20);
    assert(math.clamp(15, 0, 10) == 10);
    assert(math.clamp(-5, 0, 10) == 0);
    assert(math.clamp(5, 0, 10) == 5);
    assert(math.sign(-100) == -1);
    assert(math.sign(0) == 0);
    assert(math.sign(100) == 1);

    // 3. Float utilities
    assert(math.fmin(1.5, 2.5) == 1.5);
    assert(math.fmax(1.5, 2.5) == 2.5);
    assert(math.fclamp(15.0, 0.0, 10.0) == 10.0);
    assert(math.lerp(0.0, 10.0, 0.5) == 5.0);
    assert(math.deg_to_rad(180.0) > 3.14 && math.deg_to_rad(180.0) < 3.15);
    assert(math.rad_to_deg(math.PI) > 179.9 && math.rad_to_deg(math.PI) < 180.1);

    // 4. LibM Foreign functions
    assert(math.sqrt(16.0) == 4.0);
    assert(math.pow(2.0, 3.0) == 8.0);
    assert(math.floor(3.7) == 3.0);
    assert(math.ceil(3.2) == 4.0);
    assert(math.fabs(-5.5) == 5.5);

    libc.exit(0);
}
