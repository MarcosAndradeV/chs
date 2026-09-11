module main

import . "assert"
import "c/libc"
import "io"
import "methodmod"

fn main() {
    var p = methodmod.Point.{ x: 10, y: 20 };
    var s = p.sum();
    assert(s == 30);

    p.add_x(5);
    assert(p.x == 15);

    var s2 = methodmod.Point.sum(p);
    assert(s2 == 35);

    io.puts("Imported methods test passed successfully\n");
    libc.exit(0);
}
