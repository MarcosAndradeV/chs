module main

import . "assert"
import "io"

fn add_f32(a: f32, b: f32) -> f32 {
    return a + b;
}

fn main() {
    var x: f32 = 1.5;
    var y: f32 = 2.5;
    var z: f32 = add_f32(x, y);

    assert(z > 3.9);
    assert(z < 4.1);

    var diff: f32 = y - x;
    assert(diff > 0.9);
    assert(diff < 1.1);

    var prod: f32 = x * y;
    assert(prod > 3.7);
    assert(prod < 3.8);

    var quot: f32 = y / x;
    assert(quot > 1.6);
    assert(quot < 1.7);

    // Casts
    var d: float = cast(float) x;
    assert(d > 1.49 && d < 1.51);

    var i: int = cast(int) y;
    assert(i == 2);

    var f_from_i: f32 = cast(f32) i;
    assert(f_from_i > 1.9 && f_from_i < 2.1);

    var lit_res: f32 = add_f32(10.5, 20.5);
    assert(lit_res > 30.9 && lit_res < 31.1);

    io.puts("Float32 tests passed successfully!");
}
