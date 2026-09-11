module main

import . "assert"
import "c/libc"
import "mem"

type Point struct {
    x: int,
    y: int,
}

operator ==(p1: Point, p2: Point) -> bool {
    return p1.x == p2.x && p1.y == p2.y;
}

fn test_params(a: int, b: int, c: string) -> int {
    if c == "add" {
        return a + b;
    };
    if c == "sub" {
        return a - b;
    };
    return 0;
}

fn sum_slice(s: []int) -> int {
    var total = 0;
    var i = 0;
    for i < s.len {
        total = total + s[i];
        i = i + 1;
    };
    return total;
}

fn to_slice(s: []int) -> []int {
    return s;
}

fn main() {
    libc.printf("Checking Point operator == ...\n".data);
    var p1 = Point.{ x: 10, y: 20 };
    var p2 = Point.{ x: 10, y: 20 };
    var p3 = Point.{ x: 15, y: 20 };

    assert(p1 == p2);
    assert(!(p1 == p3));

    libc.printf("Checking named parameters ...\n".data);
    assert(test_params(a: 5, b: 3, c: "add") == 8);
    assert(test_params(c: "sub", a: 10, b: 4) == 6);
    assert(test_params(12, c: "add", b: 8) == 20);

    libc.printf("Checking pointers ...\n".data);
    var val = 42;
    var p_val: ^int = &val;
    assert(*p_val == 42);

    *p_val = 99;
    assert(val == 99);

    libc.printf("Checking arrays and slices ...\n".data);
    var arr: [4]int = .{10, 20, 30, 40};
    var slice: []int = to_slice(arr);
    assert(slice.len == 4);
    assert(slice[0] == 10);
    assert(slice[3] == 40);
    assert(sum_slice(slice) == 100);

    {
        libc.printf("Checking dynamic memory allocation ...\n".data);
        var dyn_ptr = new(int);
        defer drop(dyn_ptr);

        assert(dyn_ptr != null);
        assert(*dyn_ptr == 0);

        *dyn_ptr = 123;
        assert(*dyn_ptr == 123);
    };

    libc.printf("All checks passed!\n".data);
    libc.exit(0);
}
