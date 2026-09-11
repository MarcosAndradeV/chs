module main

import . "io"
import . "assert"

fn sum(prefix: string, nums: ...int) -> int {
    var total = 0;
    foreach n in nums {
        total = total + n;
    };
    return total;
}

fn count_args(args: ...string) -> int {
    return cast(int) args.len;
}

fn main() {
    // 1. Typed variadic ...int
    var r0 = sum("Zero");
    assert(r0 == 0);

    var r1 = sum("One", 42);
    assert(r1 == 42);

    var r3 = sum("Three", 10, 20, 30);
    assert(r3 == 60);

    // Direct slice passing
    var s: [4]int = .{1, 2, 3, 4};
    var r_slice = sum("Slice", s);
    assert(r_slice == 10);

    // 2. Typed variadic ...string
    assert(count_args() == 0);
    assert(count_args("a") == 1);
    assert(count_args("a", "b", "c") == 3);

    // 3. Typed variadic ...Any with io.print
    io.print("Testing variadic print: % %\n", "hello", 123);
    io.print("No args print\n");
}
