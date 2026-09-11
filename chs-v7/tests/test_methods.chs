module main

import . "assert"
import "c/libc"
import "io"

type Point struct {
    x: int,
    y: int,
}

fn (self: Point) sum() -> int {
    return self.x + self.y;
}

fn (self: ^Point) add_x(dx: int) {
    self.x = self.x + dx;
}

fn main() {
    var p = Point.{ x: 10, y: 20 };
    var s = p.sum();
    io.puts("Checking assertion 1");
    assert(s == 30);

    p.add_x(5);
    var s2 = p.sum();
    libc.printf("p.x after add_x: %d, s2: %d\n".data, p.x, s2);
    assert(s2 == 35);

    var s3 = Point.sum(p);
    libc.printf("s3: %d\n".data, s3);
    assert(s3 == 35);

    io.puts("Methods test passed successfully!");
    libc.exit(0);
}
