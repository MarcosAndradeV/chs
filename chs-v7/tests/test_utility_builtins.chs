module main

import "c/libc"
import . "assert"

type Point struct {
    x: int,
    y: int,
}

fn main() {
    // 1. Verify #sizeof and #alignof
    assert(#sizeof(int) == 4);
    assert(#sizeof(Point) == 8);
    assert(#alignof(Point) == 4);

    // 2. Verify new(T)
    {
        var p: ^Point = new(Point);
        defer drop(p); // Free allocated memory
        assert(p.x == 0);
        assert(p.y == 0);
        p.x = 42;
        assert(p.x == 42);
    };

    // 3. Verify make(T, count)
    {
        var s: []int = make([]int, 5);
        defer drop(s.data); // Free slice memory
        assert(s.len == 5);
        assert(s[0] == 0);
        assert(s[4] == 0);
        s[0] = 100;
        s[4] = 500;
        assert(s[0] == 100);
        assert(s[4] == 500);
    };

    libc.exit(0);
}
