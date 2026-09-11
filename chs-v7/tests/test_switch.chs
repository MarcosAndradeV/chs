module main

import . "assert"
import "c/libc"

type Point struct {
    x: int,
    y: int,
}

operator ==(a: Point, b: Point) -> bool {
    return a.x == b.x && a.y == b.y;
}

fn main() {
    var x = 2;
    var res = 0;
    switch x {
        1 -> res = 10;
        2 -> res = 20;
        _ -> res = 30;
    };
    assert(res == 20);

    var p = Point.{ x: 1, y: 2 };
    var matched = false;
    switch p {
        Point.{ x: 1, y: 3 } -> matched = false;
        Point.{ x: 1, y: 2 } -> matched = true;
        _ -> matched = false;
    };
    assert(matched);

    libc.exit(0);
}
