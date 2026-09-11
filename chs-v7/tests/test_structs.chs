module main

import . "assert"
import "c/libc"

type Point struct {
    x: int,
    y: int,
}

type Rectangle struct {
    origin: Point,
    width: int,
    height: int,
}

fn area(r: Rectangle) -> int {
    return r.width * r.height;
}

fn move_point(p: ^Point, dx: int, dy: int) {
    p.x = p.x + dx;
    p.y = p.y + dy;
}

fn main() {
    var p = Point.{ x: 10, y: 20 };
    assert(p.x == 10);
    assert(p.y == 20);

    move_point(&p, 5, -5);
    assert(p.x == 15);
    assert(p.y == 15);

    var rect = Rectangle.{
        origin: p,
        width: 100,
        height: 50,
    };
    assert(rect.origin.x == 15);
    assert(area(rect) == 5000);
    libc.exit(0);
}
