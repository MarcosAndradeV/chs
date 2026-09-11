module main

import . "assert"

type Point struct {
    x: int,
    y: int,
}

fn main() {
    // 1. Array Literals
    var a1: [3]int = [3]int.{ 1, 2, 3 };
    assert(a1[0] == 1);
    assert(a1[1] == 2);
    assert(a1[2] == 3);

    var a2: [3]int = [3]int.{};
    assert(a2[0] == 0);
    assert(a2[1] == 0);
    assert(a2[2] == 0);

    var a3: [3]int = .{};
    assert(a3[0] == 0);
    assert(a3[1] == 0);
    assert(a3[2] == 0);

    var a4: [3]int = .{ 10, 20, 30 };
    assert(a4[0] == 10);
    assert(a4[2] == 30);

    // 2. Slice Literals
    var s1: []int = []int.{ 10, 20, 30 };
    assert(s1.len == 3);
    assert(s1[0] == 10);

    var s2: []int = []int.{};
    assert(s2.len == 0);

    var s3: []int = .{};
    assert(s3.len == 0);

    // 3. DynArray Literals
    var d1: [dyn]int = .{};
    push(&d1, 100, 200);
    assert(d1.len == 2);
    assert(d1[0] == 100);
    assert(d1[1] == 200);
    drop(d1);

    var d2: [dyn]int = [dyn]int.{};
    assert(d2.len == 0);

    var d3: [dyn]int = .{};
    assert(d3.len == 0);

    // 4. Struct Literals
    var p1 = Point.{ x: 5, y: 15 };
    assert(p1.x == 5);
    assert(p1.y == 15);

    var p2 = Point.{};
    assert(p2.x == 0);
    assert(p2.y == 0);

    var p3: Point = .{};
    assert(p3.x == 0);
    assert(p3.y == 0);
}
